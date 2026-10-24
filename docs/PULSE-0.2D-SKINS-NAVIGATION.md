# PULSE-0.2D — Accelerated navigation + Winamp-inspired skin engine

**Status: IMPLEMENTED SOFTWARE CANDIDATE, physical Pico W verification pending.**
Implementation lives in `feat/pulse-0.2d-navigation-skins`, stacked on 0.2C. No physical motor, CAN or safety operations. Only explicit synthetic DEMO/ACK transport until separately reviewed real telemetry adapters exist.

## What the user asked for

Make the 128×128 ST7735S Chronoscope feel like an instrument with tactile input: held joystick directions repeat, acceleration increases with continued hold, obvious visual acknowledgement even at a list/chart boundary, and instant switching between substantially different dashboard themes ("skins" in the spirit of classic Winamp customization).

Preserve front-view final physical orientation: **joystick LEFT, K1–K4 RIGHT**, same operator-tested GPIO map:

| Movement | Raw GPIO | Rust action |
|---|---|---|
| UP | GP18 | Up |
| DOWN | GP20 | Down |
| LEFT | GP16 | Left |
| RIGHT | GP19 | Right |
| CLICK | GP17 | Click |

K1=GP8, K2=GP4, K3=GP3, K4=GP2. No joystick mappings changed in 0.2D. K1–K4 always select their dashboard in normal mode.

## Controls

**Normal four-page mode:**
- K1 SOMATIC, K2 CHRONONS, K3 IMU, K4 SYSTEM.
- Joystick single press retains current selection/filter/zoom behavior.
- Holding a **direction**: initial event immediately from GPIO edge; first auto-repeat after **440ms**, then intervals **240ms**, accelerating to **155ms** after 1.6s, and **100ms** after 3s of continuous hold. Release resets the delay. These are software *target timings*, not measured motor/display latency.
- K1–K4 and joystick CLICK are **never auto-repeated**.
- Header shows **K1/K2/K3/K4 for the actual physical page key**; joystick actions show `U/D/L/R/C` plus a small rolling event digit, even at a selection boundary. There are **only four** top-level pages. `K5`, `K6`, etc. were a misleading input-count label in earlier 0.2D builds, **not additional dashboards**. This is UI activity only, never evidence of Somatic loop progress.
- Retains operator-proven K1+K4 long hold (1.2s) to enter/exit **INPUT LAB**, which shows raw GPIO independently of the current mapping.

**Open Settings / Skins:**
- Hold **K2 + K3 simultaneously for >=1.2s**. Both buttons are otherwise direct page selectors; a non-perfectly simultaneous chord may briefly change the underlying page before opening Settings. The settings chord never communicates with the host or robot motors.
- `UP/DOWN` **or** `LEFT/RIGHT` cycles four installed skins with immediate visual preview.
- Joystick `CLICK` applies the preview to all four dashboards and returns to the previously active top-level UI.
- Hold K2+K3 again to **cancel** and restore the previously applied skin (or exit without applying).
- K1+K4 INPUT LAB and K2+K3 SETTINGS are mutually exclusive; no need for another physical button. Input Lab remains deliberately in the high-contrast Classic palette for diagnosis.
- Preview and applied skin names are human-readable but identified internally by stable keys.

**Important persistent-settings limitation:** in 0.2D the selected skin is stored **in Pico RAM only**: switching between screens and USB sessions preserves it, **disconnecting power or rebooting the Pico restores RGD Classic**. Neither RP2040 flash writes nor the external Console preferences API are in scope; persistence can be added via an explicit versioned, safe preference adapter later. No unannounced writes to XIP firmware sectors.

## Four installed skin manifests

| Skin | Stable id | Appearance / chart accent |
|---|---|---|
| **RGD Classic** | `rgd-classic` | Existing dark navy/cyan look, clean framed graphs |
| **Phosphor** | `phosphor` | Deep green-black phosphor CRT/oscilloscope palette, discrete trace points |
| **Chrome 99** | `chrome-99` | Late-1990s metallic chrome/cobalt accents, beveled chart edges |
| **Italia** | `italia` | Neutral charcoal panels, dominant saturated emerald-green accents and a clearly visible green/white/red flag (4px top strip plus 12px settings swatches) |

These are **original visual styles**, not copyrighted Winamp artwork. All dashboards use the same `pulse-ui::UiState`, `RenderData`, USB protocol and safety/provenance labels. A skin changes **palette, panel chrome and chart decoration**, never measurement values, units, timestamps, severity classes or acceptance criteria. Explicit `DEMO` remains visible under every skin; `AMBER` warnings and `RED` error indications are not recolored into green.

