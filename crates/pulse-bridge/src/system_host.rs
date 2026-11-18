//! Read-only host-Linux observation and bounded Pico System-only bridge.
//! Physical source is /proc/uptime and /proc/meminfo on the running host,
//! plus local Console /healthz; no Docker or WIT device reads are made.
//! "Console health OK" is a process/gateway observation, never robot safety.
//! For v0.4A this owns Pico USB exclusively; do not run with imu-live yet.

use pulse_protocol::{
    system_wire::{self, Frame as SystemFrame, Snapshot},
    Kind as PulseKind, StreamDecoder as PulseDecoder,
};
use serialport::SerialPort;
use std::{
    error::Error,
    fs,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    path::Path,
    thread,
    time::{Duration, Instant},
};

const PICO_TIMEOUT: Duration = Duration::from_millis(80);
const HEALTH_TIMEOUT: Duration = Duration::from_millis(350);
const CONSOLE_ADDR: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8088));

fn parse_uptime_seconds(raw: &str) -> Result<u32, &'static str> {
    let first = raw.split_whitespace().next().ok_or("empty /proc/uptime")?;
    let seconds = first.parse::<f64>().map_err(|_| "invalid /proc/uptime")?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("invalid host uptime");
    }
    Ok(seconds.min(u32::MAX as f64) as u32)
}

fn parse_ram_used_pct(raw: &str) -> Result<u8, &'static str> {
    let mut total = None;
    let mut available = None;
    for line in raw.lines() {
        let (key, data) = match line.split_once(':') {
            Some(p) => p,
            None => continue,
        };
        if key != "MemTotal" && key != "MemAvailable" {
            continue;
        }
        let mut parts = data.split_whitespace();
        let value = parts
            .next()
            .ok_or("empty meminfo value")?
            .parse::<u64>()
            .map_err(|_| "invalid meminfo value")?;
        if parts.next() != Some("kB") || parts.next().is_some() {
            return Err("unexpected /proc/meminfo unit");
        }
        match key {
            "MemTotal" => {
                if total.replace(value).is_some() {
                    return Err("duplicate MemTotal");
                }
            }
            "MemAvailable" => {
                if available.replace(value).is_some() {
                    return Err("duplicate MemAvailable");
                }
            }
            _ => unreachable!(),
        }
    }
    let total = total.ok_or("MemTotal missing")?;
    let available = available.ok_or("MemAvailable missing")?;
    if total == 0 || available > total {
        return Err("invalid Linux RAM memory accounting");
    }
    let used = total - available;
    let percent = ((used as u128 * 100) + (total as u128 / 2)) / total as u128;
    Ok(percent.min(100) as u8)
}

fn parse_health_http(raw: &str) -> bool {
    let Some((head, body)) = raw.split_once("\r\n\r\n") else {
        return false;
    };
    let Some(first) = head.lines().next() else {
        return false;
    };
    if first != "HTTP/1.1 200 OK" && first != "HTTP/1.0 200 OK" {
        return false;
    }
    let Ok(data) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    data.get("ok").and_then(|v| v.as_bool()) == Some(true)
        && data.get("actuation_enabled").and_then(|v| v.as_bool()) == Some(false)
        && data.get("api_version").and_then(|v| v.as_str()).is_some()
}

/// Inspect ONLY a fixed local endpoint. Never retries other ports/hosts.
fn console_health_ok() -> bool {
    let Ok(mut socket) = TcpStream::connect_timeout(&CONSOLE_ADDR, HEALTH_TIMEOUT) else {
        return false;
    };
    if socket.set_read_timeout(Some(HEALTH_TIMEOUT)).is_err()
        || socket.set_write_timeout(Some(HEALTH_TIMEOUT)).is_err()
    {
        return false;
    }
    if socket.write_all(b"GET /healthz HTTP/1.1\r\nHost: 127.0.0.1:8088\r\nAccept: application/json\r\nConnection: close\r\n\r\n").is_err() {
        return false;
    }
    let mut buf = [0u8; 1024];
    let mut length = 0;
    while length < buf.len() {
        match socket.read(&mut buf[length..]) {
            Ok(0) => break,
            Ok(n) => length += n,
            Err(_) => return false,
        }
    }
    let Ok(response) = std::str::from_utf8(&buf[..length]) else {
        return false;
    };
    parse_health_http(response)
}

/// Refuse known containers: process-visible /proc is not unambiguous evidence
/// of the physical Ubuntu mini-PC host in a container namespace.
fn in_known_container() -> bool {
    Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
        || fs::read_to_string("/proc/1/cgroup")
            .map(|v| v.contains("/docker/") || v.contains("/kubepods/") || v.contains("/libpod/"))
            .unwrap_or(false)
}

/// A sample may only be called on the actual Linux host outside Docker.
/// Values are measured here; never inferred from Console robot snapshots.
fn host_snapshot(seq: u32) -> Result<Snapshot, Box<dyn Error>> {
    if seq == 0 {
        return Err("host status sequence cannot be zero".into());
    }
    if in_known_container() {
        return Err("container runtime detected: run this only on the physical Ubuntu host".into());
    }
    let uptime = parse_uptime_seconds(&fs::read_to_string("/proc/uptime")?)?;
    let ram = parse_ram_used_pct(&fs::read_to_string("/proc/meminfo")?)?;
    Ok(Snapshot {
        sequence: seq,
        host_uptime_s: uptime,
        ram_used_percent: ram,
        console_health_ok: console_health_ok(),
    })
}

