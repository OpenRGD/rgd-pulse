# PULSE-0.4A — K4 SYSTEM read-only telemetry candidate

**Status:** SOURCE ONLY, 18 November 2026. New firmware has not been flashed or physically accepted.
**Base:** merged PULSE-0.3C. Physical freeze: release/pulse-0.3c-physical-20261027.
**Scope:** physical Linux host observations; informational, never a robot controller.

## What K4 can honestly display

| K4 field | Observation source | Meaning |
| --- | --- | --- |
| Pico USB | RP2040 CDC/DTR session | Open link; not robot readiness |
| Linux RAM used (%) | /proc/meminfo MemTotal and MemAvailable | Actual host kernel memory accounting, not container memory |
| Linux host uptime (s) | /proc/uptime | Physical host OS kernel uptime, not process uptime |
| Console HTTP | GET http://127.0.0.1:8088/healthz | HTTP 200 + ok:true + api_version + actuation_enabled:false; only Console process health |
| Motor online | None | N/A, not inferred from configured motors |
| CAN state | None | N/A, no CAN bus access |
| Drive voltage | None | N/A, no physical voltage sensor |

All host data must be sampled on the actual Ubuntu mini-PC outside Docker. The Console operates in a container; its /host endpoint is not proof of physical host metrics. Console /healthz can establish an API process response but **not the readiness or safety of Paky-01**. Missing/invalid Console response is unverified (N/A), not robot OFF.

## Safe transport

PULSE/SYS-1: a separate 18-byte, fixed-size, no_std packet with SY magic, version 1, CRC16-CCITT-FALSE and Probe/ProbeAck before host data. No motor, sensor-configuration, CAN, firmware-flashing, security-bypass or actuation command exists.

K4 fresh host observations expire after 2500 ms. In STALE, the most recent sample is visually muted and explicitly labelled stale. Closing USB CDC/DTR clears all host data to N/A. Samples are never written into the reserved preference journal.

The prior RP/PULSE-1 and IM/PULSE-IMU-1 decoders remain independent. Every new SYS session requires a new capability handshake and valid sequence/CRC; older Pico firmware rejects SYS rather than silently accepting arbitrary data.

## Critical v0.4A limitation: one Pico USB owner

The original imu-live and new system-live CLIs both open the SAME Pico CDC port exclusively. **Do not run them concurrently**. This wave validates K4 alone; PULSE-0.4B will design a single multiplexed host bridge for simultaneous K3+K4. System-live does not open WIT/CH340, CAN, motor, power or any other serial device.

## Read-only source-only preflight (no USB)

    cd ~/openrgd/worktrees/rgd-pulse-system04a
    cargo test --workspace
    cargo fmt --all -- --check
    cargo run -p pulse-bridge -- system-preview

Expected output contains Linux host uptime, memory used percent and Console /healthz status. If the Console is not up, health is N/A; never auto-start or change any robot service to create an apparently green status.

## Build only — no flash

    cd ~/openrgd/worktrees/rgd-pulse-system04a/firmware/pico-w
    cargo build --release
    elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w-0.4a-candidate.uf2
    python3 ../../scripts/check-pulse-uf2-prefs.py pulse-pico-w-0.4a-candidate.uf2

Do not load new UF2 until host and Pico CI green and verified hardware test conditions. Never mass erase the protected flash preference region 0x101FE000..0x10200000 or flash any STM32/ST-LINK device. The accepted PULSE-0.3C image remains a safe fallback.

## Physical acceptance plan (not yet run)

1. Disconnect motor power. Verify standard RP2040 Pico W identity/BOOTSEL and uniquely mounted RPI-RP2. Back up the journal read-only using picotool. Flash only the safely checked candidate.
2. Cold boot: confirm LCD X=1 Y=2, validated 0x68 INVOFF colors and persisted Italia/K3.
3. Close existing imu-live, serial-demo and monitor clients gracefully; do not stop unrelated services or claim exclusive control over the WIT/CH340 IMU.
4. Start one bounded test in an independent terminal:

       cd ~/openrgd/worktrees/rgd-pulse-system04a
       PICO=/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00
       test -e "$PICO" || exit 1
       cargo run -p pulse-bridge -- system-live "$PICO" 45

