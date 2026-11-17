# PULSE-0.2A — Chronoscope UI / four-screen design candidate

**Status:** DESIGN PROPOSAL, not firmware. Requires operator approval and data-contract review before the 0.2 implementation. This document does not authorize robot motion or change the canonical OpenRGD schemas.

## Goal

Build a high-quality, legible observability HMI on the real **Raspberry Pi Pico W (RP2040)** and **Seengreat Rev 1.2 ST7735S 128×128 SPI TFT**. Keep it autonomous and diagnostic-only. The main 5-inch Paky-01 screen remains the rich Console UI. The 1.44-inch Pulse LCD is a compact *window into time, sensors and host/runtime health*.

The current firmware has already shown real LCD graphics, a blinking indicator, 9 hardware inputs and USB CDC with synthetic HELLO / DEMO / ACK. It does NOT currently display real Somatic loop metrics, verified Chronons, IMU data or motor/voltage readings.

## Mandatory physical orientation — LANDSCAPE / controls LEFT–RIGHT

**Accepted operator direction:** view the front of the Seengreat board with the **joystick on the LEFT** and the **four K1–K4 buttons on the RIGHT**. Compared with the vendor portrait photograph (K1–K4 above the LCD, joystick below), rotate the **whole assembled board clockwise by 90° as viewed from the display/front**. All UI lettering, numbers, charts and movement should read naturally in this physical landscape pose, not in the original upright portrait view. **Because the physical PCB is rotated +90° clockwise, content drawn relative to the original portrait/native panel coordinate frame must be compensated by -90° counter-clockwise** (or the equivalent validated hardware MADCTL mapping).

```text
                  FRONT — OPERATOR'S LANDSCAPE VIEW
          +----------------------------------------+
          |              128 × 128 LCD             |
          |              RGD / LOOP                |
   UP     |    998 Hz             ● LIVE           |   [ K1 ] LOOP
 LEFT O RIGHT  ┌──────────────┐   1.02 ms         |   [ K2 ] CHRONONS
  DOWN   |     ╱╲__╱╲___╱╲    └──────────────┘    |   [ K3 ] IMU
         |                                        |   [ K4 ] SYSTEM
         +----------------------------------------+
  JOYSTICK              SQUARE PANEL                 FOUR BUTTONS
```

Schematic illustrates the **wide physical module**, not a wider LCD: the display still has **128×128 square pixels**. Numbers and LIVE status are graphic placeholders, not measured values. The final hardware arrangement is joystick at the left, K1..K4 in a vertical column on the right, **top-to-bottom K1 → K4** after clockwise rotation.

### Coordinate system and rotation ownership

- Define the **logical display coordinate frame from the final operator view**: `x=0` at the display's left edge closest to joystick, `x=127` at the right edge closest to keys; `y=0` at the visual top, `y=127` at the bottom.
- **Physical assembly:** +90° clockwise (portrait vendor pose → joystick left, keys right). **Rendering compensation:** -90° counter-clockwise relative to the old panel/native coordinate frame, so the user sees upright landscape text. The hardware-driver MADCTL orientation and ST7735S X/Y offsets must be validated on the actual panel. Don't commit an unverified MADCTL bit pattern; avoid double-rotating between the driver and UI layer.
- The 128×128 resolution **does not become 128×anything wider** on rotation. The physical PCB/module is landscape, not the pixel matrix aspect ratio.
- All typography, chart axes, indicators, chronological direction and gestures refer to **the logical operator view**, regardless of raw native GPIO orientation.
- Both UI and main 5-inch Console should use the same meaning for UP/DOWN, LEFT/RIGHT in their controls. A K1–K4 press always changes the top-level page, never triggers motion.

### Orientation-aware joystick mapping (proposed, physically verify)

From the previously documented vendor GPIO directions when the board is upright in portrait: joystick raw LEFT=GP16, RIGHT=GP17, UP=GP19, DOWN=GP20, CENTER=GP18. Rotating the front-view hardware **clockwise 90°** changes the operator-facing directions:

| Operator gesture in final landscape | Raw board direction | Pico GPIO |
|---|---|---|
| UP | Original LEFT | GP16 |
| DOWN | Original RIGHT | GP17 |
| LEFT | Original DOWN | GP20 |
| RIGHT | Original UP | GP19 |
| CLICK | CENTER (unchanged) | GP18 |

Use one central `raw_gpio -> logical_button` transform before UI navigation. Existing input verification established **each of 9 raw controls works**, not that the direction semantics in this new orientation have already been physically verified. Test all four joystick directions again when this layout is flashed.

Buttons: K1=GP8, K2=GP4, K3=GP3, K4=GP2; keep the direct page mapping, now in a physical top-to-bottom row at the **right of the display**.

## Mandatory direct access — the four buttons

