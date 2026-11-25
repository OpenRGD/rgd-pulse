# PULSE-0.4B — One USB owner, visible-page telemetry

**Status:** source candidate, not physically flashed or accepted.
**Source parent:** PULSE-0.4A draft `feat/pulse-0.4-k4-system-readonly`.
**Physically accepted rollback:** `release/pulse-0.3c-physical-20261027`.

## Resource policy

| Resource | K1 SOMATIC / K2 CHRONONS | K3 IMU | K4 SYSTEM |
| --- | --- | --- | --- |
| Linux host sampler | OFF | OFF | ON, max 1 Hz |
| Console HTTP /healthz | OFF | OFF | ON, max 1 Hz |
| Pulse read-only IMU WS | closed | connected | closed |
| IMU USB forwarding | OFF | <=10 Hz | OFF |
| SYS USB forwarding | OFF | OFF | <=1 Hz |
| Pico redraw | on UI change | on new accepted IMU / status | on accepted SYSTEM / status |

The separately operated WIT/CH340 **IMU publisher must not be stopped automatically** when K3 is hidden; it may have other Console consumers. This optimization reduces the incremental work of the Chronoscope bridge, not independently owned sensor acquisition. With both K3/K4 hidden, only the bounded blocking serial wait for page changes remains.

## Safe protocol extension

- **Existing** PULSE/1 `RP` (HELLO, synthetic DEMO/ACK) and PULSE/IMU-1 `IM` (Probe/ACK, real sensor observations) remain separate and unmodified.
- PULSE/SYS-1 `SY` adds two **opt-in** non-actuating frames: `SubscribePage` (host -> Pico, request *only* UI observation) and `PageHint` (Pico -> host, page index 0..3 and monotonic sequence). Page hints use the same 18-byte CRC-checked SYS framing; no commands to change the page exist.
- Pico **never emits page hints for old clients**, because only `mux-live` subscribes. `imu-live` and `system-live` remain backward-compatible.
- Pico emits hints on visible page change and then every 1500ms, with **no LCD redraw or flash write**. Host discards duplicate/out-of-order hints; a missing hint for >5s disables both source forwarders until a fresh page is received.
- Every CDC session requires PULSE/1 HELLO, IMU Probe/ACK and SYS Probe/ACK; `mux-live` refuses to transmit observations to unknown/old firmware. Page hints carry only local UI selection, not robot status or authentication.
- The Pico K4 SystemView still expires after 2500ms and K3 ImuView after 800ms. Old samples remain marked STALE/muted when hidden; on USB DTR close samples clear to NO SOURCE.

## One host-owned USB device

The new command is:

```bash
cargo run -p pulse-bridge -- mux-live \
  /dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00 120
```

It opens exactly one Pico USB CDC port. It **never** opens the WIT/CH340 IMU serial, CAN bus, motor driver or voltage sensors. Serial port identity is validated; no guessing `ttyACM0`, no arbitrary host/URL for the Console. It uses the existing trusted-local, loopback-only Console WS solely while K3 is visible, not an authenticated public endpoint.

`imu-live` and `system-live` still exist for dedicated regressions; **never run any of these in parallel** with `mux-live`.

The PULSE/SYS-1 host read-only probe and Console /healthz are not robot readiness or motion/safety authority. All unsupported CAN, motors and drive voltage remain `N/A` on K4.

## Host-only tests

```bash
cd ~/openrgd/worktrees/rgd-pulse-mux04b
cargo test --workspace
cargo fmt --all -- --check
cargo run -p pulse-bridge -- system-preview
```

No Pico USB access is required for the tests. The host preview must report Linux RAM/uptime and Console health as before, not simulated/guessed motor telemetry.

## Firmware build-only verification

```bash
cd ~/openrgd/worktrees/rgd-pulse-mux04b/firmware/pico-w
cargo build --release
elf2uf2-rs \
  target/thumbv6m-none-eabi/release/pulse-pico-w \
  pulse-pico-w-0.4b-mux-candidate.uf2
python3 ../../scripts/check-pulse-uf2-prefs.py \
  pulse-pico-w-0.4b-mux-candidate.uf2
```

Do **not** flash until host + Pico CI, UF2 protection and physical device identity are independently verified. Before flashing standard RP2040 Pico W in BOOTSEL, use `picotool` read-only backup of the last 8KiB journal `0x101FE000..0x10200000`. Never mass-erase, flash STM32, touch ST-LINK/CAN or power motors.

## Physical acceptance matrix — pending

