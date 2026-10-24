# PULSE-0.2A — Chronoscope UI / four-screen design candidate

**Status:** DESIGN PROPOSAL, not firmware. Requires operator approval and data-contract review before the 0.2 implementation. This document does not authorize robot motion or change the canonical OpenRGD schemas.

## Goal

Build a high-quality, legible observability HMI on the real **Raspberry Pi Pico W (RP2040)** and **Seengreat Rev 1.2 ST7735S 128×128 SPI TFT**. Keep it autonomous and diagnostic-only. The main 5-inch Paky-01 screen remains the rich Console UI. The 1.44-inch Pulse LCD is a compact *window into time, sensors and host/runtime health*.

The current firmware has already shown real LCD graphics, a blinking indicator, 9 hardware inputs and USB CDC with synthetic HELLO / DEMO / ACK. It does NOT currently display real Somatic loop metrics, verified Chronons, IMU data or motor/voltage readings.

## Mandatory direct access — the four buttons

| Key | Screen | Primary content |
|---|---|---|
| K1 | SOMATIC | Actual loop rate, timing/jitter, missed deadlines and a compact chart |
| K2 | CHRONONS | Chronon timeline, source and detail-on-click |
| K3 | IMU | Sensor-frame attitude, rate/acceleration and trend chart |
| K4 | SYSTEM | Configured/observed motors, CAN health, measured power and host/device status |

Every K key always opens its designated **base** screen, including when a detail modal is open. No K key may cause motor actuation or modify hardware.

## Common layout and visual system (128×128)

- Top header, y=0..15: `RGD / [PAGE]` and a compact source state `LIVE` / `DEMO` / `STALE` / `N/A`.
- Main region, y=16..110: 1–2 hero numbers, at most **one primary chart** or one timeline at once, context microcopy; no horizontally scrolling prose.
- Bottom navigation strip, y=111..127: `1 LOOP · 2 TIME · 3 IMU · 4 SYS` via compact icons/dots if text doesn't fit; active tab clearly distinguished.
- Dark navy background, white large digits, cyan data, amber uncalibrated/demo, green verified/fresh, red alert/unavailable. RGB565 quantization and actual ST7735S RGB/BGR/inversion/offset must be calibrated on the physical panel before palette acceptance.
- Target fonts: compact fixed 5×7 or 6×10 for labels, larger ~14–22 px high numerals, line and icon strokes >=1 px; avoid relying only on color for meaning.
- No infinite animations or hidden "working" indicators suggesting robot safety. Priority order: **validity / age > metric > chart > decoration**.
- When USB stops delivering frames, retain UI navigation, mark affected data `STALE` and avoid continuing an apparently live animation of telemetry. A lost USB port is not proof motors are stopped or safe.

### Layout wireframes (schematic, NOT 128px-accurate artwork)

```text
 K1 SOMATIC                    K2 CHRONONS
 +------------------+          +------------------+
 | RGD / LOOP  DEMO  |          | RGD / TIME  LIVE |
 |    --- Hz         |          | >● -0.2s OBS     |
 | TARGET 1000 Hz    |          |  ● -0.7s PLAN    |
 | J95 --us  MISS -- |          |  ● -1.4s EVENT   |
 |   _/\_/\___/\     |          |  ● -2.1s FAULT   |
 |  /          \     |          | ID: --           |
 |     5s / 30s      |          | ● Time navigator |
 | 1  [2]  3   4     |          | 1 [2] 3   4      |
 +------------------+          +------------------+

 K3 IMU                        K4 SYSTEM
 +------------------+          +------------------+
 | RGD / IMU  RAW   |          | RGD / SYS   LIVE |
 | R -4.6°  P -4.8° |          | MOTORS 22 CFG    |
 |     +------+     |          | ONLINE  --       |
 |     |  x   |     |          | CAN     --       |
 |     +------+     |          | DRIVE --.- V     |
 | |gyro| --- °/s   |          | AUX12  --.- V    |
 |  __/\___/\__     |          | USB CDC ●        |
 | YAW* --° UNCAL   |          | TEMP --°C        |
 | 1 2 [3] 4        |          | 1 2 3 [4]        |
 +------------------+          +------------------+
```

Numbers above are **illustrative** (e.g. IMU -4.6°, 1000 Hz target), never current measured values. When a value has no accepted live source, use `--` rather than a plausible number.

## K1 — SOMATIC / loop microscope