5. Expect PULSE/1 HELLO OK and PULSE/SYS-1 PROBE/ACK OK, then 1 Hz real Linux host snapshots; verify K4 HOST memory/uptime and Console health. Motors, CAN and drive voltage MUST remain N/A.
6. Verify a documented sender-pause case produces STALE, and closing USB CDC produces N/A. Do not infer precisely timed stale transitions solely from closing the USB port.
7. Run 0.3C PULSE/1 synthetic 20/20 and 100/100 ACK after system-live exits; separately verify the original IMU-only live K3 and STALE. Never run simultaneous Pico serial clients.

## Acceptance gates

- [x] Host unit tests and rustfmt green (GitHub Actions 4/4 green on `e25dbce3`)
- [x] Pico CI release build, BOOT2/vector checks and protected-flash gates green (local UF2 check still pending)
- [x] SYS corruption/magic/flags/bounds/replay/freshness unit tests green
- [x] Physical mini-PC Ubuntu Linux source preflight via `system-preview` — operator 2026-11-20
- [x] Real Pico K4 uptime/memory displayed correctly (operator; full SYS handshake logs and Console label verification not supplied)
- [ ] STALE/N/A stop test and full previous 0.3C regression
- [ ] Separate v0.4B multiplex design before concurrent K3+K4

CRC is not authentication. This only validates a trusted local prototype connection; it does not establish secure remote controls or motor safety. Never infer CAN, drive voltage or actual Somatic loop health from kernel uptime, API health, USB DTR or a configured motor inventory.


## Physical Ubuntu host preflight — 20 November 2026

**PASS (operator report, no Pico or sensor serial opened).**

The user completed the host-only `cargo run -p pulse-bridge -- system-preview`
on Paky-01's Ubuntu mini-PC, yielding:

```text
PULSE-0.4A HOST PREVIEW / ACTUAL LOCAL LINUX / NO USB / NO ACTUATION
host_uptime=266613s ram_used=37%
console_healthz_verified=true (NOT ROBOT READY)
motors=N/A can=N/A drive_voltage=N/A
=== HOST PREFLIGHT COMPLETE ===
```

This confirms local Linux `/proc/uptime` and `/proc/meminfo` parsing and
a successful validation of the fixed `127.0.0.1:8088/healthz` response.
The source represents physical host OS observations and Console process
reachability, **not** actual motor/CAN/voltage states or robot readiness.

Additional evidence: the current source head `e25dbce3f50b67030c1ce02c14d3e16a49d857bb`
has 4/4 successful GitHub Actions host/pico jobs; host tests and formatting,
Pico release ELF, BOOT2/vector and partition checks all passed.

**Not yet done:** local UF2 build/guard, protected preference-journal backup,
explicit BOOTSEL physical flash, K4 on-device visuals, SYS capability/ACK,
K4 STALE/N/A, K3 regression and 100/100 PULSE/1 ACK after flashing.
The existing physically accepted 0.3C firmware and frozen source branch
remain unchanged by this host-only test.

## On-device K4 operator verification — 21 November 2026

**Physical visual PASS (operator report):** following the PULSE-0.4A host-only
preflight, the operator confirms that the Chronoscope K4 SYSTEM displays
actual **Linux RAM used** and **host uptime** correctly. This demonstrates
the new source-to-Pico UI path functionally on the assembled hardware.

**Scope of evidence:** no complete `system-live` terminal transcript or
explicit PULSE/SYS-1 Probe/ACK counter was supplied for this second test.
The on-screen values require accepted SYS sample reception in the current
implementation, but exact timing, elapsed seconds, loss, STALE/N/A behavior,
or 100/100 USB and K3 IMU regressions **have not been independently
documented yet**. Do not infer that Console HTTP probe was separately
inspected on the LCD unless reported.

The physically accepted PULSE-0.3C release source remains frozen and
unmodified. For PULSE-0.4B, use one host process and one Pico USB CDC
session to avoid running the independent `system-live` and `imu-live`
programs concurrently.