### Technical ownership

- `crates/pulse-ui/src/skins.rs`: no_std `SkinId`, `SkinManifest`, stable id and `ChromeStyle`. Exactly four ROM-resident candidates.
- `crates/pulse-ui/src/settings.rs`: `Settings` overlay and K2+K3 chord; `preview` vs `applied` skin, commit/cancel state, unit tests.
- `crates/pulse-ui/src/input_repeat.rs`: hold-to-repeat with tiered acceleration for raw directional input indexes `4,5,7,8`, unit tests. Raw CLICK index `6` excluded.
- `crates/pulse-ui/src/lib.rs`: raw GPIO→operator action remains as physically accepted in 0.2C, adds `last_action` and `input_count` on local UI state for feedback.
- `firmware/pico-w/src/render.rs`: one shared 128×128 RGB565 scene graph with palette/visual-role mapping, optional decorative chart styles and local skin picker. It does not request additional measured telemetry.
- `firmware/pico-w/src/main.rs`: integrates two separate long-press overlays, repeat events, DTR/USB, framebuffer selection and skin preview. Existing Pico hardware SPI0 8 MHz / ST7735S MADCTL 0x60/offsets remain unchanged.

### Extension path — skin packs in the Console

Potential future `openrgd.pulse-skin.v0.1-candidate` contract:
- `skin_id`, `display_name`, `schema_version`, `supported_panel` (`128x128/rgb565`), and `chrome_style` enum;
- semantic roles `background`, `panel`, `border`, `text`, `muted`, `accent`, `positive`; **status-critical red/amber fixed**;
- optional vetted 1-bit bitmap/icons/fonts or precompiled layout variant with a bounded memory budget;
- explicit immutable package signature/version and Console permission model before any OTA/user skin import.

No arbitrary script/plugin executes on Pico; no unbounded images/fonts are imported via CDC. A future device-preference handshake could sync selected `skin_id` to the OpenRGD Console and persist it outside the microcontroller, but requires a reviewed, independent protocol version and authenticated/pinned device identity. No claim that this is already implemented.

## PULSE-0.2D.2 — K5/K6 confusion fixed (operator report, 2026-10-24)

The operator reported apparent extra `K5`, `K6` etc. while repeatedly pressing the K buttons. The firmware had always defined exactly **four dashboard pages**. A header input feedback widget incorrectly printed `"K" + (ui.input_count % 10)` for *all* four page buttons, causing the counter to look like additional screens.

The renderer now uses `Page::physical_key_label()`:
- physical K1 → `K1`, K2 → `K2`, K3 → `K3`, K4 → `K4`, regardless of how often pressed;
- joystick UP/DOWN/LEFT/RIGHT/CENTER retains the input-event counter for visible feedback at selection boundaries.

Only the feedback label changes; no GPIO mapping, page navigation, USB protocol, DEMO indicators, display rotation or robot wiring changed. `pulse-ui` unit test covers repeated page selections to catch regression. **Host tests and RP2040 firmware build passed.** Physical visual confirmation on the next flash remains desirable, not a new physical mapping calibration.

## PULSE-0.2D.1 — Italia visual contrast fix / RGB-BGR gate (operator report, 2026-10-24)

After the first hardware trial of the four skin options, the operator reported that **Italia appeared purple, black and blue**, rather than green/white/red. This report **does not** prove a controller wiring problem or identify which displayed pixels suffered an incorrect channel ordering.

**Confirmed code/design cause:** the initial Italia preset was mostly extremely dark, blue-green-tinted RGB565 colors (`bg = 2,4,6`; `panel = 5,11,11`; `border = 12,22,21` in 5/6/5 channels), with desaturated green accent and only a 1px-wide tricolore header stripe. It was not recognizably Italian even if the panel reproduced every RGB565 channel correctly.

**Source correction committed** in the active `feat/pulse-0.2d-navigation-skins` branch:
- Neutral near-black charcoal backgrounds and panels with matched R/G/B intensities; avoid purple/blue tint while preserving readability.
- Strong **true RGB565 emerald** accent `(0,63,0)`, bright white `(31,63,31)`, and explicit red `(31,0,0)`.
- 4px tricolore header across all four dashboards, 12px-tall flag preview in `SETTINGS→SKINS`, plus a larger three-color detail in the graphs.
- Literal flag colors bypass the semantic-theme `GREEN/WHITE/RED` palette-role mapper. Warning/error categories retain their semantic colors; no telemetry or control behavior changed.