| Key | Screen | Primary content |
|---|---|---|
| K1 | SOMATIC | Actual loop rate, timing/jitter, missed deadlines and a compact chart |
| K2 | CHRONONS | Chronon timeline, source and detail-on-click |
| K3 | IMU | Sensor-frame attitude, rate/acceleration and trend chart |
| K4 | SYSTEM | Configured/observed motors, CAN health, measured power and host/device status |

Every K key always opens its designated **base** screen, including when a detail modal is open. No K key may cause motor actuation or modify hardware.

## Common layout and visual system (128×128 square, landscape-mounted)

- Top header, y=0..15: `RGD / [PAGE]` and a compact source state `LIVE` / `DEMO` / `STALE` / `N/A`.
- Main region, y=16..110: 1–2 hero numbers, at most **one primary chart** or one timeline at once, context microcopy; no horizontally scrolling prose.
- Bottom navigation strip, y=111..127: `1 LOOP · 2 TIME · 3 IMU · 4 SYS` via compact icons/dots if text doesn't fit; active tab clearly distinguished.
- Dark navy background, white large digits, cyan data, amber uncalibrated/demo, green verified/fresh, red alert/unavailable. RGB565 quantization and actual ST7735S RGB/BGR/inversion/offset must be calibrated on the physical panel before palette acceptance.
- Target fonts: compact fixed 5×7 or 6×10 for labels, larger ~14–22 px high numerals, line and icon strokes >=1 px; avoid relying only on color for meaning.
- No infinite animations or hidden "working" indicators suggesting robot safety. Priority order: **validity / age > metric > chart > decoration**.
- When USB stops delivering frames, retain UI navigation, mark affected data `STALE` and avoid continuing an apparently live animation of telemetry. A lost USB port is not proof motors are stopped or safe.

### Landscape-operator wireframes (schematic, not pixel-perfect)

**Read each center display upright with the joystick physically on the left and K1..K4 on the right**. These are four interchangeable UI contents on a square LCD mounted in a wide, hand-operated PCB.

```text
     JOY          K1 / SOMATIC            KEYS       JOY          K2 / CHRONONS           KEYS
                   ┌────────────────┐      K1                     ┌────────────────┐      K1
      ↑            │RGD/LOOP   DEMO │      K2       ↑             │RGD/TIME   DEMO │      K2
    ← ● →          │    --- Hz      │      K3     ← ● →           │> OBS  -0.2s    │      K3
      ↓            │TGT 1000 Hz     │      K4       ↓             │  PLAN -0.7s    │      K4
                   │_╱╲__╱╲____     │                             │  ERR  -1.4s    │
                   │J --µs MISS --  │                             │ID --  CLICK ↵  │
                   │ ● ○ ○ ○        │                             │ ○ ● ○ ○        │
                   └────────────────┘                             └────────────────┘

     JOY          K3 / IMU                KEYS       JOY          K4 / SYSTEM             KEYS
                   ┌────────────────┐      K1                     ┌────────────────┐      K1
      ↑            │RGD/IMU    RAW  │      K2       ↑             │RGD/SYS    LIVE │      K2
    ← ● →          │ROLL    --.-°   │      K3     ← ● →           │MOTORS  22 CFG  │      K3
      ↓            │PITCH   --.-°   │      K4       ↓             │ONLINE  --      │      K4
                   │ [sensor axes]  │                             │CAN     --      │
                   │GYRO -- °/s     │                             │DRIVE   --.- V  │
                   │ ○ ○ ● ○        │                             │ ○ ○ ○ ●        │
                   └────────────────┘                             └────────────────┘
```

The right-side physical buttons **are not on-screen touch targets**: K1..K4 is the one-to-one, immediately accessible navigation. On-screen bottom four dots are optional current-page indicators. The main chart/list is always upright in the joystick-left, keys-right orientation.

Numbers above are **illustrative** (e.g. IMU -4.6°, 1000 Hz target), never current measured values. When a value has no accepted live source, use `--` rather than a plausible number.

## K1 — SOMATIC / loop microscope

**Hero:** *measured* observed loop frequency (Hz) with data age; separately show **TARGET 1000 Hz** (a configured engineering goal, not a performance claim). Secondary metrics: p95 timing jitter (µs), deadline misses, maximum observed execution budget (if instrumented). The graph shows one selectable metric at a time (Hz, jitter, duration) and a subtle target line; chart points come from actual host/runtime instrumentation, **not** USB arrival intervals.

Controls in 0.2B: UP/DOWN changes plotted metric; LEFT/RIGHT cycles 5s / 15s / 30s illustrative history windows; joystick CLICK enters **focus/zoom**. While focused, UP/DOWN cycles x1/x2/x4/x8 and LEFT/RIGHT pans within the graph history; CLICK closes the focus. This never pauses the real runtime. When not instrumented, label `NO LOOP SOURCE`, don't calculate or invent jitter. All waveforms in this implementation are expressly **DEMO fixtures**, not real historical samples.

