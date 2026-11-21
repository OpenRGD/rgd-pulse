//! PULSE-0.4B: one USB CDC owner, on-demand K3 IMU or K4 Linux host.
//! The Pico selects its own visible page; host cannot navigate its UI.
//! No WIT/CH340 serial reader, CAN socket, motor command or actuation.
//! The IMU publisher is independent and is NEVER auto-started/stopped.
//! A single physical Pico CDC session is the only mutable transport handle.
//!
//! K3: subscribe to the existing Console read-only WS only while viewed;
//!       drop subscription when hidden, forward <=10Hz valid physical IMU.
//! K4: sample physical Linux /proc + loopback Console health at 1Hz while viewed.
//! K1/K2: no host sampler or IMU subscriber; only low-rate page hints.
//! CRC, PULSE/1 HELLO, IMU Probe/ACK and SYS Probe/ACK gates are unchanged.

use crate::{
    console_imu::{self, StreamGuard},
    imu_live,
    system_host,
};
use pulse_protocol::{
    imu_wire::{self, Frame as ImuFrame},
    system_wire::{self, Frame as SystemFrame, PageHint},
};
use pulse_ui::imu::Origin;
use std::{
    error::Error,
    io::{self, Read, Write},
    thread,
    time::{Duration, Instant},
};
use tungstenite::{Error as WsError, Message};

const PAGE_HINT_TTL: Duration = Duration::from_secs(5);
const IMU_MIN_TX_INTERVAL: Duration = Duration::from_millis(100);
const SYS_MIN_TX_INTERVAL: Duration = Duration::from_secs(1);
const WS_RECONNECT_INTERVAL: Duration = Duration::from_secs(3);
const PICO_READ_TIMEOUT: Duration = Duration::from_millis(50);
const PAGE_IMU: u8 = 2;
const PAGE_SYSTEM: u8 = 3;

#[derive(Default)]
struct PageLease {
    last_seq: Option<u32>,
    last_page: Option<u8>,
    last_received: Option<Instant>,
}

impl PageLease {
    /// Ignore replayed or malformed UI metadata; this is not authority
    /// over robot operations, but avoids incorrect bandwidth selection.
    fn observe(&mut self, hint: PageHint, now: Instant) -> bool {
        if hint.sequence == 0 || hint.page > PAGE_SYSTEM {
            return false;
        }
        if let Some(last) = self.last_seq {
            let diff = hint.sequence.wrapping_sub(last);
            if diff == 0 || diff >= 0x8000_0000 {
                return false;
            }
        }
        self.last_seq = Some(hint.sequence);
        self.last_page = Some(hint.page);
        self.last_received = Some(now);
        true
    }

    fn current(&self, now: Instant) -> Option<u8> {
        let (Some(page), Some(received)) = (self.last_page, self.last_received) else {
            return None;
        };
        if now
            .checked_duration_since(received)
            .map(|delta| delta <= PAGE_HINT_TTL)
            .unwrap_or(false)
        {
            Some(page)
        } else {
            None
        }
    }
}

fn due(last: Option<Instant>, interval: Duration, now: Instant) -> bool {
    last.and_then(|t| now.checked_duration_since(t))
        .map(|elapsed| elapsed >= interval)
        .unwrap_or(true)
}

