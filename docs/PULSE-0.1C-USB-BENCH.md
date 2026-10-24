# PULSE-0.1C — Pico W USB CDC bench test

**Candidate implementation, requires physical verification.** This is a diagnostic-only connection, *not* a Somatic control channel.

## Verified by the operator on 2026-10-24 (PULSE-0.1B)

- Pico W firmware boots, gives three backlight pulses and draws stripes / a grid of nine rectangles.
- All four K1–K4 buttons and all five joystick directions including the center press respond individually.
- The controller displays a periodic blinking activity marker.
- Exact RGB/BGR order and pixel offset are still to be characterized.

These observations are **operator-reported**, not a physical certification or independent measurement.

## Operator-reported USB enumeration evidence (2026-10-24)

On Paky-01's Ubuntu mini-PC the operator successfully ran the PULSE-0.1C source branch through `cargo test --workspace` (**12/12 tests**: 4 pulse-core and 8 pulse-protocol), compiled `pulse-pico-w` for `thumbv6m-none-eabi` in release mode, and transferred the UF2 onto the `RPI-RP2` mount. After reboot, udev reported a new USB CDC `ttyACM` port with:

```text
ID_VENDOR_ID=c0de
ID_MODEL_ID=cafe
ID_MODEL=RGD_Pulse_Pico_W_prototype
```

A `/dev/serial/by-path/` symlink was also present. The numeric ttyACM index and USB hub path are host-local observations, **not portable device identifiers** and are deliberately not pinned here. The physical cable transfer and enumeration were operator observed, not independently instrumented.

## Operator-reported serial-demo evidence (2026-10-24)

The operator subsequently invoked `pulse-bridge serial-demo` on the enumerated Pulse USB CDC path. The host printed `Pico HELLO OK (protocol=1, board_id=1)`, then successfully wrote 100/100 synthetic `DemoTicks` frames, numbered 1..100. This establishes that the Pico-to-host HELLO exchange worked and the host-side serial writer accepted 100 payloads.

The LCD still showed its 9 diagnostic indicators and side stripes (reported blue/yellow). This is consistent with the bench UI retaining the button grid; color fidelity/physical orientation are not yet characterized. **The original TX-only bridge did not verify Pico-side receipt** and must not be treated as proof of 100 received frames.

This revision introduces `DemoAck` receipts on accepted CRC-valid, fresh DemoTicks, plus a visible progress bar after every ten accepted frames.

## Operator-reported ACK round-trip — 2026-10-24

On the actual Paky-01 mini-PC and physically connected Pico W, the operator executed `pulse-bridge serial-demo` with an explicit /dev/serial/by-path/ USB CDC alias and count 20. Observed host log:

```text
Pico HELLO OK (protocol=1, board_id=1)
Sending 20 explicit DEMO frames, at up to 10Hz
Expecting one CRC-checked RX DemoAck for every accepted DEMO frame.
TX DEMO seq=1 synthetic_ticks=100
RX ACK seq=1 validated_ticks=100
...
TX DEMO seq=20 synthetic_ticks=2000
RX ACK seq=20 validated_ticks=2000
PULSE ACK SUMMARY: 20/20 confirmed, 0 unexpected frames
PULSE-0.1C USB CDC ROUND TRIP VERIFIED for synthetic DEMO frames.
```

**Conclusion:** 20/20 command frames yielded matching sequence/value ACK frames validated by host CRC decoder. This is evidence of actual bidirectional **synthetic** round-trip; it is not a Somatic loop measurement, real-time performance certification or test of all reconnect scenarios.

**Observed condition:** one power-cycle of Pico hardware was needed before the successful run. The cause is **unknown** (possible CDC DTR/session-lifetime handling or a startup timing issue). Do not normalize this as expected production operation.

**Remaining acceptance:** consecutive serial-demo invocations without power-cycle, repeated DTR close/open, USB unplug/replug recovery, 100/100 or longer-frame run, screen progress/stale color and 9 input responsiveness while USB active. Panel RGB/BGR mapping is still unverified.

## PULSE-0.1C.1 — Candidate reconnect hardening

After the first successful 20/20 hardware ACK run, the operator reported needing a **power-cycle before that successful run**. Upstream Embassy USB 0.4 CDC implementation exposes `CdcAcmClass::dtr()`; its `wait_connection()` only waits for an enabled USB endpoint and **does not wait for the host's DTR**. Closing and reopening a serial port can leave the endpoint enabled, retaining the old session in the initial firmware.

Candidate repair in this branch:
- Pico checks `cdc.dtr()` at bounded intervals; when false, resets old sequence, decoder and live state.
- Host deliberately holds DTR low for 300 ms after opening before asserting it, ensuring the device sees a new session edge.
- Pico should emit a new `Hello` on every serial port reopening without unplugging the USB cable.

**This is a plausible mechanism, not a confirmed diagnosis of the original power-cycle.** Reflash the candidate and test repeated sessions physically.

### Recovery acceptance sequence (USB-only, never actuator CAN)

First, with the existing firmware still loaded, optionally repeat `serial-demo` three times without touching the USB cable to characterize the prior behavior. Then upgrade Pico and host from the latest branch and run:

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.1c-usb-cdc
git pull --ff-only
cargo test --workspace

cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
# Flash manually via Pico BOOTSEL and RPI-RP2 (see prior instructions).
```

Confirm the actual Pico USB device path through udev before using the demo command; USB hub paths can change. Perform three **separate** demo invocations without physical power-cycle (replace `<VERIFIED_PICO_PATH>` with the real alias):

```bash
cd ~/openrgd/rgd-pulse
for attempt in 1 2 3; do
  printf '\n=== CDC session %s ===\n' "$attempt"
  cargo run -p pulse-bridge -- serial-demo /dev/serial/by-path/<VERIFIED_PICO_PATH> 20 || break
  sleep 2
done
```

Each run must show `Pico HELLO OK`, `20/20 confirmed`, `0 unexpected frames`. Verify a later count-100 run and USB unplug/replug independently, then check buttons/joystick under active traffic. Capture any failure **before** power-cycling.

## Upgrade to USB CDC

On the Ubuntu mini-PC, use the existing `feat/pulse-0.1c-usb-cdc` branch (stacked on `feat/pulse-0.1a-foundation`):

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.1c-usb-cdc
git pull --ff-only

cargo test --workspace

cd firmware/pico-w
rustup target add thumbv6m-none-eabi
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Disconnect the Pico from USB, hold its **BOOTSEL** key (not LCD K1–K4), reconnect to a powered USB data hub (no 12/24V robot power), and release BOOTSEL. Confirm the FAT mount is **RPI-RP2** (avoid other devices like `DIS_G431CB`) then transfer the newly built UF2.

```bash
findmnt -rn -o TARGET /media/"$USER"/RPI-RP2
cp -v pulse-pico-w.uf2 "/media/$USER/RPI-RP2/"
sync
```

Do not run that copy unless the mount exists and really is the Pico bootloader.

After reboot the old grid/joystick should remain functional. The top color stripe now indicates:

| Color | Diagnostic meaning |
|---|---|
| Blue | No USB CDC serial session |
| Cyan | CDC opened, no valid demo frames received |
| Green | Recent valid explicitly synthetic `DemoTicks` frames |
| Red | CDC session open, synthetic frames stale (>900 ms) |

The thin orange strip while green is an additional **DEMO** marker: no actual robot loop rate is being displayed. Physical color ordering may differ pending RGB/BGR calibration.

## Find the correct serial port

Paky-01 has other USB serial/STM32 devices; **never assume /dev/ttyACM0 is the Pico**.

```bash
lsusb
ls -l /dev/serial/by-path/ 2>/dev/null || true
ls -l /dev/serial/by-id/ 2>/dev/null || true
for d in /dev/ttyACM*; do
  [ -e "$d" ] || continue
  echo "=== $d ==="
  udevadm info -q property -n "$d" | grep -E 'ID_VENDOR=|ID_MODEL=|ID_VENDOR_ID=|ID_MODEL_ID=|ID_PATH='
done
```

Look for `RGD Pulse Pico W prototype` and prototype VID:PID `c0de:cafe`, and match a `/dev/serial/by-path/...` alias if available. This prototype deliberately does **not** claim a unique USB serial number. Production requires a properly registered VID/PID and unique device identity.

Then run (replace with the actual path found above):

```bash
cd ~/openrgd/rgd-pulse
cargo run -p pulse-bridge -- serial-demo /dev/serial/by-path/YOUR_REAL_PICO_PATH 100
```

The bridge will **not send anything** until the Pico returns a version-1 `Hello` containing the prototype board ID. It sends `DemoTicks` at up to 10Hz, waits for a CRC-valid `DemoAck` with matching sequence and value for each frame, then stops; this should make the top bar green while frames arrive and stale/red when no new demo data arrives, unless closing the port first returns it to blue. K1–K4 and joystick must remain responsive throughout.


On the Ubuntu host, a successful post-upgrade run ends with:

```text
PULSE ACK SUMMARY: 100/100 confirmed, 0 unexpected frames
PULSE-0.1C USB CDC ROUND TRIP VERIFIED for synthetic DEMO frames.
```

`TX DEMO` alone is insufficient evidence. Keep the Pico isolated from motors and robot power. If acknowledgement is lost, the command exits with an error; do not mark the wave complete.

If the port fails with permission denied, check `groups` and `ls -l` on the port, then grant normal group access via your Ubuntu account's configured device access policies. Avoid running the process as root.

## Acceptance checklist

- [x] USB CDC appears as a **new identified** device, not an actuator controller (operator-reported VID:PID c0de:cafe).
- [x] Correct `Hello` handshake before transmitting any fixture (operator-reported).
- [x] Pico `DemoAck` confirms 20/20 fresh CRC-valid DEMO frames, matching sequence/value; 100/100 not yet tested.
- [ ] Demo receive progress bar advances in ten-frame steps and completes at 100.
- [ ] Green demo bar while receiving valid synthetic frames (RGB/BGR verification pending).
- [ ] Device visually detects stale stream or closed connection.
- [ ] Nine buttons and LCD activity remain functional during USB.
- [ ] Unplug/replug and repeated USB serial open/close are stable.
- [ ] No power, motor, CAN, actuation or calibration operation triggered.

## Protocol boundaries

Frame v1 is 18 bytes, little endian: `RP` magic, version, kind, sequence, sender uptime, value and CRC16-CCITT-FALSE. `Kind::DemoTicks` is explicitly synthetic; `LoopTicks` remains reserved for genuine read-only measurements after an independently reviewed Somatic adapter exists. CDC USB packet boundaries must not be interpreted as protocol frame boundaries.

The Pico does not offer a remote command interface in this wave. The device is **not** a safety supervisor. A CRC detects accidental transfer corruption; it does not authenticate a sender.
