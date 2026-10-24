# Pico W + Seengreat ST7735S — Chronoscope firmware

**Current candidate PULSE-0.2B (180° landscape correction)** — read-only four-page observer. The first 0.2B firmware was physically flashed on 2026-10-24: OPENRGD splash and dashboard graphics appeared and K1–K4 worked, **but the screen content was upside-down**. A corrected orientation is now committed, awaiting another physical test. Earlier 0.1C USB synthetic ACK/DTR results remain separate and must be re-run on this version.

## Hardware

- Raspberry Pi Pico W (RP2040), Seengreat PICO-1.44inch-LCD-Display Rev 1.2.
- 128×128 square color TFT using **SPI0 hardware at 8MHz** (GP6 SCK, GP7 MOSI), ST7735S, GP5 CS / GP11 D-C / GP12 RESET / GP13 BL.
- The assembled PCB is physically oriented **90° clockwise from vendor portrait**, so the **joystick is left and K1..K4 are right**, vertically stacked.
- Initial MADCTL=0xA0 (MY|MV) was **physically observed to draw upside-down** in the joystick-left/buttons-right pose. Updated candidate **MADCTL=0x60 (MX|MV)** flips the graphic content by 180° without changing input GPIO mappings. This correction, RGB/BGR order and pixel offsets still need physical confirmation.
- Nine active-low buttons, already proven individually in earlier 0.1B hardware work.

## Navigation

| Physical input | Behavior |
|---|---|
| K1 | SOMATIC, shows actual-loop **unavailable** until adapter exists |
| K2 | CHRONONS, DEMO-only timeline, joystick click opens selected event |
| K3 | IMU, uncalibrated SENSOR-frame readout unavailable, DEMO-only chart |
| K4 | SYSTEM, BHL-reference **22 CFG** plus unknown observed motors, CAN and voltage |
| Joystick UP/DOWN | Browse an item/metric |
| Joystick LEFT/RIGHT | Change history window / filter / section |
| Joystick CLICK | Focus/detail ON or OFF |
| Chart focused, UP/DOWN | Zoom x1→x2→x4→x8 (or reverse) |
| Chart focused, LEFT/RIGHT | Pan through graph history |

**No joystick or K-key action is a robot motion command.**

## INPUT LAB (PULSE-0.2C diagnostic candidate)

If joystick actions are visually ambiguous, **hold K1 + K4 for about 1.2 seconds** to enter a *local, read-only* GPIO diagnostic. Release the keys. Move each joystick direction once; the screen shows **RAW PIN GPnn** and its current **FW MAP** interpretation side by side, plus held/released status and a press counter. Tap each K key individually to confirm its physical GPIO. Hold K1+K4 again to exit. Normal K1–K4/joystick page navigation is suppressed while the lab is open. Only after verifying raw pins should the software direction mapping be corrected.

See [PULSE-0.2C INPUT LAB BENCH](../../docs/PULSE-0.2C-INPUT-LAB-BENCH.md) for a reproducible five-direction test and a concise report format. This mode sends **no hardware, CAN or actuator commands**, and has no remote USB control endpoints.

## Compile and flash

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.2c-input-lab
git pull --ff-only
cargo test --workspace

cd firmware/pico-w
rustup target add thumbv6m-none-eabi
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Power **only** from the Pico's Micro-USB port via a real data hub. If flashing, hold the **Pico BOOTSEL**, connect Micro-USB and then release BOOTSEL. Explicitly verify the **RPI-RP2** mount; another STMicroelectronics device can appear as a disk on this mini-PC. See [the physical bench plan](../../docs/CHRONOSCOPE-0.2B-HARDWARE-BENCH.md) for cautious copying, confirmation and regression tests.

## Rendering and communication

- The frame renderer uses a bounded 32KiB RGB565 framebuffer and `embedded-graphics` typography, charts and cards.
- USB stack is spawned before LCD init, and TFT reset/sleep delays are monotonic async timers; no CPU-spin boot animations.
- Redraw on page, selection, zoom/focus or 10 synthetic DemoAck frames. Rendering uses 8MHz hardware SPI instead of the original bit-banged driver.
- USB CDC session resets on DTR falling. It emits a v1 HELLO when rearmed and ACKs only newly accepted valid `DemoTicks` frames. All frames and charts remain explicitly synthetic.
- The 5-inch main Console remains the rich host interface. The Pico will later receive bounded, read-only summaries through versioned telemetry UI messages; it does **not** fetch motors, CAN, IMU serial or Chronons by itself.

## Still unverified on physical 0.2B firmware

- Physical 0.2B first flash confirmed splash, graphics and K1–K4; **180° corrected display orientation (0x60)**, RGB/BGR order, X/Y offsets, clipping and logical joystick direction mapping still need validation.
- Actual redraw latency and joysticks during simultaneous USB traffic.
- 3/3 repeated serial opens, 100/100 ACK and cable disconnect/reconnect regressions.
- Real Somatic loop measurement, Chronograph provenance/Chronon data, calibrated IMU, full joint mapping and measured voltage.

The Pico is not an E-stop, safety supervisor, balance sensor or actuator controller. CRC detects accidental transmission corruption but does not authenticate devices.