1. Preserve 0.3C source and the existing protected preferences; flash only reviewed 0.4B UF2 to a confirmed RP2040 Pico W. Verify 0.3C colors/MADCTL, LCD X=1 Y=2, Italia skin/K3 persistence and button mappings.
2. Close old Pico CDC serial clients, not the separately owned IMU publisher. If publisher is running, ensure it is **the only WIT serial owner**. If not, K4 should still work while K3 correctly reports no fresh source.
3. Start bounded `mux-live <exact-pico-by-id> 120`. Expect HELLO, IMU Probe/ACK, SYS Probe/ACK and page subscription. Keep this as the **sole** Pico serial client.
4. Select **K3**. Host log should say `PICO VIEW = K3`, then `Console read-only WS subscribed`, then `K3 IMU #...`. Real K3 sensor-frame values update; missing IMU data must become STALE/NO SOURCE rather than DEMO.
5. Select **K4**. Host log says `PICO VIEW = K4`, K4 Linux RAM/uptime and verified Console process status update at 1Hz; motors/CAN/drive voltage remain `N/A`.
6. Select **K1** then **K2** for at least 3s each. No additional K3/K4 source frames should be forwarded while hidden; the bridge remains connected and CPU should be low, not spin-looping.
7. Return K3 then K4. Each page refreshes without restarting the bridge, extra publisher, WIT serial reader or Pico CDC process.
8. Allow `mux-live` to exit, then run **separate** `serial-demo` 20/20 and 100/100 CRC ACK, and verify DTR reopen. Repeat real K3 and K4 standalone regressions only when Pico CDC is free.
9. Independently test source loss, WebSocket unavailable, invalid/replayed page hints and safe rollback to the frozen 0.3C if necessary.

## CI and physical gates

- [ ] Host tests and format; page hint CRC, invalid page, replay/timeout, source-selection tests.
- [ ] Pico BOOT2/vector and flash partition compile gates.
- [ ] Local UF2 address map below `0x101FE000`.
- [ ] Per-page CPU/memory and fd baselines measured separately; preliminary single mux-live process snapshot: 0.3% CPU, 3864 KiB RSS (22 Nov). No publisher comparison yet.
- [x] Physical K1↔K4 page hints and real K4 Linux RAM/uptime/Console in one Pico CDC process (22 Nov). K3 **WebSocket subscription only** verified; no forwarded IMU sample line in supplied logs.
- [ ] Disconnected/STALE regressions and PULSE/1 20/20+100/100 CRC ACK.
- [ ] No motor/CAN commands, no container-as-host metrics, no untrusted "healthy robot" claims.

## Future always-on deployment

After the complete physical acceptance, operate **one** managed host service for the multiplexed bridge with bounded restart/backoff and strict device identity, no sudo and no duplicate serial ownership. Do not auto-install or auto-enable in this candidate. Source acquisition (publisher) remains a separate, explicit service with its own lifecycle. No control surface should automatically start motors or sensors.

## Physical adaptive mux bench — 22 November 2026

**Operator-reported functional PASS for USB negotiation, page selection and K4 data.**
The actual mini-PC `pulse-bridge mux-live` process was observed at elapsed
`01:36` (within a bounded 120s session), using:

```text
PID 134989   %CPU 0.3   RSS 3864 KiB   ELAPSED 01:36
PULSE/1 HELLO OK
PULSE/IMU-1 PROBE/ACK OK
PULSE/SYS-1 PROBE/ACK OK
PULSE/SYS-1 page subscription requested (Pico decides current K page)
Adaptive policy: K3 IMU only when viewed, K4 Linux at 1Hz when viewed
PICO VIEW = K4
K4 HOST #1 uptime=430645s RAM=42% Console /healthz=OK
K4 HOST #2 uptime=430646s RAM=42% Console /healthz=OK
K4 HOST #3 uptime=430647s RAM=42% Console /healthz=OK
PICO VIEW = K1
PICO VIEW = K2
PICO VIEW = K3
Console read-only WS subscribed (K3 visible)
PICO VIEW = K4
K4 HOST #10 uptime=430665s RAM=41% Console /healthz=OK
K4 HOST #20 uptime=430675s RAM=42% Console /healthz=OK
PICO VIEW = K1
PICO VIEW = K2
PICO VIEW = K3
Console read-only WS subscribed (K3 visible)
PICO VIEW = K4
K4 HOST #30 uptime=430692s RAM=41% Console /healthz=OK
K4 HOST #40 uptime=430703s RAM=41% Console /healthz=OK
PICO VIEW = K1
PICO VIEW = K2
PICO VIEW = K3
Console read-only WS subscribed (K3 visible)
```

**Proven by this captured output:** three independent protocol negotiation
responses accepted; Pico-to-host page hints received for all four K pages;
read-only Console WS subscription initiated only for K3; K4 host Linux
uptime/RAM and `/healthz` sampled and forwarded; one Pulse Bridge process
remained active throughout the observed page switching. No concurrent Pico
serial reader is reported.

**Observed resource sample:** `%CPU 0.3` and `RSS 3864 KiB` (~3.77 MiB)
at elapsed `01:36`. This is a **single process snapshot**, not a controlled
per-page utilization comparison or proof of 24/7 stability. PID is session
ephemeral, not a contractual device identity.