## K2 — CHRONONS / timeline and detail

A compact, newest-first list with 3–5 rows of **real referenced Chronons** from an approved HyperAion/Chronograph adapter. Each row carries relative age, short type tag (OBS/PLAN/ACT/ERR), truncated summary, and tiny type mark. UP/DOWN selects older/newer records; LEFT/RIGHT cycles an `ALL / OBS / PLAN / ACTION / ERROR` filter; CLICK opens selected detail; CLICK again closes it.

Detail view: short stable ID and type, recorded source/observer identity **where actually present**, source event time versus host receipt time, a 1–2 line summary and optional relations/count. The host stores and paginates full text/data; the Pico receives a bounded preview. A "thought" means an **explicit recorded decision summary**; the display must never invent introspective reasoning or suggest it can expose a model's private chain of thought. If Chronon source is unavailable, display `NO CHRONON FEED`. In demonstration mode use `DEMO` watermark on each synthetic entry.

Selecting a record may send a non-actuating `chronon.inspect` UI request to the host/5-inch Console in a later wave, never a motor command.

## K3 — IMU / uncalibrated sensor-frame laboratory

**Hero:** roll/pitch values in degrees from the sensor's declared frame, `RAW / SENSOR XYZ` badge. Small 2D crosshair or tilt/vector widget **must say SENSOR**, not `robot balanced` or `world horizon`, until the mounting extrinsic is physically verified and calibrated.

Additional: gyro magnitude or selected X/Y/Z axis (°/s), acceleration vector norm (g or m/s²), temperature (only when corresponding WIT field actually exists), optional Euler yaw marked `YAW* UNCAL`. Primary chart: selectable 10s/30s roll, pitch, angular speed trend. UP/DOWN selects signal; LEFT/RIGHT changes graph window; CLICK enters a larger **DEMO chart focus**. While focused, UP/DOWN changes x1/x2/x4/x8 zoom and LEFT/RIGHT pans. Verified live sensor-axis details remain an adapter wave, not part of this fixture.

**Existing Paky-01 source:** the `italiarobotica/paky-01` branch `feat/imu-console-bridge` already contains passive WIT/JY901 UART decoding and candidate Console observation publishing around 10Hz. It publishes sensor-frame readings and host *receive timestamps* to the loopback Console API. Do **not** open the same CH340 `/dev/ttyUSB0` independently from Pulse: one serial reader at a time, and the CH340 port is not Pulse's `ttyACM`. Pulse must subscribe to a nonblocking host-side fan-out from an approved Console/IMU service adapter; avoid duplicating a sensor process. Current transform/quaternion and yaw calibration are missing. These readings are observations only, never authorized for motor control.

## K4 — SYSTEM / physical and host inventory

4–6 compact tiles; order by criticality:
1. `MOTORS 22 CFG` from BHL V1 **reference only**; display `ONLINE --` until a verified, read-only and authorized bus enumeration is available. `22/22 ONLINE` must be shown only if all 22 expected, mapped and observed devices are verified in the same scan.
2. `CAN --` per-bus presence/error freshness, only if a published safe read-only SocketCAN diagnostics source exists. No CAN scans/commands automatically initiated by K4.
3. `DRIVE --.- V`, `AUX12 --.- V`, `AUX5 --.- V` **only from measured and identified rails** with acquisition time and provenance; never infer measured supply from a generic actuator's historical telemetry.
4. `HOST TEMP --`, CPU availability, USB link state and freshness, MCU firmware version/runtime.
5. Optional system snapshot mini-chart: measured drive voltage history or aggregate per-joint health strip when actual named joint IDs are bound and fresh.

**Current truth:** `paky-01/main` `electronics/can/joint_nodes.csv` has 22 `NOT_MAPPED` records, and `electronics/power/topology.json` declares `DRAFT_NOT_VERIFIED` with null as-built voltages. Therefore the initial honest screen is **22 configured, online unknown, voltages unknown**. Avoid displaying `0/22` when there was no bus scan, and `24.0V` based only on rail naming.

## Input mapping

| Page | UP/DOWN | LEFT/RIGHT | CLICK |
|---|---|---|---|
| K1 SOMATIC | Select Hz/jitter/execution DEMO chart | Choose 5/15/30 s window | Enter/exit chart focus; in focus UP/DOWN zoom x1..x8 and LEFT/RIGHT pan |
| K2 CHRONONS | Select older/newer entry | Filter ALL/OBS/PLAN/ACT/ERR | Open/close Chronon detail |
| K3 IMU | Select roll/pitch/gyro/accel fixture view | History 10/20/30 s | Enter/exit chart focus; in focus UP/DOWN zoom x1..x8 and LEFT/RIGHT pan |
| K4 SYSTEM | Select motors/CAN/power/host tile | Move to previous/next subpanel | Expand selected tile / back |

