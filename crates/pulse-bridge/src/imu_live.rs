//! Bounded read-only IMU subscriber: local Console broadcast -> Pico USB.
//!
//! Never opens the WIT/CH340 serial device. Never calls Console POST.
//! No secrets or publisher tokens transmitted. This path is explicitly
//! loopback-only; Console GET/WS lack user authentication in v0.5, so this
//! is a trusted-local prototype, not an authenticated remote connection.
//! Commands on Pico USB carry ONLY sensor observations/capability probes.
//! No CAN, motor, calibration or actuator messages are defined here.

use crate::console_imu::{self, Rejection, StreamGuard, WS_STREAM_URL};
use pulse_protocol::{
    imu_wire::{self, Decoder as ImuDecoder, Frame as ImuFrame},
    Kind as PulseKind, StreamDecoder as PulseDecoder,
};
use pulse_ui::imu::Origin;
use serialport::SerialPort;
use std::{
    error::Error,
    io::{self, Read, Write},
    thread,
    time::{Duration, Instant},
};
use tungstenite::{
    stream::MaybeTlsStream,
    Message,
};

const MAX_TX_RATE_HZ: u64 = 10;
const MIN_TX_INTERVAL: Duration = Duration::from_millis(100);
const SERIAL_TIMEOUT: Duration = Duration::from_millis(80);
const WS_TIMEOUT: Duration = Duration::from_millis(150);
const PROTOTYPE_BOARD_ID: u32 = 1;

/// clock_gettime(CLOCK_MONOTONIC) shares the clock domain with Python
/// time.monotonic_ns() from Paky-01's publisher on this SAME Linux host.
/// std::time::Instant has an unspecified epoch and must NOT be mixed with
/// Python's absolute monotonic-ns values.
pub fn host_monotonic_ns() -> io::Result<u64> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if ts.tv_sec < 0 || !(0..1_000_000_000).contains(&ts.tv_nsec) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid CLOCK_MONOTONIC"));
    }
    Ok((ts.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(ts.tv_nsec as u64))
}

fn expect_pico_hello(port: &mut dyn SerialPort) -> Result<(), Box<dyn Error>> {
    port.write_data_terminal_ready(false)?;
    thread::sleep(Duration::from_millis(300));
    port.write_data_terminal_ready(true)?;
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut decoder = PulseDecoder::new();
    let mut buf = [0u8; 64];
    while Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(count) => {
                for byte in buf[..count].iter().copied() {
                    if let Some(Ok(frame)) = decoder.push(byte) {
                        if frame.kind == PulseKind::Hello && frame.value == PROTOTYPE_BOARD_ID {
                            return Ok(());
                        }
                    }
                }
            }
            Err(error) if matches!(error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {}
            Err(error) => return Err(Box::new(error)),
        }
    }
    Err("Pico PULSE/1 HELLO missing: abort IMU subscription without sending sensor data".into())
}

fn require_imu_capability(port: &mut dyn SerialPort) -> Result<(), Box<dyn Error>> {
    port.write_all(&imu_wire::encode(ImuFrame::Probe))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut decoder = ImuDecoder::new();
    let mut buf = [0u8; 64];
    while Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(count) => {
                for byte in buf[..count].iter().copied() {
                    match decoder.push(byte) {
                        Some(Ok(ImuFrame::ProbeAck)) => return Ok(()),
                        Some(Ok(_)) | Some(Err(_)) | None => {}
                    }
                }
            }
            Err(error) if matches!(error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {}
            Err(error) => return Err(Box::new(error)),
        }
    }
    Err("Pico PULSE/IMU-1 capability ACK missing; old firmware? no IMU samples sent".into())
}

/// Uses a fixed local WS endpoint only; never accepts arbitrary URLs to
/// avoid SSRF/network disclosure and accidental LAN gateway exposure.
fn connect_console() -> Result<tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>, Box<dyn Error>> {
    let (mut socket, _) = tungstenite::connect(WS_STREAM_URL)?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(Some(WS_TIMEOUT))?,
        _ => return Err("unexpected remote/TLS stream; only loopback WS is allowed".into()),
    }
    Ok(socket)
}

fn report_sample(sample: &pulse_ui::imu::ImuSample, count: u32) {
    // Report numbers + source seq, never transmit tokens or dump raw JSON.
    println!(
        "IMU #{} seq={} SENSOR XYZ roll={:.2} pitch={:.2} yaw*={:.2} age={}ms YAW_UNCAL",
        count,
        sample.source_sequence,
        sample.roll_cdeg as f64 / 100.0,
        sample.pitch_cdeg as f64 / 100.0,
        sample.yaw_cdeg as f64 / 100.0,
        sample.host_age_at_forward_ms.unwrap_or(u32::MAX),
    );
}

