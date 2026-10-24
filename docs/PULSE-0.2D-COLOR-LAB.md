# PULSE-0.2D.3 — ST7735S Color Lab, physical RGB/BGR × inversion

**Status: COLOR-CALIBRATION PHYSICAL PASS, 2026-10-24 (operator report).** Mode 4 **NORM / BGR** shows the correct green/white/red swatches on the actual Seengreat ST7735S panel. The source boot default was then changed to this physically demonstrated mode; **the revised automatic power-on behavior has not yet been re-flashed and verified**. This is a separate draft PR stacked on `feat/pulse-0.2d-navigation-skins`.

## Why this test exists

Operator reports on the actual Seengreat PICO-1.44inch LCD Rev 1.2 (ST7735S, 128×128): the revised **Italia** skin's three **explicit literal RGB565** swatches that were meant to appear **green / white / red** look **magenta / black / yellow**.

This is a precise signal, not a minor palette design difference:

| Programmed RGB565 | Hex | Color with bitwise complement alone | Complement + red↔blue swap | Operator saw |
|---|---|---|---|---|
| Pure GREEN | `0x07E0` | Magenta `0xF81F` | Magenta `0xF81F` | **Magenta** |
| Pure WHITE | `0xFFFF` | Black `0x0000` | Black `0x0000` | **Black** |
| Pure RED | `0xF800` | Cyan `0x07FF` | Yellow `0xFFE0` | **Yellow** |

**All three match the model** of an unwanted inversion plus a swapped red/blue channel. This is a strong *hypothesis* for the physical display configuration, not an established electronics fault or a certified ST7735S transfer function. A panel driver with different inversion/gamma could need a different sequence; only a controlled optical test settles it.

### Two independent relevant ST7735S commands

- `0x20` = `INVOFF` (display inversion OFF); `0x21` = `INVON` (ON).
- `0x36` = `MADCTL` (Memory Data Access Control), bit `0x08` = BGR order. The **orientation bits MX|MV are 0x60**, already operator-accepted with joystick LEFT and K1..K4 RIGHT. Therefore `MADCTL=0x60` and `MADCTL=0x68` keep orientation unchanged.
- `0x3A` `COLMOD=0x05` retains 16-bit RGB565; Pico SPI0 remains 8 MHz GP6/GP7 with software/full framebuffer and explicit upper/lower RGB565 byte order unchanged.

The **original** color settings were `MADCTL=0x60` plus `INVON=0x21`. The **0.2D.3 Color Lab** demonstrated that the assembled device instead needs `MADCTL=0x68` plus `INVOFF=0x20`. The boot default on this PR now follows that operator-verified combination; the other three modes remain accessible for diagnostics without reflashing.

## One-time firmware update then four modes

The new **Color Lab** opens with a third independent hold chord:

- **Hold K1 + K2 together for >=1.2 seconds** (normal hardware K buttons).
- Joystick LEFT/RIGHT (or UP/DOWN) advances between **four numbered modes** with immediate physical LCD command update and full repaint.
- Screen always draws the same 3 **pure numeric RGB565** swatches: `G=0x07E0`, `W=0xFFFF`, `R=0xF800`. Their labels name the *requested* colors, even if the panel shows different ones.
- **CLICK applies the currently previewed controller mode for the rest of this power-on session** and returns to the original Chronoscope dashboard.
- Hold K1+K2 again to **cancel**, restoring the previously applied mode. **No XIP flash preference writes**: reboot restores the *compiled, operator-verified boot default* NORM / BGR (0x68 + 0x20), regardless of temporary Color Lab experiments.
- Existing **K1+K4 INPUT LAB** and **K2+K3 SETTINGS/SKINS** remain; Color Lab is mutually exclusive with those overlays.
- K1..K4 direct dashboard navigation in normal mode, the validated GP18=UP/GP20=DOWN/GP16=LEFT/GP19=RIGHT/GP17=CLICK, PULSE/1 USB HELLO/DEMO/ACK and non-actuating boundaries remain unchanged. Individual K presses may select a normal page while entering a two-key hold chord; this is harmless.

| Lab mode | Header label | MADCTL | Inversion command | Predicted colors G/W/R under the two-error hypothesis |
|---|---|---|---|---|
| 1 (original) | `INV / RGB` | `0x60` | `INVON 0x21` | **magenta / black / yellow** (reported baseline) |
| 2 | `NORM / RGB` | `0x60` | `INVOFF 0x20` | green / white / **blue** |
| 3 | `INV / BGR` | `0x68` | `INVON 0x21` | magenta / black / **cyan** |
| 4 (**physical PASS**) | `NORM / BGR` | `0x68` | `INVOFF 0x20` | **green / white / red — observed by operator** |

These **predictions assume the inferred double transformation** and are not measured results. The actual physical sequence might differ. Record what really happens in modes 1–4.

## Physical result and source promotion — 2026-10-24