/// Dedicated prototype port identity, never WIT CH340 or another ttyACM.
fn valid_pico_path(path: &str) -> bool {
    const PREFIX: &str = "/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_";
    const SUFFIX: &str = "-if00";
    if !path.starts_with(PREFIX) || !path.ends_with(SUFFIX) {
        return false;
    }
    let mid = &path[PREFIX.len()..path.len() - SUFFIX.len()];
    !mid.is_empty()
        && mid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// One subscription *only* while K3 is visible. A stale/missing WS cannot
/// prevent K4 from working, and no automatic publisher lifecycle is implied.
pub fn run(serial_by_id: &str, seconds: u64) -> Result<(), Box<dyn Error>> {
    if !(1..=600).contains(&seconds) {
        return Err("duration must be 1..600 seconds".into());
    }
    if !valid_pico_path(serial_by_id) {
        return Err("explicit Pico CDC by-id required, never a WIT/CH340 or guessed tty".into());
    }
    // Check host provenance BEFORE opening the Pico. This rejects known
    // containers and fails cleanly on missing/malformed physical Linux data.
    let _ = system_host::host_snapshot(1)?;
    let mut port = serialport::new(serial_by_id, 115_200)
        .timeout(PICO_READ_TIMEOUT)
        .open()?;
    imu_live::expect_pico_hello(&mut *port)?;
    println!("PULSE/1 HELLO OK");
    imu_live::require_imu_capability(&mut *port)?;
    println!("PULSE/IMU-1 PROBE/ACK OK");
    system_host::require_sys_capability(&mut *port)?;
    println!("PULSE/SYS-1 PROBE/ACK OK");

    // Host asks for page *observation*, not UI control. The Pico only
    // sends this if it accepted a SYS probe in the same USB CDC session.
    port.write_all(&system_wire::encode(SystemFrame::SubscribePage))?;
    println!("PULSE/SYS-1 page subscription requested (Pico decides current K page)");
    println!("Adaptive policy: K3 IMU only when viewed, K4 Linux at 1Hz when viewed");
    println!("Publisher is independently managed; NO CH340/CAN/motor access");

    let mut page_decoder = system_wire::Decoder::new();
    let mut page = PageLease::default();
    let mut reader = [0u8; 64];
    let mut ws = None;
    let mut ws_retry_after = Instant::now();
    let mut source_guard = StreamGuard::default();
    let mut last_imu_tx: Option<Instant> = None;
    let mut last_sys_tx: Option<Instant> = None;
    let mut previous_page: Option<u8> = None;
    let mut imu_sent = 0u32;
    let mut sys_sent = 0u32;
    let mut imu_rejected = 0u32;
    let started = Instant::now();
    let deadline = started + Duration::from_secs(seconds);

    while Instant::now() < deadline {
        // Only one thread reads/writes Pico CDC; no race with standalone
        // imu-live/system-live. 50ms bounded serial read avoids busy spinning.
        match port.read(&mut reader) {
            Ok(len) => {
                for byte in reader[..len].iter().copied() {
                    if let Some(Ok(SystemFrame::PageHint(hint))) = page_decoder.push(byte) {
                        if page.observe(hint, Instant::now()) {
                            if previous_page != Some(hint.page) {
                                println!("PICO VIEW = K{}", hint.page + 1);
                            }
                        }
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => return Err(Box::new(error)),
        }

        let now = Instant::now();
        let active = page.current(now);
        if active != previous_page {
            // On each visible page transition give the new source a quick
            // refresh, rather than waiting for the old page's send interval.
            last_imu_tx = None;
            last_sys_tx = None;
            if previous_page == Some(PAGE_IMU) && active != Some(PAGE_IMU) {
                ws = None; // drop local WS subscription: no unnecessary read
            }
            previous_page = active;
        }

        if active == Some(PAGE_SYSTEM) && due(last_sys_tx, SYS_MIN_TX_INTERVAL, now) {
            let seq = sys_sent.checked_add(1).ok_or("SYS sequence overflow")?;
            let observation = system_host::host_snapshot(seq)?;
            port.write_all(&system_wire::encode(SystemFrame::Snapshot(observation)))?;
            sys_sent = seq;
            last_sys_tx = Some(Instant::now());
            if sys_sent <= 3 || sys_sent % 10 == 0 {
                println!(
                    "K4 HOST #{} uptime={}s RAM={}% Console /healthz={}",
                    sys_sent,
                    observation.host_uptime_s,
                    observation.ram_used_percent,
                    if observation.console_health_ok { "OK" } else { "N/A" },
                );
            }
        }

        // K1/K2/K4 never open or poll the Console IMU WS. IMU publisher
        // remains untouched and can still serve other Console subscribers.
        if active != Some(PAGE_IMU) {
            ws = None;
            continue;
        }

        if ws.is_none() && now >= ws_retry_after {
            match imu_live::connect_console() {
                Ok(socket) => {
                    println!("Console read-only WS subscribed (K3 visible)");
                    ws = Some(socket);
                    source_guard = StreamGuard::default();
                }
                Err(_) => {
                    // No secrets, arbitrary endpoints or verbose network
                    // error bodies; K3 goes STALE/NO SOURCE as designed.
                    ws_retry_after = Instant::now() + WS_RECONNECT_INTERVAL;
                }
            }
        }

        let mut lost_ws = false;
        if let Some(socket) = ws.as_mut() {
            match socket.read() {
                Ok(Message::Text(json)) => {
                    // Parse and quantize only when K3 is actually visible.
                    let host_now_ns = imu_live::host_monotonic_ns()?;
                    match console_imu::parse_stream_frame(
                        &json,
                        host_now_ns,
                        Origin::ConsoleReadOnlyObservation,
                    ) {
                        Ok(Some((sample, source_ns))) => {
                            if source_guard.accept(sample, source_ns, host_now_ns).is_err() {
                                imu_rejected = imu_rejected.saturating_add(1);
                                continue;
                            }
                            if !due(last_imu_tx, IMU_MIN_TX_INTERVAL, Instant::now()) {
                                continue;
                            }
                            match console_imu::to_wire(sample) {
                                Ok(wire) => {
                                    port.write_all(
                                        &imu_wire::encode(ImuFrame::SensorSample(wire)),
                                    )?;
                                    imu_sent = imu_sent.saturating_add(1);
                                    last_imu_tx = Some(Instant::now());
                                    if imu_sent <= 3 || imu_sent % 10 == 0 {
                                        println!(
                                            "K3 IMU #{} seq={} age={}ms RAW / YAW_UNCAL",
                                            imu_sent,
                                            sample.source_sequence,
                                            sample.host_age_at_forward_ms.unwrap_or(u32::MAX),
                                        );
                                    }
                                }
                                Err(_) => {
                                    imu_rejected = imu_rejected.saturating_add(1);
                                }
                            }
                        }
                        Ok(None) => {} // heartbeat, telemetry, non-IMU event
                        Err(_) => {
                            imu_rejected = imu_rejected.saturating_add(1);
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    socket.send(Message::Pong(data))?;
                }
                Ok(Message::Close(_)) => lost_ws = true,
                Ok(_) => {}
                Err(WsError::Io(ref e))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => lost_ws = true,
            }
        }
        if lost_ws {
            ws = None;
            ws_retry_after = Instant::now() + WS_RECONNECT_INTERVAL;
        }
    }

    // Closing DTR drops both Pico live sessions; no stale live data remain.
    println!(
        "PULSE MUX SUMMARY: IMU forwarded={} SYS forwarded={} rejected={} (source-only)",
        imu_sent, sys_sent, imu_rejected,
    );
    println!("No robot safety, CAN or motor state inferred.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_hints_are_required_and_expire_without_new_pico_observation() {
        let t0 = Instant::now();
        let mut page = PageLease::default();
        assert_eq!(page.current(t0), None);
        assert!(page.observe(PageHint { sequence: 1, page: PAGE_IMU }, t0));
        assert_eq!(page.current(t0), Some(PAGE_IMU));
        assert_eq!(page.current(t0 + PAGE_HINT_TTL), Some(PAGE_IMU));
        assert_eq!(page.current(t0 + PAGE_HINT_TTL + Duration::from_millis(1)), None);
    }

    #[test]
    fn page_switch_only_changes_forwarding_policy() {
        let t0 = Instant::now();
        let mut page = PageLease::default();
        page.observe(PageHint { sequence: 10, page: PAGE_IMU }, t0);
        assert_eq!(page.current(t0), Some(PAGE_IMU));
        page.observe(PageHint { sequence: 11, page: PAGE_SYSTEM }, t0 + Duration::from_millis(10));
        assert_eq!(page.current(t0 + Duration::from_millis(10)), Some(PAGE_SYSTEM));
        page.observe(PageHint { sequence: 12, page: 0 }, t0 + Duration::from_millis(20));
        assert_eq!(page.current(t0 + Duration::from_millis(20)), Some(0));
    }

    #[test]
    fn rejects_replayed_hint_wrong_page_or_zero_sequence() {
        let mut page = PageLease::default();
        let t0 = Instant::now();
        assert!(page.observe(PageHint { sequence: 5, page: 3 }, t0));
        for bad in [
            PageHint { sequence: 5, page: 2 },
            PageHint { sequence: 4, page: 2 },
            PageHint { sequence: 0, page: 3 },
            PageHint { sequence: 6, page: 4 },
        ] {
            assert!(!page.observe(bad, t0));
        }
        assert_eq!(page.current(t0), Some(3));
    }

    #[test]
    fn page_seq_wrap_and_independent_rates() {
        let mut page = PageLease::default();
        let t0 = Instant::now();
        assert!(page.observe(PageHint { sequence: u32::MAX, page: 2 }, t0));
        assert!(page.observe(PageHint { sequence: 1, page: 3 }, t0));
        assert!(!due(Some(t0), SYS_MIN_TX_INTERVAL, t0 + Duration::from_millis(999)));
        assert!(due(Some(t0), SYS_MIN_TX_INTERVAL, t0 + Duration::from_secs(1)));
        assert!(due(Some(t0), IMU_MIN_TX_INTERVAL, t0 + Duration::from_millis(100)));
    }

    #[test]
    fn pico_port_identity_never_matches_ch340_or_nested_paths() {
        assert!(valid_pico_path(
            "/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00"
        ));
        for wrong in [
            "/dev/ttyACM0",
            "/dev/serial/by-path/pci-0000:00:14.0-usb-0:2.4:1.0-port0",
            "/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_../../ttyUSB0-if00",
            "/dev/serial/by-id/usb-Other_JY901-if00",
        ] {
            assert!(!valid_pico_path(wrong));
        }
    }
}
