# Pico W + Seengreat ST7735S — Chronoscope firmware

**Current candidate PULSE-0.2D.3 — calibrated ST7735S color profile.** Color Lab's mode4 was **physically confirmed** on 2026-10-24: NORM / BGR shows correct green, white and red. This branch now uses mode4 as the **compiled boot default**. The new automatic startup has not yet been re-flashed and independently verified; historical joystick/UI and synthetic USB tests remain separate evidence.

## Hardware

- Raspberry Pi Pico W (RP2040), Seengreat PICO-1.44inch-LCD-Display Rev 1.2.
- 128×128 square color TFT using **SPI0 hardware at 8MHz** (GP6 SCK, GP7 MOSI), ST7735S, GP5 CS / GP11 D-C / GP12 RESET / GP13 BL.
- The assembled PCB is physically oriented **90° clockwise from vendor portrait**, so the **joystick is left and K1..K4 are right**, vertically stacked.
- Panel rotation **MX|MV** remains unchanged from the visually accepted operator orientation (joystick LEFT, K1–K4 RIGHT). The Color Lab isolated **MADCTL 0x68** (adds BGR bit) + **INVOFF 0x20** as the physical G/W/R calibration PASS. That combination is now applied by `lcd.init()` before the OPENRGD splash; only precise panel X/Y offsets and final reboot regression remain to verify.
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

**Physical functional acceptance PASS (2026-10-24, operator report):** joystick UP=GP18, DOWN=GP20, LEFT=GP16, RIGHT=GP19, CLICK=GP17, with K1–K4 correct. After reflashing the corrected `pulse_ui::landscape_action`, the operator confirmed all five inputs in Input Lab **and** normal four-screen graphical navigation, Chronon detail, Somatic focus/zoom and a 20/20 synthetic USB CDC ACK regression. GP17 CLICK and GP18 UP differ from the initial vendor-oriented hypothesis; do not regress to that earlier mapping.

To recheck the mapping, **hold K1 + K4 for about 1.2 seconds** to enter a *local, read-only* GPIO diagnostic. Release the keys. Move each joystick direction once; the screen shows **RAW PIN GPnn** and its newly mapped **FW MAP** interpretation side by side, plus held/released status and a press counter. Tap each K key individually to confirm its physical GPIO. Hold K1+K4 again to exit. Normal K1–K4/joystick page navigation is suppressed while the lab is open.

See [PULSE-0.2C INPUT LAB BENCH](../../docs/PULSE-0.2C-INPUT-LAB-BENCH.md) for a reproducible five-direction test and a concise report format. This mode sends **no hardware, CAN or actuator commands**, and has no remote USB control endpoints.

## PULSE-0.2D.3 — Color Lab (diagnostic candidate)

**Physical color calibration PASS 2026-10-24:** ITALIA's literal green/white/red bars previously appeared magenta/black/yellow. The operator confirmed **Color Lab mode4 (NORM / BGR)** renders the three correct colors. The earlier firmware used **`MADCTL 0x60 + INVON 0x21`**; this branch now starts automatically with **`MADCTL 0x68 + INVOFF 0x20`**. The revised startup still needs physical boot/reboot verification.

### Test all 4 combinations with one flash

Hold **K1+K2 for >=1.2s** to enter `RGD / COLOR LAB`. Three large literal RGB565 blocks request **G / W / R**. Tap joystick RIGHT to cycle:

1. `INV / RGB`: `MADCTL=0x60`, `INVON=0x21` (old/default).
2. `NORM / RGB`: `MADCTL=0x60`, `INVOFF=0x20`.
3. `INV / BGR`: `MADCTL=0x68`, `INVON=0x21`.
4. `NORM / BGR`: `MADCTL=0x68`, `INVOFF=0x20` (**physical PASS; new default**).

No orientation, GPIO or USB changes between modes. Joystick CLICK **applies** the preview for the remainder of this power-on session; hold K1+K2 to **cancel** and return to the previously applied mode. On **every reboot**, firmware now starts in the physically verified `NORM / BGR` configuration, regardless of temporary Color Lab previews. K1+K4 INPUT LAB and K2+K3 skin SETTINGS remain unchanged.

**Full physical findings and follow-up boot verification:** [PULSE-0.2D-COLOR-LAB.md](../../docs/PULSE-0.2D-COLOR-LAB.md). Mode4 was physically accepted; only the *new automatic boot path* is awaiting operator confirmation. No motor/sensor/host write operations or RP2040 flash preference writes are added.

## PULSE-0.2D — Scroll acceleration and skins (candidate)

Built-in **Winamp-inspired skin switcher** on the 128×128 Chronoscope. No new hardware controls and no change to GPIO mapping. Normal K1–K4 pages and the operator-approved physical joystick directions are preserved.

- **Hold K2 + K3 together for at least 1.2 seconds** to open **SETTINGS → SKINS**.
- Joystick **UP/DOWN or LEFT/RIGHT** cycles four fully local, instant-preview skins: **RGD Classic**, **Phosphor** (green oscilloscope), **Chrome 99** (metallic late-1990s), **Italia** (subtle green/white/red accent).
- Joystick **CLICK** applies the preview and returns to the normal pages. Hold **K2+K3** again to **cancel** without applying.
- The selected skin stays active on all four dashboards **until the Pico loses power or reboots**. It is **RAM-only** in this wave; we do not write RP2040 firmware flash or claim the skin is persisted in the Console. A future, separately accepted settings adapter may sync the stable skin key to persistent host preferences.
- Hold the joystick in a direction for **440ms** to begin autorepeat; repetition accelerates from 240ms to 155ms after 1.6s, to 100ms after 3s. K1–K4 and joystick **CLICK never autorepeat**.
- A tiny letter+counter in the top bar acknowledges each navigation event, including when list/zoom bounds prevent movement.
- Existing **K1+K4** Input Lab remains accessible and uses high-contrast Classic coloring.

Four pages, visual `DEMO` labels, real vs synthetic provenance, USB CDC `HELLO / DEMO / CRC ACK` and no-motion safety boundaries are unchanged. Original skin accents **never recolor warning/danger semantics**.

See [PULSE-0.2D skins/navigation spec](../../docs/PULSE-0.2D-SKINS-NAVIGATION.md). Current implementation remains a **software candidate** until a physical Pico test verifies settings chords, acceleration and USB regression.

## Compile and flash

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.2d-color-lab
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

- Physical 0.2C input/navigation and 20/20 synthetic HELLO/ACK were operator-verified. ST7735S Color Lab mode4 was physically verified to show correct G/W/R; the *new compiled boot default* and final pixel offsets/clipping still require physical regression.
- Actual redraw latency and joysticks during simultaneous USB traffic.
- 3/3 repeated serial opens, 100/100 ACK and cable disconnect/reconnect regressions.
- Real Somatic loop measurement, Chronograph provenance/Chronon data, calibrated IMU, full joint mapping and measured voltage.

The Pico is not an E-stop, safety supervisor, balance sensor or actuator controller. CRC detects accidental transmission corruption but does not authenticate devices.
