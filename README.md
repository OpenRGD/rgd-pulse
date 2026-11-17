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
* `crates/pulse-ui`: `no_std` four-screen navigation, joystick focus/zoom, raw-to-landscape input mapping and host unit tests.
* `firmware/pico-w`: Embassy Rust, SPI0 ST7735S landscape display, four DEMO dashboards and USB CDC ACK-compatible transport.
* `hardware/boards/`: verified vendor pin mappings, revision and limitations.

## Host-side quick start

```sh
cargo test --workspace
cargo run -p pulse-bridge -- demo 8
```

`demo` prints **SYNTHETIC / NO_REAL_ROBOT**. Its loop counters, timing and CRC frames are fixtures only.

## Pico W bring-up

See [firmware/pico-w/README.md](firmware/pico-w/README.md), [PULSE-0.1C USB hardware evidence](docs/PULSE-0.1C-USB-BENCH.md) and [PULSE-0.2B four-screen physical acceptance plan](docs/CHRONOSCOPE-0.2B-HARDWARE-BENCH.md). Previous firmware proved USB synthetic 100/100, DTR reopen 3/3 and button inputs; **the new 0.2B landscape firmware remains unverified on real hardware**. Power only by **Pico Micro-USB**, never Paky-01 power/motors.

## Repository authority

* Canonical schema/contract authority: [OpenRGD/openrgd](https://github.com/OpenRGD/openrgd). Nothing in this repo silently updates it.
* Instance-specific hardware and evidence: [italiarobotica/paky-01](https://github.com/italiarobotica/paky-01). Physical configuration must not be invented.
* The [module manifest](module.openrgd.json) is **proposal-only** and does not claim the Module Registry is operational.
* Synthesized telemetry is never represented as measured loop performance.
* The Pico is informational only; no motor control, calibration writing, actuator flashing or safety override.

## USB CDC prototype (candidate)\n\n```sh\n# Explicit port path, verified by udevadm. Never guess ttyACM0 on Paky-01.\ncargo run -p pulse-bridge -- serial-demo /dev/serial/by-path/YOUR_PICO_PATH 20\n```\n\nOnly frames marked `DemoTicks` are transmitted, never real motor commands.\n\n## Chronoscope UI candidate (design only)

The four-screen HMI design is in [PULSE-0.2A UI SPEC](docs/CHRONOSCOPE-0.2A-UI-SPEC.md). The stacked 0.2B candidate now implements K1 SOMATIC / K2 CHRONONS / K3 IMU / K4 SYSTEM, **joystick LEFT / K buttons RIGHT**, bounded graph focus/zoom, 128×128 framebuffer rendering and USB-first boot. **These pages are fixtures**, not real Somatic/Chronograph/IMU/motor telemetry. Follow [the 0.2B physical bench plan](docs/CHRONOSCOPE-0.2B-HARDWARE-BENCH.md) before accepting the new firmware.

## Planned waves

PULSE-0.1A protocol + synthetic host; PULSE-0.1B LCD/buttons hardware bring-up; PULSE-0.1C USB CDC bridge; PULSE-0.2 Chronoscope timeline. Contract promotion and canonical registration need separate review and PRs.

See [docs/ROADMAP.md](docs/ROADMAP.md) and [hardware/boards/seengreat-pico-144-rev12.md](hardware/boards/seengreat-pico-144-rev12.md).