**Potential physical issue deliberately *not* changed yet:** the ST7735S `MADCTL=0x60` uses RGB ordering (bit 3 = 0). The controller has an independent RGB/BGR color-filter selector at bit 3 (`0x08`); switching to `0x68` would swap red/blue **globally** without changing rotation, if and only if a physical calibration proves the panel requires BGR. The current firmware also issues `INVON=0x21` from earlier verified bring-up; panel inversion/gamma may influence appearance, but is not diagnosed by one subjective color report. Do **not** change inversion or RGB/BGR globally based only on the Italia palette report.

### Physical color-test acceptance

1. Flash the updated branch on **Pico only**, keep joystick LEFT and K1–K4 RIGHT.
2. Open K2+K3 SETTINGS→SKINS, preview **Italia** and inspect the **three large swatches left-to-right**.
3. They must be recognizable as **GREEN – WHITE – RED**, with **RED on the RIGHT**. On any normal dashboard in Italia skin, verify the header band is also green/white/red and the data panels are neutral charcoal with green accents.
4. **If the third/right swatch is BLUE or PURPLE**, report that explicitly; that is actionable evidence of RGB/BGR mismatch or another physical color problem. Prefer a clear photo of the LCD displaying the three flag swatches, then test the `MADCTL BGR` bit in a separate, hardware-scoped color-calibration wave.
5. Verify Classic/Phosphor/Chrome 99 remain legible. Run a basic 20/20 synthetic USB ACK regression if the display appears responsive.

**Until this post-fix physical check passes, the revised Italia palette is a code candidate, not an accepted calibrated skin.**

## Physical bench checklist

Build and flash **only the Pico W**. Keep it isolated from robot actuators, CAN power and 12/24 V; use a verified RPI-RP2 BOOTSEL volume.

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.2d-navigation-skins
git pull --ff-only
cargo test --workspace
cargo fmt --all -- --check

cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Hardware steps:

1. Confirm the short **OPENRGD** splash, upright landscape orientation, four K pages and working GP17 CLICK.
2. On K2 CHRONONS, press DOWN and release: one list step. Hold DOWN for >3s: items advance continuously until the bounded end, with faster repeat after the initial delay. At the last item, the header key indicator still increments. Release; a new press starts again from the initial delay.
3. On K1 SOMATIC, CLICK focus, hold UP: zoom increases but saturates safely at **x8**. No full renderer lockup; CLICK still closes. Left/right pans synthetic fixture timeline, never the actual loop.
4. Hold K2+K3 together >=1.2s. SKINS menu appears. Browse 4 distinct visual choices with UP/DOWN or LEFT/RIGHT and see **immediate preview**.
5. CLICK to apply, confirm all K1..K4 dashboards adopt that theme while `DEMO` and unverified readouts remain truthful.
6. Reopen settings, select another preview, K2+K3 long hold to cancel, confirm the previously **applied** theme remains active.
7. K1+K4 still opens INPUT LAB, whose RAW GPIO map is unchanged; no theme selection occurs inside Input Lab.
8. Close/reopen USB serial and run `pulse-bridge serial-demo` with the **verified by-id Pico** for 20 then 100 synthetic frames. Expect HELLO and matching CRC-checked ACKs (no motor commands). The Pico display should remain responsive.
9. Disconnect/reconnect Pico **USB only** and confirm restart uses **RGD Classic** (explicit current RAM-only behavior), with no inability to boot or enumerate.

```bash
cd ~/openrgd/rgd-pulse
PICO_PORT="/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00"
if [ -e "$PICO_PORT" ]; then
    cargo run -p pulse-bridge -- serial-demo "$PICO_PORT" 20
else
    echo "Pico Pulse not found: inspect lsusb -d c0de:cafe / udev"
fi
```

Do not infer actual Somatic 1 kHz loop frequency from these synthetic tests. Panel palette RGB/BGR and offset precision remain hardware commissioning items. Leave PULSE-0.2D PR **draft** until physical acceptance. The parent PR stack is still open.

### Software test gates

- Host `cargo test --workspace`, `cargo fmt --all -- --check`.
- Pico `cargo build --release`, RP2040 BOOT2 256 bytes @0x10000000 and vector table @0x10000100 validated by CI.
- Hold-repeat tests: first delay, speed tiers, release/reset, no CLICK/K-key repeats, saturation.
- Settings tests: short chord ignored, long hold toggles once, preview vs applied, cancel rollback, disallow other overlay, bounds/wrapping and stable keys.
- UI feedback independent of selected metric state; no false real telemetry claims.
