# OpenRGD Pulse & Chronoscope

**PULSE-0.3C — physically accepted observation-only Chronoscope (Paky-01, 2026-10-27).** Features and hardware baseline are frozen; see [release acceptance and freeze](docs/RELEASE-PULSE-0.3C.md). K3 displays real, uncalibrated IMU sensor-frame data, and no screen controls robot motion.

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
* `crates/pulse-bridge`: synthetic demo/USB CDC regression and verified same-host Console IMU read-only WS→Pico bridge. No real Somatic Runtime integration.
* `crates/pulse-ui`: `no_std` four-screen navigation and focus/zoom, physically accepted joystick GPIO map, accelerated hold repeat, local K2+K3 Settings and stable visual skin manifests, all host-tested.
* `firmware/pico-w`: Embassy Rust, SPI0 ST7735S landscape 128x128 display; real K3 IMU RAW/STALE sensor readout plus K1/K2/K4 DEMO or unknown, versioned USB CDC, 8KiB preference journal.
* `hardware/boards/`: verified vendor pin mappings, revision and limitations.

## Host-side quick start

```sh
cargo test --workspace
cargo run -p pulse-bridge -- demo 8
```

`demo` prints **SYNTHETIC / NO_REAL_ROBOT**. Its loop counters, timing and CRC frames are fixtures only.

## Pico W bring-up

See [firmware/pico-w/README.md](firmware/pico-w/README.md), [PULSE-0.2C accepted hardware input/UI/20-ACK evidence](docs/PULSE-0.2C-INPUT-LAB-BENCH.md), and [PULSE-0.2D skin/navigation bench plan](docs/PULSE-0.2D-SKINS-NAVIGATION.md). The combined 0.3C build passed operator hardware checks for inputs/pages/skins, color mode, X=1/Y=2 LCD alignment, persisted Italia/K3, 100/100 synthetic USB ACK, and real K3 IMU/STALE. Power the Pico only via Micro-USB, never Paky-01 motors/12–24V.

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

## Chronoscope skins and navigation (0.3C hardware-verified)

The HMI design is documented in [PULSE-0.2A UI SPEC](docs/CHRONOSCOPE-0.2A-UI-SPEC.md). The four pages K1 SOMATIC / K2 CHRONONS / K3 IMU / K4 SYSTEM have been exercised on Paky-01 with the joystick LEFT and four buttons RIGHT. Holding joystick directions repeats with acceleration, and **K2+K3 SETTINGS → SKINS** previews Classic, Phosphor, Chrome 99, and Italia. CLICK applies the preview. As of 0.3C, the applied skin and last K1–K4 page are persisted in a CRC16 flash journal (not every preview; no writes during USB DTR). See [the skin design](docs/PULSE-0.2D-SKINS-NAVIGATION.md) and [accepted 0.3C release](docs/RELEASE-PULSE-0.3C.md). Charts remain clearly DEMO unless linked to a real historical source: the live K3 numeric IMU sensor observations are physical, not historical chart samples.

## PULSE-0.3B: real IMU from the existing Console subscriber (physical PASS)

The real Paky-01 WIT/JY901 sensor already has an exclusive Python reader. **Do not open it again from Pulse**. The existing Console exposes `ws://127.0.0.1:8088/api/v1/stream?robot_id=paky-01`; subscribe to `console.v1 type=imu` with `console.imu.v0.1-candidate` data. The publisher POST and token are **not** used.

`pulse-bridge imu-live` validates the read-side schema, physical source identity, session/sequence, time quality and uncalibrated sensor axes; forwards only a bounded <=10 Hz stream over the separate `PULSE/IMU-1` CRC-protected protocol, with an explicit per-session Pico capability handshake. It never reuses `DemoTicks` for real readings.

K3 displays RAW sensor-frame roll, pitch, `YAW* UNCAL`, gyro/acceleration selected X/Y/Z, temperature, age and `STALE` after 800ms. All historical trend charts still explicitly say DEMO; Somatic/Chronons/System remain unchanged. Serial demos and the fixed ST7735S 0x68/INVOFF color profile must be regression tested.

```bash
cd ~/openrgd/rgd-pulse
PICO_PORT="/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00"
cargo run -p pulse-bridge -- imu-live "$PICO_PORT" 30
```

**Physical commissioning PASS (2026-10-27):** Console WS, PULSE/1 HELLO, PULSE/IMU-1 PROBE/ACK, numbered real samples on K3, and greyscale STALE after publisher stop. See [recorded IMU live bench](docs/PULSE-0.3B-CONSOLE-IMU-LIVE-BENCH.md). The Console's local WS uses **loopback restriction, not user authentication**; do not expose :8088 to the LAN. No motor actuation, calibration or certified robot orientation is implemented.

## Planned waves

PULSE-0.1A through 0.3C have been consolidated to main; the source baseline is frozen on `release/pulse-0.3c-physical-20261027`. Next separately reviewed waves: read-only K4 SYSTEM host/peripheral health, then K1 SOMATIC and K2 CHRONONS when real source adapters exist. Canonical contract promotion and module registration remain separate reviews.

See [docs/ROADMAP.md](docs/ROADMAP.md) and [hardware/boards/seengreat-pico-144-rev12.md](hardware/boards/seengreat-pico-144-rev12.md).

## PULSE-0.3C: persisted skin and last K page (physical PASS)

The Chronoscope can now **restore the applied skin and last K1–K4 page from an append-only two-sector flash journal** on a standard 2MiB RP2040 Pico W. This is deliberately separate from live IMU telemetry. Records are 256 bytes with CRC; malformed/torn records fall back to the most recent confirmed record or Classic/K1. The validated color mode (NORM/BGR) remains the constant boot default, not a saved theme selection. A write is deferred until >=3s idle and >=15s after a previous write, and is suspended while USB CDC DTR is active. Preferences changed immediately before unplugging may not be saved.

**Physical hardware PASS:** operator restored Italia + K3 IMU after full power removal and after the final update; the LCD boots perfectly aligned at X=1/Y=2 and with validated colors. USB CDC recorded 100/100 CRC ACK and zero unexpected frames, plus the live IMU/STALE behavior. See [release evidence](docs/RELEASE-PULSE-0.3C.md), [0.3C persistence bench](docs/PULSE-0.3C-PERSISTENT-PREFERENCES-BENCH.md) and `scripts/check-pulse-uf2-prefs.py` before any reflash. This uses the last 8KiB of the verified 2MiB RP2040 flash; never use on an unknown board/flash chip, RP2350 or a mass-erase recovery routine if preferences must be preserved.
