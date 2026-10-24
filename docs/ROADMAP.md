# Pulse rollout

| Wave | Deliverable | Acceptance |
|---|---|---|
| PULSE-0.1A | Versioned no_std frame, CRC, host observer, synthetic demo | Host unit tests; no real hardware or motion claims |
| PULSE-0.1B | Pico W display local bring-up and input diagnostics | Hardware photo, pinout verified, operator tests all 9 inputs |
| PULSE-0.1C | Bidirectional USB CDC, framing/resync, serial ID | Unplug/replug tested while Pico independently powered |
| PULSE-0.1D | Somatic telemetry read-only adapter | Measured jitter and deadline misses, no blocking of control loop |
| PULSE-0.2 | Chronoscope | Referenced real Chronons, time provenance, no fabricated thoughts |
| PULSE-0.3 | Console discovery and configuration | Explicit compatibility contract and staging tests |

## Authority / safety

Do not treat simulated telemetry as observed hardware measurements. Report link freshness and Somatic loop progress separately: an active bridge is not evidence that the control loop advanced. A Pico heartbeat cannot substitute for certified motor safety/E-stop. Device controls are diagnostic-only.

## Decisions

1. Keep OpenRGD standard independent of this implementation.
2. Keep physical Paky-01 wiring and evidence in italiarobotica/paky-01.
3. Separate host and embedded Cargo workspaces to avoid accidentally building Cortex-M firmware for the host.
4. The local LCD bring-up has no USB CDC protocol yet; `pulse-bridge demo` is a host-only fixture.