pub fn run(serial_by_id: &str, seconds: u64) -> Result<(), Box<dyn Error>> {
    if seconds == 0 || seconds > 600 {
        return Err("duration must be 1..600 seconds".into());
    }
    if !serial_by_id.starts_with("/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_")
        || !serial_by_id.ends_with("-if00")
    {
        return Err("Pico by-id USB CDC identity required; never use IMU/CH340 or guessed ttyACM0".into());
    }
    println!("PULSE/IMU-1 read-only physical sensor; NO MOTION / SENSOR AXES UNVERIFIED");
    println!("Connecting only to Console WebSocket at ws://127.0.0.1:8088");
    println!("Warning: Console loopback listener is NOT user-authenticated; do not expose on LAN");

    // Independent USB CDC port for the Pico. This is NOT the IMU's serial.
    let mut port = serialport::new(serial_by_id, 115_200)
        .timeout(SERIAL_TIMEOUT)
        .open()?;
    expect_pico_hello(&mut *port)?;
    println!("PULSE/1 HELLO OK");
    require_imu_capability(&mut *port)?;
    println!("PULSE/IMU-1 PROBE/ACK OK (Pico supports typed sensor frames)");

    let mut ws = connect_console()?;
    println!("Console v0.5 WS connected (same-host, read-only)");
    let mut guard = StreamGuard::default();
    let mut sent: u32 = 0;
    let mut rejected: u32 = 0;
    let mut ignored: u32 = 0;
    let mut last_tx: Option<Instant> = None;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < deadline {
        match ws.read() {
            Ok(Message::Text(json)) => {
                let now_ns = host_monotonic_ns()?;
                match console_imu::parse_stream_frame(
                    &json,
                    now_ns,
                    Origin::ConsoleReadOnlyObservation,
                ) {
                    Ok(None) => ignored = ignored.saturating_add(1),
                    Ok(Some((sample, source_ns))) => {
                        if let Err(e) = guard.accept(sample, source_ns, now_ns) {
                            rejected = rejected.saturating_add(1);
                            if rejected <= 3 {
                                eprintln!("IMU sample rejected by replay/shape guard: {e:?}");
                            }
                            continue;
                        }
                        // Keep only latest accepted sample under excessive WS
                        // rates; do not block the Console broadcaster.
                        let may_tx = last_tx
                            .map(|t| t.elapsed() >= MIN_TX_INTERVAL)
                            .unwrap_or(true);
                        if !may_tx {
                            continue;
                        }
                        let wire = match console_imu::to_wire(sample) {
                            Ok(wire) => wire,
                            Err(e) => {
                                rejected = rejected.saturating_add(1);
                                if rejected <= 3 {
                                    eprintln!("IMU sample rejected by wire bounds: {e:?}");
                                }
                                continue;
                            }
                        };
                        port.write_all(&imu_wire::encode(ImuFrame::SensorSample(wire)))?;
                        last_tx = Some(Instant::now());
                        sent = sent.saturating_add(1);
                        if sent <= 3 || sent % MAX_TX_RATE_HZ as u32 == 0 {
                            report_sample(&sample, sent);
                        }
                    }
                    Err(Rejection::NotImu) => ignored = ignored.saturating_add(1),
                    Err(e) => {
                        rejected = rejected.saturating_add(1);
                        if rejected <= 3 {
                            eprintln!("Console message rejected: {e:?}");
                        }
                    }
                }
            }
            Ok(Message::Ping(content)) => {
                ws.send(Message::Pong(content))?;
            }
            Ok(Message::Close(_)) => {
                eprintln!("Console WS closed; Pico marks the last IMU sample STALE");
                break;
            }
            Ok(_) => ignored = ignored.saturating_add(1),
            Err(tungstenite::Error::Io(ref error)) if matches!(error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {}
            Err(error) => return Err(Box::new(error)),
        }
    }
    println!("PULSE IMU SUMMARY: forwarded={sent} rejected={rejected} non_imu={ignored}");
    println!("When no more IMU data arrives, Pico must show STALE then NO SOURCE after reset.");
    if sent == 0 {
        return Err("no verified IMU observations forwarded; confirm publisher & Console WS".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_is_hardcoded_to_ipv4_loopback_no_write_api() {
        assert_eq!(WS_STREAM_URL, "ws://127.0.0.1:8088/api/v1/stream?robot_id=paky-01");
        assert!(!WS_STREAM_URL.contains("0.0.0.0"));
        assert_eq!(console_imu::REST_LATEST_URL,
            "http://127.0.0.1:8088/api/v1/robots/paky-01/imu");
    }

    #[test]
    fn clock_monotonic_on_linux_is_nonnegative_and_advances() {
        let first = host_monotonic_ns().unwrap();
        let second = host_monotonic_ns().unwrap();
        assert!(second >= first);
    }
}
