use pulse_core::Observer;
use pulse_protocol::{decode, encode, Frame, Kind, StreamDecoder};
use std::{
    env,
    fmt::Write as _,
    io::{Read, Write},
    process, thread,
    time::{Duration, Instant},
};

const PROTOTYPE_BOARD_ID: u32 = 1;

fn usage() -> ! {
    eprintln!(
        "usage:\n  pulse-bridge demo [count]\n  pulse-bridge serial-demo <explicit-port-path> [count]"
    );
    process::exit(2);
}

fn parse_count(s: Option<&String>, default: u32) -> u32 {
    let count: u32 = match s {
        Some(n) => n.parse().unwrap_or_else(|_| {
            eprintln!("count must be an integer");
            process::exit(2)
        }),
        None => default,
    };
    if count > 1000 {
        eprintln!("count must be <= 1000");
        process::exit(2);
    }
    count
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("demo") => {
            if args.len() > 2 {
                usage();
            }
            run_offline_demo(parse_count(args.get(1), 8));
            Ok(())
        }
        Some("serial-demo") => {
            if args.len() < 2 || args.len() > 3 {
                usage();
            }
            serial_demo(&args[1], parse_count(args.get(2), 20))
        }
        _ => usage(),
    };
    if let Err(error) = result {
        eprintln!("Pulse error: {error}");
        process::exit(1);
    }
}

fn run_offline_demo(count: u32) {
    println!("PULSE-0.1A SYNTHETIC / NO_REAL_ROBOT / NO_USB");
    let mut observer = Observer::new(300);
    for i in 0..count {
        let timestamp = i * 100;
        // Synthetic Somatic counter; there are no physical measurements.
        let frame = Frame {
            kind: Kind::LoopTicks,
            sequence: i + 1,
            uptime_ms: timestamp,
            value: (i + 1) * 100,
        };
        let bytes = encode(frame);
        let received = decode(&bytes).expect("our own encoded frame must decode");
        observer.observe(received, timestamp as u64).unwrap();
        let mut hex = String::new();
        for b in bytes {
            write!(&mut hex, "{b:02X}").unwrap();
        }
        println!(
            "seq={:03} ticks={:05} link={:?} loop={:?} frame={hex}",
            frame.sequence,
            frame.value,
            observer.link_state(timestamp as u64),
            observer.loop_state(timestamp as u64)
        );
    }
    let after = (count as u64).saturating_mul(100) + 301;
    println!(
        "DISCONNECT simulation: link={:?} loop={:?} (time={after}ms)",
        observer.link_state(after),
        observer.loop_state(after)
    );
}

/// Bench-only USB CDC test. Requires an *explicit* port, never enumerates
/// and guesses among STM32 actuators / other serial adapters.
fn serial_demo(path: &str, count: u32) -> Result<(), Box<dyn std::error::Error>> {
    println!("PULSE-0.1C SYNTHETIC / NO_REAL_ROBOT / NO_MOTION");
    println!("Opening requested USB CDC port: {path}");

    let mut port = serialport::new(path, 115_200)
        .timeout(Duration::from_millis(100))
        .open()?;
    // Embassy's CdcAcmClass::wait_connection requires DTR asserted.
    port.write_data_terminal_ready(true)?;

    let handshake_deadline = Instant::now() + Duration::from_secs(6);
    let mut decoder = StreamDecoder::new();
    let mut buf = [0u8; 64];
    let mut handshake_seen = false;

    while Instant::now() < handshake_deadline {
        match port.read(&mut buf) {
            Ok(len) => {
                for byte in buf[..len].iter().copied() {
                    if let Some(Ok(frame)) = decoder.push(byte) {
                        if frame.kind == Kind::Hello && frame.value == PROTOTYPE_BOARD_ID {
                            println!("Pico HELLO OK (protocol=1, board_id=1)");
                            handshake_seen = true;
                            break;
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => return Err(Box::new(error)),
        }
        if handshake_seen {
            break;
        }
    }
    if !handshake_seen {
        return Err("Pico HELLO not received. No demo frames were transmitted: verify the exact CDC device path, DTR and firmware".into());
    }

    println!("Sending {count} explicit DEMO frames, at up to 10Hz");
    println!("Expecting one CRC-checked RX DemoAck for every accepted DEMO frame.");
    let mut confirmed: u32 = 0;
    let mut unexpected: u32 = 0;
    for i in 0..count {
        let sent_at = Instant::now();
        let frame = Frame {
            kind: Kind::DemoTicks,
            sequence: i + 1,
            uptime_ms: i * 100,
            value: (i + 1) * 100,
        };
        port.write_all(&encode(frame))?;
        println!(
            "TX DEMO seq={} synthetic_ticks={}",
            frame.sequence, frame.value
        );

        let deadline = Instant::now() + Duration::from_millis(450);
        let mut acknowledged = false;
        while Instant::now() < deadline {
            match port.read(&mut buf) {
                Ok(len) => {
                    for byte in buf[..len].iter().copied() {
                        if let Some(result) = decoder.push(byte) {
                            match result {
                                Ok(receipt)
                                    if receipt.kind == Kind::DemoAck
                                        && receipt.sequence == frame.sequence
                                        && receipt.value == frame.value =>
                                {
                                    acknowledged = true;
                                    confirmed += 1;
                                    println!(
                                        "RX ACK seq={} validated_ticks={}",
                                        receipt.sequence, receipt.value
                                    );
                                }
                                Ok(_) | Err(_) => {
                                    unexpected += 1;
                                }
                            }
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
                Err(error) => return Err(Box::new(error)),
            }
            if acknowledged {
                break;
            }
        }

        if !acknowledged {
            eprintln!(
                "NO ACK for DEMO seq={} (check Pico receive path / USB hub)",
                frame.sequence
            );
        }

        // Cap the send rate at 10Hz; a delayed ACK slows transmission rather
        // than overflowing the Pico's small CDC endpoint buffers.
        if let Some(wait) =
            (sent_at + Duration::from_millis(100)).checked_duration_since(Instant::now())
        {
            thread::sleep(wait);
        }
    }
    println!("PULSE ACK SUMMARY: {confirmed}/{count} confirmed, {unexpected} unexpected frames");
    println!("Do not interpret demo values as Somatic loop performance.");
    if confirmed != count {
        return Err(format!(
            "Incomplete USB delivery: {confirmed}/{count} DEMO frames acknowledged by Pico"
        )
        .into());
    }
    println!("PULSE-0.1C USB CDC ROUND TRIP VERIFIED for synthetic DEMO frames.");
    Ok(())
}
