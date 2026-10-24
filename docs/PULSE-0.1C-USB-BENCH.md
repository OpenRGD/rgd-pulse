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

**Not yet verified:** a successful `HELLO` response, actual 10 Hz `DemoTicks` transport, the corresponding status colors, live input responsiveness while CDC is active, and disconnection/reconnection recovery. Do not mark PULSE-0.1C end-to-end complete from compilation or USB enumeration alone.

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
cargo run -p pulse-bridge -- serial-demo /dev/serial/by-path/YOUR_REAL_PICO_PATH 20
```

The bridge will **not send anything** until the Pico returns a version-1 `Hello` containing the prototype board ID. It sends `DemoTicks` at 10Hz and then stops; this should make the top bar green while frames arrive and stale/red when no new demo data arrives, unless closing the port first returns it to blue. K1–K4 and joystick must remain responsive throughout.

If the port fails with permission denied, check `groups` and `ls -l` on the port, then grant normal group access via your Ubuntu account's configured device access policies. Avoid running the process as root.

## Acceptance checklist

- [x] USB CDC appears as a **new identified** device, not an actuator controller (operator-reported VID:PID c0de:cafe).
- [ ] Correct `Hello` handshake before transmitting any fixture.
- [ ] Green demo bar while receiving valid synthetic frames.
- [ ] Device visually detects stale stream or closed connection.
- [ ] Nine buttons and LCD activity remain functional during USB.
- [ ] Unplug/replug and repeated USB serial open/close are stable.
- [ ] No power, motor, CAN, actuation or calibration operation triggered.

## Protocol boundaries

Frame v1 is 18 bytes, little endian: `RP` magic, version, kind, sequence, sender uptime, value and CRC16-CCITT-FALSE. `Kind::DemoTicks` is explicitly synthetic; `LoopTicks` remains reserved for genuine read-only measurements after an independently reviewed Somatic adapter exists. CDC USB packet boundaries must not be interpreted as protocol frame boundaries.

The Pico does not offer a remote command interface in this wave. The device is **not** a safety supervisor. A CRC detects accidental transfer corruption; it does not authenticate a sender.