/// Operator preflight: read-only host metrics, NO USB and NO IMU.
pub fn preview() -> Result<(), Box<dyn Error>> {
    let data = host_snapshot(1)?;
    println!("PULSE-0.4A HOST PREVIEW / ACTUAL LOCAL LINUX / NO USB / NO ACTUATION");
    println!(
        "host_uptime={}s ram_used={}%",
        data.host_uptime_s, data.ram_used_percent
    );
    println!(
        "console_healthz_verified={} (NOT ROBOT READY)",
        data.console_health_ok
    );
    println!("motors=N/A can=N/A drive_voltage=N/A");
    Ok(())
}

fn expect_pico_hello(port: &mut dyn SerialPort) -> Result<(), Box<dyn Error>> {
    port.write_data_terminal_ready(false)?;
    thread::sleep(Duration::from_millis(300));
    port.write_data_terminal_ready(true)?;
    let mut decoder = PulseDecoder::new();
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut buf = [0u8; 64];
    while Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(len) => {
                for byte in &buf[..len] {
                    if let Some(Ok(frame)) = decoder.push(*byte) {
                        if frame.kind == PulseKind::Hello && frame.value == 1 {
                            return Ok(());
                        }
                    }
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(e) => return Err(Box::new(e)),
        }
    }
    Err("PULSE/1 HELLO missing; SYSTEM never sends samples to old/unknown firmware".into())
}

fn require_sys_capability(port: &mut dyn SerialPort) -> Result<(), Box<dyn Error>> {
    port.write_all(&system_wire::encode(SystemFrame::Probe))?;
    let mut decoder = system_wire::Decoder::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut buf = [0u8; 64];
    while Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(len) => {
                for byte in &buf[..len] {
                    if let Some(Ok(SystemFrame::ProbeAck)) = decoder.push(*byte) {
                        return Ok(());
                    }
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(e) => return Err(Box::new(e)),
        }
    }
    Err("PULSE/SYS-1 capability ACK missing; refuse host sample transmission".into())
}

/// Exclusive Pico-only system test. This does not open IMU/CH340 serial.
/// Keep separate from imu-live until the reviewed multiplex bridge wave.
pub fn run(port_path: &str, seconds: u64) -> Result<(), Box<dyn Error>> {
    if !(1..=600).contains(&seconds) {
        return Err("duration must be 1..600 seconds".into());
    }
    const PREFIX: &str = "/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_";
    const SUFFIX: &str = "-if00";
    if !port_path.starts_with(PREFIX) || !port_path.ends_with(SUFFIX) {
        return Err("exact Pico CDC by-id required; never guess ttyACM0 or open WIT serial".into());
    }
    let id = &port_path[PREFIX.len()..port_path.len() - SUFFIX.len()];
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Pico CDC identity is malformed; no nested paths or traversal".into());
    }
    // Confirm host metrics before taking exclusive ownership of Pico USB.
    let _first = host_snapshot(1)?;
    let mut port = serialport::new(port_path, 115_200)
        .timeout(PICO_TIMEOUT)
        .open()?;
    expect_pico_hello(&mut *port)?;
    println!("PULSE/1 HELLO OK");
    require_sys_capability(&mut *port)?;
    println!("PULSE/SYS-1 PROBE/ACK OK (read-only host status)");
    println!("No WIT serial, CAN or motor device opened. imu-live MUST NOT run simultaneously.");

    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut sent = 0u32;
    while Instant::now() < deadline {
        let t0 = Instant::now();
        let sequence = sent.checked_add(1).ok_or("sequence overflow")?;
        let snapshot = host_snapshot(sequence)?;
        port.write_all(&system_wire::encode(SystemFrame::Snapshot(snapshot)))?;
        sent = sequence;
        if sent <= 3 || sent % 10 == 0 {
            println!(
                "K4 HOST #{} uptime={}s RAM={}%, Console /healthz {} (ROBOT I/O N/A)",
                sent,
                snapshot.host_uptime_s,
                snapshot.ram_used_percent,
                if snapshot.console_health_ok {
                    "OK"
                } else {
                    "N/A"
                }
            );
        }
        if let Some(wait) = (t0 + Duration::from_secs(1)).checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
    }
    println!("PULSE SYS SUMMARY: forwarded={sent} (no robot readiness or source calibration)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_uptime_and_rejects_nonsense() {
        assert_eq!(parse_uptime_seconds("12345.67 54321.00\n"), Ok(12345));
        assert!(parse_uptime_seconds("-1.0 0\n").is_err());
        assert!(parse_uptime_seconds("NaN 0\n").is_err());
        assert!(parse_uptime_seconds("garbage").is_err());
    }

    #[test]
    fn calculates_available_based_real_ram_without_overpromising() {
        let data = "MemTotal:       16384000 kB\nMemFree: 100 kB\nMemAvailable:   12288000 kB\n";
        assert_eq!(parse_ram_used_pct(data), Ok(25));
        assert!(parse_ram_used_pct("MemTotal: 2000 kB\nMemAvailable: 3000 kB").is_err());
        assert!(parse_ram_used_pct("MemTotal: 2000 kB\n").is_err());
        assert!(parse_ram_used_pct("MemTotal: 0 kB\nMemAvailable: 0 kB").is_err());
    }

    #[test]
    fn console_health_has_strict_local_json_identity() {
        let good = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true,\"api_version\":\"console.v1\",\"actuation_enabled\":false}";
        assert!(parse_health_http(good));
        assert!(!parse_health_http(
            &good.replace("200 OK", "503 Unavailable")
        ));
        assert!(!parse_health_http(
            &good.replace("\"ok\":true", "\"ok\":false")
        ));
        assert!(!parse_health_http(&good.replace(
            "\"actuation_enabled\":false",
            "\"actuation_enabled\":true"
        )));
        assert!(!parse_health_http("HTTP/1.1 200 OK\r\n\r\n<html>ok</html>"));
    }
}
