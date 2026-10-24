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
- Header shows a tiny input event indicator (`U/D/L/R/C/K` plus rolling counter digit), even if the selected row cannot move. This is UI activity only, *not* evidence of Somatic loop progress.
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
| **Italia** | `italia` | Graphite-green instrumentation with a small green/white/red stripe |

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
