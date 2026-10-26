# Pulse rollout

| Wave | Deliverable | Acceptance |
|---|---|---|
| PULSE-0.1A | Versioned no_std frame, CRC, host observer, synthetic demo | Host unit tests; no real hardware or motion claims |
| PULSE-0.1B | Pico W display local bring-up and input diagnostics | Hardware photo, pinout verified, operator tests all 9 inputs |
| PULSE-0.1C | USB CDC bench handshake, stream framing/resync, explicit synthetic demo | Physical serial connect, stale detection, all 9 inputs operational during USB. Unique device ID deferred. |
| PULSE-0.1D | Somatic telemetry read-only adapter (future) | Measured jitter and deadline misses from runtime source, no blocking of control loop |
| PULSE-0.2A | Four-screen Chronoscope design | Operator-accepted joystick LEFT / K buttons RIGHT, DEMO-vs-real clarity |
| PULSE-0.2B | Four graphical dashboards and focus/zoom | Physical splash/pages/buttons observed; real telemetry sources deferred |
| PULSE-0.2C | Raw GPIO Input Lab and physical mapping | **PASS** on 2026-10-24: five operator joystick directions, four K pages, Chronon detail, chart zoom, synthetic CDC 20/20 ACK |
| PULSE-0.2D | Hold-to-repeat + skin engine | **Software candidate**: 440 ms initial hold, stepped acceleration, input-event feedback, four selectable skins via K2+K3 menu; physical acceptance pending |
| PULSE-0.2E (proposed) | Persistent per-device preferences + richer skin packaging | Versioned skin IDs and safe host preference sync, no arbitrary MCU code or unreviewed flash writes |
| PULSE-0.3C (in progress) | Durable Chronoscope preferences (skin + last K page), two-sector 8KiB CRC journal | host tests + Pico firmware CI + UF2 partition check, then physical cold-boot restoration + USB regressions; not accepted until physical PASS |\n| PULSE-0.2F (proposed) | Real telemetry source adapters | Approved read-only Somatic, Paky-01 IMU fan-out, source-linked Chronons; no invented sensor readings |
| PULSE-0.3 | Console discovery and configuration | Explicit compatibility contract and staging tests |

## Skin engine design gate (2026-10-24)

The first four ROM presets are **RGD Classic**, **Phosphor**, **Chrome 99** and **Italia**. The 0.2D picker is accessed by holding **K2+K3**, applying with joystick CLICK or cancelling with the same chord. The selected skin is **RAM-only for this release**, not persisted over power cycles. The source of truth for skin identity/style is `crates/pulse-ui/src/skins.rs`, with visual color roles in `firmware/pico-w/src/render.rs`.

Skins may change background, accent and decoration but **never** semantic warning colors, provenance labels, measurements, motor states or any hardware actions. A future OpenRGD Console settings surface may synchronize a stable skin ID only after an explicit non-actuating compatibility contract.

See `docs/PULSE-0.2D-SKINS-NAVIGATION.md` for input/UX timing and a hardware acceptance matrix.

## Authority / safety

Do not treat simulated telemetry as observed hardware measurements. Report link freshness and Somatic loop progress separately: an active bridge is not evidence that the control loop advanced. A Pico heartbeat cannot substitute for certified motor safety/E-stop. Device controls are diagnostic-only.

## Decisions

1. Keep OpenRGD standard independent of this implementation.
2. Keep physical Paky-01 wiring and evidence in italiarobotica/paky-01.
3. Separate host and embedded Cargo workspaces to avoid accidentally building Cortex-M firmware for the host.
4. `pulse-bridge demo` is host-only synthetic; `serial-demo` sends explicit synthetic frames to an identified Pico W via USB CDC. No real Somatic Runtime adapter is present.
5. Operator-reported PULSE-0.1C.1 DTR reopening passed 3/3 sessions (60/60 ACK); later 100-frame and hardware re-enumeration evidence were also collected. The subsequent 0.2C build passed 20/20 ACK on the real assembled Chronoscope. These are synthetic transport tests, **not** Somatic loop timings or actuator-safety certifications.