**Hero:** *measured* observed loop frequency (Hz) with data age; separately show **TARGET 1000 Hz** (a configured engineering goal, not a performance claim). Secondary metrics: p95 timing jitter (µs), deadline misses, maximum observed execution budget (if instrumented). The graph shows one selectable metric at a time (Hz, jitter, duration) and a subtle target line; chart points come from actual host/runtime instrumentation, **not** USB arrival intervals.

Controls: UP/DOWN changes plotted metric; LEFT/RIGHT cycles 5s / 30s / 60s history; joystick CLICK toggles a frozen *view only* (does not pause the runtime). Read-only. When not instrumented, label `NO LOOP SOURCE`, don't calculate or invent jitter.

## K2 — CHRONONS / timeline and detail

A compact, newest-first list with 3–5 rows of **real referenced Chronons** from an approved HyperAion/Chronograph adapter. Each row carries relative age, short type tag (OBS/PLAN/ACT/ERR), truncated summary, and tiny type mark. UP/DOWN selects older/newer records; LEFT/RIGHT cycles an `ALL / OBS / PLAN / ACTION / ERROR` filter; CLICK opens selected detail; CLICK again closes it.

Detail view: short stable ID and type, recorded source/observer identity **where actually present**, source event time versus host receipt time, a 1–2 line summary and optional relations/count. The host stores and paginates full text/data; the Pico receives a bounded preview. A "thought" means an **explicit recorded decision summary**; the display must never invent introspective reasoning or suggest it can expose a model's private chain of thought. If Chronon source is unavailable, display `NO CHRONON FEED`. In demonstration mode use `DEMO` watermark on each synthetic entry.

Selecting a record may send a non-actuating `chronon.inspect` UI request to the host/5-inch Console in a later wave, never a motor command.

## K3 — IMU / uncalibrated sensor-frame laboratory

**Hero:** roll/pitch values in degrees from the sensor's declared frame, `RAW / SENSOR XYZ` badge. Small 2D crosshair or tilt/vector widget **must say SENSOR**, not `robot balanced` or `world horizon`, until the mounting extrinsic is physically verified and calibrated.

Additional: gyro magnitude or selected X/Y/Z axis (°/s), acceleration vector norm (g or m/s²), temperature (only when corresponding WIT field actually exists), optional Euler yaw marked `YAW* UNCAL`. Primary chart: selectable 10s/30s roll, pitch, angular speed trend. UP/DOWN selects signal; LEFT/RIGHT changes graph window; CLICK opens per-axis numeric details/calibration status.

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
| K1 SOMATIC | Select Hz/jitter/execution graph | Choose 5/30/60 s | Freeze/unfreeze display snapshot, *not runtime* |
| K2 CHRONONS | Select older/newer entry | Filter ALL/OBS/PLAN/ACT/ERR | Open/close Chronon detail |
| K3 IMU | Select roll/pitch/gyro/accel view | History 10/30 s | Sensor-axis detail / back |
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
2. All four K keys switch screens immediately; joystick mapping and detail dismissal work; no reflash or reboot required for navigation.
3. DEMO datasets available offline for all four screens, *visibly marked DEMO* at all times.
4. Real read-only IMU bridge consumes existing fan-out without serial-reader contention; sensor-frame labels and calibration warning remain visible.
5. Missing system numbers show `--`, not zero, guessed voltages or 22/22 online.
6. USB disconnection marks streams stale without freezing buttons/menu.
7. 100/100 synthetic ACK regression and 3/3 DTR session reopen stay green after UI changes.
8. Future real loop and Chronon adapters are tested independently; no claim of 1kHz actual performance, Chronograph synchronization, motor safety or active actuation.

## Decisions awaiting design sign-off

- Four top-level pages locked to K1..K4? **Recommended yes.**
- Joystick click for detail/back? **Recommended yes.**
- K2 list of chronological *verified source events* before arbitrary "thought" transcripts? **Recommended yes.**
- K3 scientific *sensor axes* visualization until RGD-Calibration validates sensor-to-torso frame? **Recommended yes.**
- K4 inventory truth levels (`CFG`, `OBSERVED`, `UNKNOWN`) instead of naive `22/22`? **Recommended yes.**
- Color palette and font legibility on physical display? **Verify via visual commissioning.**
- UI protocol version/shape? **Candidate needed; no canonical contract change without review.**

**No firmware modifications, motor commands or boot/flash steps are authorized by this design document.**
