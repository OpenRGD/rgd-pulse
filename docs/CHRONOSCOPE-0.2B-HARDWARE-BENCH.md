# PULSE-0.2B — Four-screen landscape Chronoscope physical bench

**Status:** firmware candidate compiled in CI; this document is a physical operator acceptance procedure, NOT a report of completed hardware tests. Do not confuse 0.1C hardware successes with 0.2B physical evidence.

## Operator physical bring-up and 180° correction — 2026-10-24

**Operator-reported PULSE-0.2B first flash:** OPENRGD splash appeared briefly, then the graphical pages and charts rendered. K1–K4 page buttons reportedly worked correctly. **Observed defect:** graphics were rotated **180° upside-down** relative to the intended front view (joystick LEFT, K1–K4 RIGHT). Do not call the first-flash visual orientation a pass. Physical joystick directions, graph focus/zoom, exact colors/pixel offsets and post-0.2B USB ACK regressions were not separately reported at this stage.

**Minimal source fix:** changed `firmware/pico-w/src/lcd.rs` ST7735S `MADCTL_LANDSCAPE_CANDIDATE` from `0xA0` (`MY|MV`) to **`0x60` (`MX|MV`)**. The two landscape settings differ by a 180° display rotation. There are **no changes** to K1–K4 GPIOs, landscape joystick mapping, 128×128 framebuffer, Somatic/demo data, USB CDC or ACK handling.

**Status after fix:** new firmware candidate; **not yet physically re-flashed or verified**. The panel memory offsets remain provisional (X=2, Y=3), and physical clipping/color order still require confirmation.

### Short acceptance run for correction

1. Pull the new `feat/pulse-0.2b-chronoscope-four-pages` HEAD, build/flash the UF2 using the instructions below. Do **not** rotate the physical board a second time.
2. Hold the board with joystick on **LEFT** and buttons **RIGHT**. Confirm the OPENRGD splash, SOMATIC headings, charts and footers are upright. Verify labels are **not mirrored** and none are cut off at the panel edges.
3. Press K1, K2, K3, K4, and then joystick UP/DOWN/LEFT/RIGHT/CLICK in their actual operator-facing directions; K keys worked in the previous flash, but logical joystick axis mapping has not yet been physically evidenced.
4. On K1 SOMATIC, click the joystick, then UP twice: `ZOOM x1 -> x2 -> x4`; CLICK closes the focus.
5. If the orientation is correct, run the existing read-only 20/100-frame USB synthetic tests (no motors). Capture any new display clipping or color issues.

## Changes compared with verified 0.1C

- Native display target **128×128 pixels**; physically mount/hold entire Seengreat Rev 1.2 PCB 90° clockwise from vendor upright orientation: **joystick LEFT / K1..K4 RIGHT**.
- Corrected landscape ST7735S MADCTL candidate **`0x60` (MX|MV)** after the first real 0.2B flash displayed inverted graphics with `0xA0` (MY|MV); X/Y offsets (2,3) are still provisional. The corrected orientation has **not yet** been tested physically; panel borders and RGB/BGR still need verification.
- **Hardware SPI0 8MHz** on GP6/GP7 for RGB565 full-frame transfer. One 32KiB framebuffer with bounded, explicit redraws. UI rendering is independent of the read-only USB CDC ACK bridge.
- **USB initialization before LCD boot effects**; timer-based LCD reset/SLPOUT delays instead of long blocking CPU spins. Intended to help the earlier >5s USB startup lag, but only a fresh kernel timing trial can verify an improvement.
- Four direct K-page selectors + local joystick **focus and zoom**. No joystick action sends a robot command.
- All simulated timeline entries and graphs visibly marked **DEMO**; live Somatic/IMU/Chronon/Power data are **not connected**.
- DTR close/reopen and CRC-confirmed DEMO ACK sequence from 0.1C preserved.

## Code-only checks

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.2b-chronoscope-four-pages
git pull --ff-only
cargo test --workspace
cargo fmt --all -- --check

cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Do not flash until the compiled UF2 exists. At present the mini-PC is equipped with several other USB serial, CAN and STM32 devices. Power the Pico **only by USB**, never connect it to Paky-01's motor, 12V or 24V wiring.

## Flash only the Pico

Disconnect the Pico's Micro-USB, hold Pico **BOOTSEL**, reconnect and release. Confirm exactly the `RPI-RP2` mass-storage label (not an STM32 `DIS_...` disk). Then:

```bash
cd ~/openrgd/rgd-pulse/firmware/pico-w
PICO_MOUNT="/media/$USER/RPI-RP2"
if mountpoint -q "$PICO_MOUNT" && [ -f pulse-pico-w.uf2 ]; then
  cp -v pulse-pico-w.uf2 "$PICO_MOUNT/"
  sync
else
  echo "NOT FLASHED: Pico mass-storage mount or UF2 not available"
fi
```

## Physical acceptance matrix

| Operation | Expected on the 90° clockwise-mounted PCB |
|---|---|
| Boot | OPENRGD splash, then SOMATIC |
| K1 (top right) | SOMATIC |
| K2 | CHRONONS |
| K3 | IMU |
| K4 (bottom right) | SYSTEM |
| Joystick physical UP | Previous metric/event/tile |
| DOWN | Next metric/event/tile |
| LEFT/RIGHT | History window, chronon filter, or tile |
| Joystick CLICK in SOMATIC/IMU | Enters focus chart; `ZOOM x1` displayed |
| In chart focus, UP then UP | Zoom factor advances to x2 then x4; graph scale changes |
| In chart focus, DOWN | Zoom level reduces |
| In chart focus, LEFT/RIGHT | History pan changes |
| CLICK in chart focus | Closes focus; K1–K4 always escape |
| CLICK in CHRONONS | Opens a clearly labelled DEMO-only event detail; click closes |
| CLICK in SYSTEM | Enlarges selected inventory value, with missing fields `--` |
| Robot data absent | SOMATIC loop shows `---Hz`/no real source; IMU `--.-`; SYSTEM `22 CFG` but `ONLINE --` and `DRIVE --.-V` |
| USB closed | Link marker inactive; local keys/pages still work |
| USB active | Synthetic ACK and progress bar; buttons work while 20/100-frame demo is transmitting |
| Power disconnected | Replug and recover without flash or motor connection |

**Critical visual checks:** top of text toward the top of the landscape-mounted board; graph X-axis left→right; no horizontal mirrored letters; actual K1..K4 placement. Verify that no text is clipped by the provisional panel offsets. Photograph the display with joystick physically on the left and buttons on the right if a rotation correction is needed.

## USB regressions (synthetic only)

Find the Pico by its own prototype USB identity, not any other `/dev/ttyUSB0`:

```bash
lsusb -d c0de:cafe
ls -l /dev/serial/by-id/*OpenRGD*
udevadm info -q property -n /dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00 |
  grep -E '^ID_(VENDOR_ID|MODEL_ID|MODEL)='
```

Confirm the device exists before proceeding:

```bash
cd ~/openrgd/rgd-pulse
PICO_PORT="/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00"
if [ -e "$PICO_PORT" ]; then
  for n in 1 2 3; do
    cargo run -p pulse-bridge -- serial-demo "$PICO_PORT" 20 || break
    sleep 2
  done
  cargo run -p pulse-bridge -- serial-demo "$PICO_PORT" 100
fi
```

Required result: 3/3 new HELLO handshakes, 60/60 ACK, 100/100 additional ACK, zero unexpected frames. If any regression fails, capture host output / USB dmesg **before** power-cycling. No Somatic control loop measurements, Chronograph correctness or actuator safety claims.

## Limitations / follow-up

1. **None of the four application telemetry feeds is connected yet.** Real Somatic timing requires separately reviewed read-only runtime instrumentation. Real Chronons require source-linked Chronograph/HyperAion API; no invented decisions. Real IMU must be fanned out from Paky-01's existing read-only publisher, not by reopening its CH340 serial device; physical extrinsics and absolute yaw still uncalibrated. Motor count is a BHL reference, not real online inventory; drive voltage unknown.
2. Full framebuffer at 8MHz SPI0 is an intentional MVP tradeoff; subsequent wave can implement dirty rectangles and DMA if physical refresh latency/USB ACK performance warrants it.
3. Explicitly synthetic charts are deterministic preview shapes, not measurement history. The UI can zoom and pan their plotted sampling range; host time series can be supplied later over a separately versioned telemetry UI contract.
4. Initial 0.2B firmware physically displayed the splash/pages/charts and K-buttons responded, but `0xA0` rendered upside-down. The new `0x60` orientation, RAM offsets and color palette remain **unverified corrections/candidates** and need another physical check.
5. Production must use a legitimate USB VID/PID and per-device serial number; prototype identification alone is not unique.