The operator has now confirmed: **"sì la modalità 4 offre i colori corretti"**. This closes the Color Lab's core diagnosis for this actual Pico/Seengreat unit: **MADCTL 0x68 (MX|MV|BGR) + INVOFF 0x20** produces the correct explicit green/white/red swatches. We do not infer identical calibration for arbitrary other LCD boards or untested hardware revisions.

**Applied in source:**
- `pulse-ui::color_lab::VALIDATED_BOOT_MODE = ColorMode::NormalBgr` is now the single default.
- `ColorLab::new()` starts with that applied/preview mode, preventing cancellation or reopen from falling back to the earlier inverted palette.
- `lcd.init()` calls `apply_color_mode(VALIDATED_BOOT_MODE)` *before the OPENRGD splash*.
- The Pico `lcd_color_mode` state tracker initializes to the same constant, so firmware and actual LCD registers remain synchronized.
- All four modes remain selectable via K1+K2, so the previous legacy 0x60+INVON configuration can still be inspected but is never the new startup default.
- Explicit regression tests assert mode4 `0x68`, `0x20` and `ColorLab` reboot/default values; existing mapping, USB/ACK and no-motor contracts are unchanged.

**Remaining acceptance:** after building/flashing this revised source, physically power-cycle/replug the Pico **without opening Color Lab** and verify the *first* OPENRGD splash, all four dashboards and Italia skin show correct colors. Also check K1–K4 and USB synthetic 20/20 ACK remain functional. The test above proves physical mode4 effectiveness, but **not yet the new boot-time default code**.

## Physical procedure

Build from the **separate Color Lab branch**, not the 0.2D skin or 0.3A IMU branch:

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.2d-color-lab
git pull --ff-only

cargo test --workspace
cargo fmt --all -- --check

cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Power and flash **only Pico via its Micro-USB**. Enter Pico **BOOTSEL**, explicitly verify `RPI-RP2` removable drive, and never copy a UF2 to the STM32 programmer drive:

```bash
PICO_MOUNT="$(findmnt -rn -S LABEL=RPI-RP2 -o TARGET | head -n 1)"
if [ -n "$PICO_MOUNT" ] && mountpoint -q "$PICO_MOUNT" &&
   [ -f pulse-pico-w.uf2 ]; then
    echo "Verified Pico mount: $PICO_MOUNT"
    cp -v pulse-pico-w.uf2 "$PICO_MOUNT/"
    sync
else
    echo "ABORT: Pico BOOTSEL RPI-RP2 drive or UF2 absent"
fi
```

### Reading the color test

1. **After flashing the new source, do a power-cycle test first.** Confirm the splash and normal dashboards are now correct without opening Color Lab.
2. Hold **K1+K2** for about 1.2 seconds to open `RGD / COLOR LAB`. It must **start on MODE 4 / NORM / BGR**, showing GREEN / WHITE / RED directly.
3. You can inspect the other modes with joystick RIGHT/LEFT (mode4 wraps to 1), but **return to mode4** or hold K1+K2 to cancel any experiment; CLICK applies a selected temporary test mode.
4. Verify the three large requested RGB565 swatches are correct in mode4, that closing the lab preserves that choice and that reboot always restores the calibrated default.
5. If the new boot-time splash or the mode4 swatches are not correct, capture the reported colors; do not assume a different rotation, GPIO mapping, gamma or byte order without evidence.
6. Test the selected mode on **all four skins** (especially `DEMO`, RED errors and GREEN status): the correct mode should not accidentally hide stale/unverified data or change any GPIO/USB behavior.
7. Optionally rerun the verified synthetic USB `serial-demo` for 20 frames, ensuring `HELLO` and **20/20 CRC ACK / 0 unexpected** before accepting any driver default change.

### Minimal report from operator

```text
1 INV / RGB  -> LEFT ?, MID ?, RIGHT ?
2 NORM / RGB -> LEFT ?, MID ?, RIGHT ?
3 INV / BGR  -> LEFT ?, MID ?, RIGHT ?
4 NORM / BGR -> LEFT ?, MID ?, RIGHT ?
Chosen mode:
```

**Mode 4 has succeeded on the physical device, and the source default has been promoted** from `0x60 + 0x21` to `0x68 + 0x20`. The Color Lab stays available as a reversible diagnostic. Do not change orientation, MCU pin assignments, warning semantics or USB protocol. The operator must still verify the **new boot path** after flashing, since this source promotion was made *after* the physical color-mode experiment.

### Source verification

Hardware: Seengreat 1.44 inch LCD Display wiki describes ST7735S and SPI0 on GP6/GP7. ST7735S published command definitions and mature TFT_eSPI ST7735 driver confirm `INVOFF=0x20`, `INVON=0x21`, `MADCTL=0x36`, MX=0x40, MV=0x20 and BGR=0x08. This Color Lab tests those documented controls without touching external robot electronics.
