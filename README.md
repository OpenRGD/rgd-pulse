# OpenRGD Pulse & Chronoscope

**PULSE-0.2D — joystick acceleration + dashboard skin candidate.** An independent, non-actuating diagnostics node for OpenRGD robots.

This repository separates the RP2040/Pico W device firmware from host-side telemetry and hardware-specific robot inventories. **It is not a safety controller, an E-stop, or a motor driver.**

## Architecture

```text
Somatic Runtime / sensor / Chronon sources (future read-only adapters)
                                     |
                              pulse-bridge (Ubuntu)
                                     | USB CDC PULSE/1: HELLO / DEMO / ACK verified
                              Pico W (non-actuating)
                                     |
                    4x Chronoscope dashboards + skin switcher
                                     |
                     Seengreat ST7735S 128x128 SPI0 LCD
```

* `crates/pulse-protocol`: `no_std` fixed-size binary telemetry frame v1, CRC-16/CCITT-FALSE.
* `crates/pulse-core`: host-side link and loop-progress observer with tests.
* `crates/pulse-bridge`: synthetic offline demo and **candidate** USB CDC serial-demo with explicit device-path and HELLO handshake. No real Somatic Runtime integration.
* `crates/pulse-ui`: `no_std` four-screen navigation and focus/zoom, physically accepted joystick GPIO map, accelerated hold repeat, local K2+K3 Settings and stable visual skin manifests, all host-tested.
* `firmware/pico-w`: Embassy Rust, SPI0 ST7735S landscape display, four DEMO dashboards and USB CDC ACK-compatible transport.
* `hardware/boards/`: verified vendor pin mappings, revision and limitations.

## Host-side quick start

```sh
cargo test --workspace
cargo run -p pulse-bridge -- demo 8
```

`demo` prints **SYNTHETIC / NO_REAL_ROBOT**. Its loop counters, timing and CRC frames are fixtures only.

## Pico W bring-up

See [firmware/pico-w/README.md](firmware/pico-w/README.md), [PULSE-0.2C accepted hardware input/UI/20-ACK evidence](docs/PULSE-0.2C-INPUT-LAB-BENCH.md), and [PULSE-0.2D skin/navigation bench plan](docs/PULSE-0.2D-SKINS-NAVIGATION.md). Earlier firmware passed 100/100 synthetic ACK and 3/3 DTR reopen; **0.2C** passed physical joystick/pages/zoom and 20/20 synthetic ACK. **0.2D skins and hold-to-repeat remain unverified on real hardware.** Power the Pico only via Micro-USB, never Paky-01 motors/12–24V.

## Repository authority

* Canonical schema/contract authority: [OpenRGD/openrgd](https://github.com/OpenRGD/openrgd). Nothing in this repo silently updates it.
* Instance-specific hardware and evidence: [italiarobotica/paky-01](https://github.com/italiarobotica/paky-01). Physical configuration must not be invented.
* The [module manifest](module.openrgd.json) is **proposal-only** and does not claim the Module Registry is operational.
* Synthesized telemetry is never represented as measured loop performance.
* The Pico is informational only; no motor control, calibration writing, actuator flashing or safety override.

## USB CDC prototype (synthetic hardware-verified)

```sh
# Explicit port path, verified by udevadm. Never guess ttyACM0 on Paky-01.
cargo run -p pulse-bridge -- serial-demo /dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00 20
```

Only explicitly synthetic `DemoTicks` are sent; no real motor commands or actual Somatic loop readings.

## Chronoscope skins and navigation candidate

The HMI design is documented in [PULSE-0.2A UI SPEC](docs/CHRONOSCOPE-0.2A-UI-SPEC.md). The four pages K1 SOMATIC / K2 CHRONONS / K3 IMU / K4 SYSTEM have been exercised on Paky-01 with the joystick LEFT and four buttons RIGHT. In 0.2D, holding joystick directions repeats with acceleration and the new **K2+K3 SETTINGS → SKINS** menu previews four original visual styles (Classic, Phosphor, Chrome 99, Italia); CLICK applies for **this power-on session only**. See [0.2D bench/design](docs/PULSE-0.2D-SKINS-NAVIGATION.md). Graphs remain explicitly DEMO fixtures, not measured Somatic/Chronon/IMU data; no RP2040 flash preference writes.

## Planned waves

PULSE-0.1A protocol, 0.1B LCD/buttons, 0.1C synthetic USB CDC, 0.2A four-page design, 0.2B graphical firmware, 0.2C physically verified input map, **0.2D joystick repeat and skins (candidate)**. Next: persistent per-device skin preferences, then read-only real Somatic/IMU/Chronon adapters. Canonical contract promotion and module registration require separate reviews.

See [docs/ROADMAP.md](docs/ROADMAP.md) and [hardware/boards/seengreat-pico-144-rev12.md](hardware/boards/seengreat-pico-144-rev12.md).
