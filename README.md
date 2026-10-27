# OpenRGD Pulse & Chronoscope

**PULSE-0.3B — physical IMU via Console read-only fan-out (software candidate).** Independent, non-actuating diagnostics node for OpenRGD robots; K3 IMU hardware integration is not yet physically accepted.

This repository separates the RP2040/Pico W device firmware from host-side telemetry and hardware-specific robot inventories. **It is not a safety controller, an E-stop, or a motor driver.**

## Architecture

```text
Paky-01 IMU / 1 existing exclusive WIT reader
          |
    imu_publish.py / accepted physical observations
          |
  OpenRGD Console localhost:8088  (NOT user-authenticated WS)
          |  console.v1 type=imu / console.imu.v0.1-candidate
     pulse-bridge imu-live (strict source validation)
          |  USB CDC PULSE/IMU-1 Probe/ACK + 56-byte sensor frames
          |  Existing PULSE/1 DEMO/ACK remains independent
       Pico W / K3 RAW SENSOR XYZ, age / STALE, YAW* UNCAL
          |
  4 Chronoscope pages / skins / Color Lab ST7735S 0x68+INVOFF
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

## PULSE-0.3B: real IMU from the existing Console subscriber (candidate)

The real Paky-01 WIT/JY901 sensor already has an exclusive Python reader. **Do not open it again from Pulse**. The existing Console exposes `ws://127.0.0.1:8088/api/v1/stream?robot_id=paky-01`; subscribe to `console.v1 type=imu` with `console.imu.v0.1-candidate` data. The publisher POST and token are **not** used.

`pulse-bridge imu-live` validates the read-side schema, physical source identity, session/sequence, time quality and uncalibrated sensor axes; forwards only a bounded <=10 Hz stream over the separate `PULSE/IMU-1` CRC-protected protocol, with an explicit per-session Pico capability handshake. It never reuses `DemoTicks` for real readings.

K3 displays RAW sensor-frame roll, pitch, `YAW* UNCAL`, gyro/acceleration selected X/Y/Z, temperature, age and `STALE` after 800ms. All historical trend charts still explicitly say DEMO; Somatic/Chronons/System remain unchanged. Serial demos and the fixed ST7735S 0x68/INVOFF color profile must be regression tested.

```bash
cd ~/openrgd/rgd-pulse
PICO_PORT="/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00"
cargo run -p pulse-bridge -- imu-live "$PICO_PORT" 30
```

**This feature has not yet passed the mini-PC↔Pico physical IMU commissioning.** See the complete safe flash and bench plan in [PULSE-0.3B Console IMU Live Bench](docs/PULSE-0.3B-CONSOLE-IMU-LIVE-BENCH.md). The Console's local WS uses **loopback restriction, not user authentication**; do not expose :8088 to the LAN. No motor actuation, calibration or certified robot orientation is implemented.

## Planned waves

PULSE-0.1A protocol, 0.1B LCD/buttons, 0.1C synthetic USB CDC, 0.2A four-page design, 0.2B graphical firmware, 0.2C physically verified input map, **0.2D joystick repeat and skins (candidate)**. **0.3B IMU Console read-only adapter (software candidate)** is the next wave; actual Somatic/Chronon adapters and persistent skin preferences remain separate. Canonical contract promotion and module registration require separate reviews.

See [docs/ROADMAP.md](docs/ROADMAP.md) and [hardware/boards/seengreat-pico-144-rev12.md](hardware/boards/seengreat-pico-144-rev12.md).

## PULSE-0.3C: persistent skin and last page (candidate)

The Chronoscope can now **restore the applied skin and last K1–K4 page from an append-only two-sector flash journal** on a standard 2MiB RP2040 Pico W. This is deliberately separate from live IMU telemetry. Records are 256 bytes with CRC; malformed/torn records fall back to the most recent confirmed record or Classic/K1. The validated color mode (NORM/BGR) remains the constant boot default, not a saved theme selection. A write is deferred until >=3s idle and >=15s after a previous write, and is suspended while USB CDC DTR is active. Preferences changed immediately before unplugging may not be saved.

**Physical preferences PASS:** operator restored Italia + K3 IMU after full power removal. **LCD alignment physical PASS in LCD ALIGN** with X=1/Y=2 (27 Oct 2026); these values are now the firmware defaults. The updated boot default still needs a cold-boot check after reflashing, as do USB 20/20 and 100/100 ACK regressions. See [PULSE-0.3C persistence bench](docs/PULSE-0.3C-PERSISTENT-PREFERENCES-BENCH.md) and `scripts/check-pulse-uf2-prefs.py` before flashing. This uses the last 8KiB of the verified 2MiB RP2040 flash; never use on an unknown board/flash chip, RP2350 or a mass-erase recovery routine if preferences must be preserved.
