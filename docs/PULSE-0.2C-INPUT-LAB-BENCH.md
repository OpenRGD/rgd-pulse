# PULSE-0.2C — Physical joystick and K1–K4 INPUT LAB

**Status:** no-std Input Lab firmware candidate; **physical test pending**. No robot actuation, CAN access, IMU serial access or real Somatic telemetry.

## Why a dedicated screen?

On 128×128 graphical dashboards, some joystick actions at the current selection boundary do not visibly change the display (e.g. UP while already on the first Chronon). Navigation presently handles **press edges only**: holding a direction does **not** repeatedly scroll. Neither symptom proves a reversed physical direction.

The input diagnostic **separates** (a) which Pico GPIO is physically active from (b) which logical software command the current navigation map produces. Those must be compared to the operator's physical direction in the final landscape pose.

## Enter / exit

With the Pico display upright, **joystick LEFT, four K buttons RIGHT**, hold **K1 + K4 together for at least 1.2 seconds**. A screen titled `RGD / INPUT` with green `RAW GPIO` appears. **Release both**. Do not rotate the board during the test.

Hold the same **K1 + K4** pair for at least 1.2 seconds to exit to the normal four dashboard pages. Holding without releasing will toggle **only once**, so it cannot oscillate between modes. The K1+K4 combination itself does not count as a diagnostic event.

## What you see

```text
+-----------------------+
| RGD / INPUT  RAW GPIO |
| MOVE JOYSTICK / KEYS  |
|                       |
| RAW PIN  | FW MAP     |
| GP16     | UP         |
| HELD     | CHECK DIR  |
|                       |
| PRESSES: 2            |
| LAST INPUT PERSISTS   |
| HOLD K1+K4: EXIT      |
+-----------------------+
```

- **RAW PIN:** actual Pico GPIO sampled from active-low input, independent of any assumed orientation.
- **FW MAP:** what the firmware currently interprets that raw pin to mean. This mapping is still a **hypothesis**.
- **HELD / RELEASED:** the last recorded pin is currently held/released.
- **PRESSES:** number of *new press edges* detected since entering INPUT LAB, not autorepeat; the last press stays visible after you let go.
- Controls have no effect on Somatic, CAN, actuators, camera, IMU or other robot components during the diagnostic.

## Run exactly these five physical joystick tests

Push each direction **once**, release fully, and look at the left **RAW PIN** and right **FW MAP** fields. The first column is *what you actually did with your hand*; the GPIO is what the firmware reports and should be written down even if unexpected.

| You physically push | Current expected RAW PIN (candidate) | Current FW MAP |
|---|---|---|
| UP / SU | GP16 | UP |
| DOWN / GIÙ | GP17 | DOWN |
| LEFT / SINISTRA | GP20 | LEFT |
| RIGHT / DESTRA | GP19 | RIGHT |
| CENTER CLICK | GP18 | CLICK |

Test K1, K2, K3 and K4 separately as well:

| Physical K key | Expected GPIO | Current FW MAP |
|---|---|---|
| K1 | GP8 | K1/LOOP |
| K2 | GP4 | K2/TIME |
| K3 | GP3 | K3/IMU |
| K4 | GP2 | K4/SYS |

**Important:** the expected GPIO table is taken from current vendor pinout plus the **unverified** 90° physical orientation model, not from a fresh measurement of this assembled unit. If a physical UP shows `GP20 / LEFT`, report **exactly that**. We can then change only `landscape_action` and leave the raw pin driver untouched.

### Simple operator report

```text
UP    -> GP__
DOWN  -> GP__
LEFT  -> GP__
RIGHT -> GP__
CLICK -> GP__
K1..K4 individually -> OK / exception
```

Also note if a key produces no GPIO update, triggers multiple unexpected presses, or changes the last input only after a noticeable delay. That indicates a different class of problem than an orientation mapping swap.

## Firmware from the feature branch

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.2c-input-lab
git pull --ff-only

cargo test --workspace
cargo fmt --all -- --check

cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Reflash **only the Pico** through Pico BOOTSEL and the explicitly verified `/media/$USER/RPI-RP2` mount. Several other devices on Paky-01 expose serial ports or mass-storage volumes. Do not use the STM32 programming disk and do not attach motors/12–24V robot power.

```bash
cd ~/openrgd/rgd-pulse/firmware/pico-w
PICO_MOUNT="/media/$USER/RPI-RP2"
if mountpoint -q "$PICO_MOUNT" && [ -f pulse-pico-w.uf2 ]; then
  cp -v pulse-pico-w.uf2 "$PICO_MOUNT/"
  sync
else
  echo "NOT FLASHED: verify exact RPI-RP2 mount and compiled UF2"
fi
```

## USB and UI regression

Normal K1 SOMATIC, K2 CHRONONS, K3 IMU and K4 SYSTEM pages should remain reachable once exiting the lab. Zoom/pan demo data remain explicitly synthetic. The existing PULSE/1 HELLO/DEMO/CRC ACK/DTR protocol is **unchanged**.

Once the GPIO tests are complete, locate the actual Pulse USB CDC device by `lsusb -d c0de:cafe`, never by blindly selecting `ttyACM0` or the separate IMU `ttyUSB0`. Optionally run the existing 20-frame USB `serial-demo` to confirm no transport regression. If the ACK run fails, capture error logs before power cycling.

## Acceptance criteria

- [ ] K1+K4 chord reliably enters and exits INPUT LAB after a 1.2s hold.
- [ ] RAW PIN remains visible after each joystick release and matches reported GPIO.
- [ ] Five physical joystick directions independently reported, with no assumed axis swap.
- [ ] Four physical K keys independently reported, with no remap conflicts.
- [ ] UI and USB remain operational after exiting diagnostics.
- [ ] Only after physical evidence, update logical GPIO→UP/DOWN/LEFT/RIGHT mapping and its tests.
