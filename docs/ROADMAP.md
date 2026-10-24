# Pulse rollout

| Wave | Deliverable | Acceptance |
|---|---|---|
| PULSE-0.1A | Versioned no_std frame, CRC, host observer, synthetic demo | Host unit tests; no real hardware or motion claims |
| PULSE-0.1B | Pico W display local bring-up and input diagnostics | Hardware photo, pinout verified, operator tests all 9 inputs |
| PULSE-0.1C | USB CDC bench handshake, stream framing/resync, explicit synthetic demo | Physical serial connect, stale detection, all 9 inputs operational during USB. Unique device ID deferred. |
| PULSE-0.1D | Somatic telemetry read-only adapter | Measured jitter and deadline misses, no blocking of control loop |
| PULSE-0.2 | Chronoscope | Referenced real Chronons, time provenance, no fabricated thoughts |
| PULSE-0.3 | Console discovery and configuration | Explicit compatibility contract and staging tests |

## Authority / safety

Do not treat simulated telemetry as observed hardware measurements. Report link freshness and Somatic loop progress separately: an active bridge is not evidence that the control loop advanced. A Pico heartbeat cannot substitute for certified motor safety/E-stop. Device controls are diagnostic-only.

## Decisions

1. Keep OpenRGD standard independent of this implementation.
2. Keep physical Paky-01 wiring and evidence in italiarobotica/paky-01.
3. Separate host and embedded Cargo workspaces to avoid accidentally building Cortex-M firmware for the host.
4. `pulse-bridge demo` is host-only synthetic; `serial-demo` sends explicit synthetic frames to an identified Pico W via USB CDC. No real Somatic Runtime adapter is present.
5. Operator-reported PULSE-0.1C.1 DTR reopening succeeded 3/3 sessions (60/60 CRC-confirmed ACK); keep the draft gate open for USB unplug/replug, 100-frame tests, LCD/input checks during traffic.
