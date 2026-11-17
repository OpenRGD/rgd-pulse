# Pulse rollout

**Baseline release:** [PULSE-0.3C physical acceptance and source freeze](RELEASE-PULSE-0.3C.md). Consolidated on 2026-11-17; frozen tested firmware source: `release/pulse-0.3c-physical-20261027`. New features must go to subsequent branches.

| Wave | Deliverable | Acceptance |
|---|---|---|
| PULSE-0.1A | Versioned no_std frame, CRC, host observer, synthetic demo | Host unit tests; no real hardware or motion claims |
| PULSE-0.1B | Pico W display local bring-up and input diagnostics | Hardware photo, pinout verified, operator tests all 9 inputs |
| PULSE-0.1C | USB CDC bench handshake, stream framing/resync, explicit synthetic demo | Physical serial connect, stale detection, all 9 inputs operational during USB. Unique device ID deferred. |
| PULSE-0.1D | Somatic telemetry read-only adapter (future) | Measured jitter and deadline misses from runtime source, no blocking of control loop |
| PULSE-0.2A | Four-screen Chronoscope design | Operator-accepted joystick LEFT / K buttons RIGHT, DEMO-vs-real clarity |
| PULSE-0.2B | Four graphical dashboards and focus/zoom | Physical splash/pages/buttons observed; real telemetry sources deferred |
| PULSE-0.2C | Raw GPIO Input Lab and physical mapping | **PASS** on 2026-10-24: five operator joystick directions, four K pages, Chronon detail, chart zoom, synthetic CDC 20/20 ACK |
| PULSE-0.2D | Hold-to-repeat + skin engine | Integrated into physically accepted 0.3C: joystick and four skins verified; no instrumented input timing benchmark. |
| PULSE-0.2E (superseded) | Persistent per-device applied skin/last page | Implemented and physically verified in 0.3C two-sector CRC journal. Richer visual packaging and host preference synchronization remain future reviewed work. |
| PULSE-0.3B | Console WS -> Rust Pulse Bridge -> USB PULSE/IMU-1 -> K3 real IMU sensor-frame values | **Physical PASS 2026-10-27** on 0.3C Pico: PULSE/1 HELLO, PULSE/IMU-1 PROBE/ACK, real K3 readings and RAW→STALE on publisher stop. Console null-after-TTL and end-run counts not separately captured; sensor axes remain uncalibrated. |
| PULSE-0.3C | Pico skin/last-K-page persistence (2×4KiB CRC flash journal), on-device PREFS/LCD diagnostics, default X=1 Y=2 | **Physical PASS 2026-10-27**: Italia+K3 restored; LCD/colours clean; 100/100 synthetic USB ACK with 0 unexpected. IMU source integration is a separate PULSE-0.3B acceptance. |
| PULSE-0.2F (remaining) | Other real telemetry adapters | Paky-01 IMU K3 fan-out delivered by 0.3B/0.3C. K1 SOMATIC, K2 Chronons and K4 SYSTEM source adapters still require separate review. |
| PULSE-0.4 (proposed) | Read-only host/peripheral K4 SYSTEM first, then other source adapters | Strict source identity, missing-data N/A, authenticated operator boundary where needed; new PR + new physical acceptance. |

## Skin engine design gate (2026-10-24)

The first four ROM presets are **RGD Classic**, **Phosphor**, **Chrome 99** and **Italia**. The skin picker is accessed by holding **K2+K3**, applying with joystick CLICK or cancelling with the same chord. **Historical 0.2D behavior was RAM-only; in physically accepted 0.3C the applied skin persists across power cycles together with the last K page**. Menu previews, Color Lab and transient focus remain RAM-only. The source of truth for skin identity/style is `crates/pulse-ui/src/skins.rs`, with visual color roles in `firmware/pico-w/src/render.rs`.

Skins may change background, accent and decoration but **never** semantic warning colors, provenance labels, measurements, motor states or any hardware actions. A future OpenRGD Console settings surface may synchronize a stable skin ID only after an explicit non-actuating compatibility contract.

See `docs/PULSE-0.2D-SKINS-NAVIGATION.md` for input/UX timing and a hardware acceptance matrix.

## Authority / safety

Do not treat simulated telemetry as observed hardware measurements. Report link freshness and Somatic loop progress separately: an active bridge is not evidence that the control loop advanced. A Pico heartbeat cannot substitute for certified motor safety/E-stop. Device controls are diagnostic-only.

## Decisions

1. Keep OpenRGD standard independent of this implementation.
2. Keep physical Paky-01 wiring and evidence in italiarobotica/paky-01.
3. Separate host and embedded Cargo workspaces to avoid accidentally building Cortex-M firmware for the host.
4. `pulse-bridge demo` is host-only synthetic; `serial-demo` sends explicit synthetic frames to an identified Pico W via USB CDC. No real Somatic Runtime adapter is present.
5. Historical 0.1C.1 DTR reopening passed 3/3 sessions; 0.2C passed 20/20 synthetic ACK; final 0.3C passed 100/100 synthetic ACK (0 unexpected), verified K3 live physical IMU with PULSE/IMU-1 PROBE/ACK and publisher-stop STALE. Synthetic transport is **not** Somatic loop timing or actuator safety.
6. Preserve the operator-tested 0.3C source at `release/pulse-0.3c-physical-20261027`; consolidate history with merge commits and keep any new telemetry features on a distinct later branch.
