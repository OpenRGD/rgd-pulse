use std::{env, fmt::Write as _, process};
use pulse_core::Observer;
use pulse_protocol::{decode, encode, Frame, Kind};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("demo") {
        eprintln!("usage: cargo run -p pulse-bridge -- demo [count]");
        process::exit(2);
    }
    let count: u32 = match args.get(1) {
        Some(n) => n.parse().unwrap_or_else(|_| {
            eprintln!("count must be an integer");
            process::exit(2)
        }),
        None => 8,
    };
    if count > 1000 {
        eprintln!("count must be <= 1000");
        process::exit(2);
    }

    println!("PULSE-0.1A SYNTHETIC / NO_REAL_ROBOT / NO_USB");
    let mut observer = Observer::new(300);
    for i in 0..count {
        let timestamp = i * 100;
        // Synthetic Somatic counter: advances at 100 ticks per 100 ms; not measured.
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
        println!("seq={:03} ticks={:05} link={:?} loop={:?} frame={hex}",
            frame.sequence, frame.value,
            observer.link_state(timestamp as u64),
            observer.loop_state(timestamp as u64));
    }
    let after = (count as u64).saturating_mul(100) + 301;
    println!("DISCONNECT simulation: link={:?} loop={:?} (time={after}ms)",
        observer.link_state(after), observer.loop_state(after));
}