**Open gates:** The pasted trace shows K3 WS subscriptions but **no**
`K3 IMU #... forwarded` lines. Therefore real IMU sample delivery in
`mux-live` remains **UNVERIFIED**; the separately managed IMU publisher
may have been stopped. Test a fresh Console IMU GET while K3 is visible,
then inspect bridge IMU summary. The trace does not include the final
`PULSE MUX SUMMARY`, a controlled source-stop `STALE/N/A` test, or
post-update `PULSE/1` serial-demo 20/100 ACK regressions. Do not merge
0.4B until these are addressed.

No claims of measured motor/CAN/drive voltage or motion readiness.
The accepted 0.3C source rollback branch is unchanged.


## PULSE-0.4B.1 — K3 source readiness & reproducible regression (25 Nov 2026)

**Scope of changes:** host Pulse Bridge diagnostics, read-only Console GET
preflight and offline tests only. The firmware already installed in the
Pico from the prior 0.4B test does **not** need to be reflashed for these
changes. `PULSE/1`, `PULSE/IMU-1`, `PULSE/SYS-1` wire frames, Pico
flash, hardware ownership, and the 0.3C rollback branch are unchanged.

### Why K3 showed a WebSocket but no IMU lines

The 22 Nov log proves the Console WS was **connected**, not that the
publisher had sent a fresh real IMU observation. No `K3 IMU #...` or
`PULSE MUX SUMMARY` was supplied. The missing publisher is a plausible
cause, **not verified root cause**; first inspect Console's physical
observation GET and whether the sole WIT/CH340 reader is running.

The mux now distinguishes:
- `Console WS ... awaiting physical samples`: subscriber connection only;
- `K3 IMU #...`: an accepted real Console sample sent over Pico USB;
- `K3 NO VERIFIED IMU SAMPLES after 4s`: K3 displayed without any
  accepted sample; warns once per K3 visit, does not auto-start publisher;
- `PULSE MUX SUMMARY ... K3_visits/N_empty/WS_imu_received/WS_connect_failures`:
  helps distinguish no source, rejected samples, and forwarding.

No source or failed validation automatically remains `STALE/N/A`, and K4
keeps working. Changing pages cannot turn a Console health check into
a sensor-health claim.

### Read-only preflight (no Pico port opened)

On the physical Ubuntu mini-PC, not in Docker:

```bash
cd ~/openrgd/worktrees/rgd-pulse-mux04b
git fetch origin feat/pulse-0.4b-adaptive-multiplex
git merge --ff-only origin/feat/pulse-0.4b-adaptive-multiplex
python3 scripts/pulse_mux_imu_preflight.py
```

Expected: `IMU_CONSOLE_STATUS=FRESH_CONSOLE_OBSERVATION` followed by
sequence and same-host observation age. A Console `null` yields
`NO_FRESH_SOURCE`; invalid schema/source/time/robot/motion claims fail
closed and exit nonzero. Preflight does **not** authenticate the WIT
device independently and does not open sensor serial or change publisher
lifecycle.

If `NO_FRESH_SOURCE`, inspect the independently managed Paky-01 IMU
publisher; do **not** spawn a second reader on CH340. Correct the
source-lifecycle problem before claiming multiplex K3 PASS.

### Bounded operator acceptance: K3 + K4 + PULSE/1

```bash
cd ~/openrgd/worktrees/rgd-pulse-mux04b
bash scripts/pulse-mux-acceptance.sh
```

This script fails before USB use if the Console cannot supply a fresh
claimed physical IMU observation; validates one by-id Pico CDC, refuses a
busy serial device, runs one `mux-live` for 120 seconds and records logs
in `~/openrgd/reports`. While running, select K3 for >=10s, then K4 for
>=5s, K1 and K2, and K3 again. It requires *nonzero* real IMU and K4
forwarding, actual K3/K4 page hints, and a final mux summary. Once
mux-live has closed the Pico serial, it runs **sequential** synthetic
20/20 and 100/100 PULSE/1 ACK regressions, with zero unexpected frames.

This test **never flashes the Pico**, opens WIT/CH340, installs a
service, starts a publisher, reads CAN, or issues motor commands. It is
an operator physical test, not an unattended background process.

Separate gates still remain: calibrated end-to-end rate/latency,
publisher-stop `RAW→STALE`, cold-boot preference retention after 0.4B,
long-run stability and a controlled per-page CPU/RAM comparison. Do not
merge PR #12/#13 solely on the basis of passing unit tests, preflight,
or one short mux session.

**Reality Ingress note:** [Pulse PR #14](https://github.com/OpenRGD/rgd-pulse/pull/14)
is coordination-only and does not alter this real IMU flow. RI evidence
refs cannot be promoted to K3 physical samples or History/Chronons.
