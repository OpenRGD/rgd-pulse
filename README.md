# OpenRGD Pulse & Chronoscope

**PULSE-0.1A — foundation candidate.** An independent, non-actuating diagnostics node for OpenRGD robots.

This repository separates the RP2040/Pico W device firmware from host-side telemetry and hardware-specific robot inventories. **It is not a safety controller, an E-stop, or a motor driver.**

## Architecture

```text
Somatic Runtime (out of scope) ---> host telemetry adapter (future)
                                     |
                              pulse-bridge (host)
                                     | versioned, CRC-checked frames (future USB CDC)
                              Pico W (non-actuating)
                                     |
                              Seengreat ST7735S LCD
```

* `crates/pulse-protocol`: `no_std` fixed-size binary telemetry frame v1, CRC-16/CCITT-FALSE.
* `crates/pulse-core`: host-side link and loop-progress observer with tests.
* `crates/pulse-bridge`: synthetic offline demo and **candidate** USB CDC serial-demo with explicit device-path and HELLO handshake. No real Somatic Runtime integration.
* `firmware/pico-w`: separate Embassy Rust workspace for the Seengreat 1.44-inch Rev 1.2 LCD, keys and candidate USB CDC.
* `hardware/boards/`: verified vendor pin mappings, revision and limitations.

## Host-side quick start

```sh
cargo test --workspace
cargo run -p pulse-bridge -- demo 8
```

`demo` prints **SYNTHETIC / NO_REAL_ROBOT**. Its loop counters, timing and CRC frames are fixtures only.

## Pico W bring-up

See [firmware/pico-w/README.md](firmware/pico-w/README.md) and [PULSE-0.1C USB bench guide](docs/PULSE-0.1C-USB-BENCH.md). All nine inputs and the LCD startup were operator-verified on 2026-10-24; CDC operation still requires physical testing. Power only by **Pico Micro-USB**, disconnected from Paky-01 power/motors.

## Repository authority

* Canonical schema/contract authority: [OpenRGD/openrgd](https://github.com/OpenRGD/openrgd). Nothing in this repo silently updates it.
* Instance-specific hardware and evidence: [italiarobotica/paky-01](https://github.com/italiarobotica/paky-01). Physical configuration must not be invented.
* The [module manifest](module.openrgd.json) is **proposal-only** and does not claim the Module Registry is operational.
* Synthesized telemetry is never represented as measured loop performance.
* The Pico is informational only; no motor control, calibration writing, actuator flashing or safety override.

## USB CDC prototype (candidate)\n\n```sh\n# Explicit port path, verified by udevadm. Never guess ttyACM0 on Paky-01.\ncargo run -p pulse-bridge -- serial-demo /dev/serial/by-path/YOUR_PICO_PATH 20\n```\n\nOnly frames marked `DemoTicks` are transmitted, never real motor commands.\n\n## Chronoscope UI candidate (design only)

The four-screen hardware HMI proposal (K1 SOMATIC / K2 CHRONONS / K3 IMU / K4 SYSTEM), joystick gestures, compact charts and data provenance policy are specified in [PULSE-0.2A UI SPEC](docs/CHRONOSCOPE-0.2A-UI-SPEC.md). This is **not implemented**, does not change canonical contracts and does not turn synthetic telemetry into real measurements.

## Planned waves

PULSE-0.1A protocol + synthetic host; PULSE-0.1B LCD/buttons hardware bring-up; PULSE-0.1C USB CDC bridge; PULSE-0.2 Chronoscope timeline. Contract promotion and canonical registration need separate review and PRs.

See [docs/ROADMAP.md](docs/ROADMAP.md) and [hardware/boards/seengreat-pico-144-rev12.md](hardware/boards/seengreat-pico-144-rev12.md).