Debounce inputs and ensure K1..K4 always escape overlays. No joystick input can become a robot-motion or actuator command.

## Performance and architecture boundary

- Present panel is 128×128 RGB565: full-frame RGB565 memory is **32,768 bytes** out of RP2040's 264 KiB SRAM. Avoid unbounded allocations or large text caches.
- Target telemetry summary delivery 5–10 Hz, display refresh 5–10 Hz at most and **dirty rectangle** redraws rather than expensive full-screen bit-bang loops. Prefer ST7735S hardware SPI (and DMA where validated) to the current software SPI for responsive rich graphics. Buttons must stay reactive during USB frame handling.
- High-frequency Somatic 1kHz control runs only on the host/runtime; Pico never schedules it. Host aggregator exports decimated, bounded, optional summaries. Jitter is computed at the real loop instrumentation source.
- Store compact bounded history on MCU (e.g., 60 samples per active chart and 16–32 Chronon *summaries*); full Chronons, raw telemetry and replay reside on the host.
- Preserve PULSE/1 USB HELLO/DEMO/ACK and DTR session-reopen behavior. Do not repurpose current 18-byte `DemoTicks` frames to silently carry real sensor data or long Chronon text. Introduce separately versioned candidate typed UI messages, with bounded payloads and explicit capability negotiation.
- Source envelope on every metric: `source_id`, `observed_at_monotonic` or explicitly identified receipt time, `data_origin` (REAL/SYNTHETIC), `quality` (VERIFIED/UNCALIBRATED/UNKNOWN), `freshness`, optional frame/calibration refs. Chronon IDs and times must not be fabricated.
- K2 may return to a cached timeline when the host goes offline, visibly marked `OFFLINE/CACHED`, not new live events.

## MVP acceptance gates (not yet met)

1. Visible legible headings, icons, numbers and chart pixels; calibrated RGB/BGR, inversion and column/row offsets.
2. In the **joystick-left / K1–K4-right** physical landscape pose, text and chart axes are upright; all four K keys switch screens immediately; physical joystick UP/DOWN/LEFT/RIGHT follows the orientation mapping above and detail dismissal works, without reflash/reboot during navigation.
3. DEMO datasets available offline for all four screens, *visibly marked DEMO* at all times.
4. Real read-only IMU bridge consumes existing fan-out without serial-reader contention; sensor-frame labels and calibration warning remain visible.
5. Missing system numbers show `--`, not zero, guessed voltages or 22/22 online.
6. USB disconnection marks streams stale without freezing buttons/menu.
7. 100/100 synthetic ACK regression and 3/3 DTR session reopen stay green after UI changes.
8. Future real loop and Chronon adapters are tested independently; no claim of 1kHz actual performance, Chronograph synchronization, motor safety or active actuation.

## Orientation design decision

- **LOCKED (operator request 2026-10-24):** Chronoscope HMI is physically landscape, joystick **LEFT**, four keys **RIGHT**; physically rotate the board +90° clockwise from the vendor portrait view, and compensate graphics -90° in native pixel coordinates (or equivalent validated MADCTL rotation), so content is upright.
- Hardware test gate: verify physical clockwise mounting, K1..K4 top-to-bottom, calibrated MADCTL and raw-to-logical joystick axes before claiming the landscape implementation is complete.

## 0.2B implementation proposal (stacked branch)

The next branch `feat/pulse-0.2b-chronoscope-four-pages` adds `pulse-ui` navigation state with host unit tests, candidate hardware SPI LCD renderer and four on-device pages. It is still awaiting real 0.2B physical acceptance and does **not** read canonical Somatic/Chronon/IMU/voltage data. Zoom/pan acts on explicitly synthetic illustration samples.

## Decisions awaiting design sign-off

- Four top-level pages locked to K1..K4? **Recommended yes.**
- Joystick click for detail/back? **Recommended yes.**
- K2 list of chronological *verified source events* before arbitrary "thought" transcripts? **Recommended yes.**
- K3 scientific *sensor axes* visualization until RGD-Calibration validates sensor-to-torso frame? **Recommended yes.**
- K4 inventory truth levels (`CFG`, `OBSERVED`, `UNKNOWN`) instead of naive `22/22`? **Recommended yes.**
- Color palette and font legibility on physical display? **Verify via visual commissioning.**
- UI protocol version/shape? **Candidate needed; no canonical contract change without review.**

**No firmware modifications, motor commands or boot/flash steps are authorized by this design document.**
