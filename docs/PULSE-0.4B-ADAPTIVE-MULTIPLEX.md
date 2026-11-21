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
- [ ] Host physical resource usage measured under K1/K2, K3, K4 (CPU, memory, file handles); distinguish IMU publisher vs Pulse Bridge processes.
- [ ] Physical K3↔K4 switch with **one** Pico serial handle and real source observations.
- [ ] Disconnected/STALE regressions and PULSE/1 20/20+100/100 CRC ACK.
- [ ] No motor/CAN commands, no container-as-host metrics, no untrusted "healthy robot" claims.

## Future always-on deployment

After the complete physical acceptance, operate **one** managed host service for the multiplexed bridge with bounded restart/backoff and strict device identity, no sudo and no duplicate serial ownership. Do not auto-install or auto-enable in this candidate. Source acquisition (publisher) remains a separate, explicit service with its own lifecycle. No control surface should automatically start motors or sensors.
