# PULSE-0.2C — Physical joystick and K1–K4 INPUT LAB

**Status: PULSE-0.2C FUNCTIONAL HARDWARE ACCEPTANCE — PASS (operator-reported, 2026-10-24).** Five joystick directions, K1–K4, normal Chronoscope navigation/focus/zoom and a 20/20 CRC-confirmed **synthetic** USB CDC round-trip were reported successful. This does not certify real Somatic metrics, runtime determinism, actuator safety, or untested endurance/USB reconnect cases.

## Operator-reported input evidence — 2026-10-24

With the board held **joystick LEFT / K1..K4 RIGHT**, Input Lab displayed the following GPIO numbers when the operator physically moved the joystick:

| Physical gesture | Observed GPIO | Raw array index | Corrected software action |
|---|---|---|---|
| SU / UP | **GP18** | 7 | `Action::Up` |
| GIÙ / DOWN | **GP20** | 5 | `Action::Down` |
| SINISTRA / LEFT | **GP16** | 4 | `Action::Left` |
| DESTRA / RIGHT | **GP19** | 8 | `Action::Right` |
| CENTRO / CLICK | **GP17** | 6 | `Action::Click` |

K1, K2, K3, K4 were reported **all correct**; raw GPIO sampling remains GP8, GP4, GP3, GP2 respectively.

**Important discrepancy:** the operator reports the physical **UP** gesture as GP18 and **CENTER CLICK** as GP17, unlike the vendor-derived 0.2B hypothesis where GP18 was center and GP17 another directional pin. We preserve the first real observations for a software-only remapping, but do not treat them as independently certified pinout; recheck GP18 and GP17 **twice each**, with full release between presses, after flashing the candidate.

Source of truth distinction:
- The GPIO reported by the device is a **physical input observation**.
- The software action (`UP`, `CLICK`, etc.) is **our interpretation**, configurable and testable.
- Firmware reflash/USB ACK/normal graphical navigation post-remap remain unverified until the next operator run.
- This module has no motion commands; the physical controls remain local diagnostic UI only.

## Operator physical re-test — PASS, 2026-10-24

After loading the corrected `feat/pulse-0.2c-input-lab` firmware, the operator reported: **"mapping perfetto, ho appena testato e tutto è mappato correttamente ora nell'input lab"**.

**Accepted for this wave:** the software interpretation of all five physical joystick gestures now matches the actual joystick-left/keys-right hardware as displayed by Input Lab: UP→GP18, DOWN→GP20, LEFT→GP16, RIGHT→GP19, CLICK→GP17. This also confirms the unexpectedly assigned **GP17 CLICK** and **GP18 UP** were functional in the operator's on-device test. K1–K4 were already reported correct and their mapping was not changed.

**Not established by this specific report:** confirmation of all four dashboard pages *after exiting Input Lab*, click-to-focus/zoom behavior in the graphical pages, hold-to-exit chord, button hold-repeat behavior, and 20/20 USB HELLO/DEMO/ACK regression on this new firmware. Keep those gates separate; this firmware is still an informational, non-actuating observer.

## Operator's final graphical UI + USB regression — PASS (2026-10-24)

After the post-remap Input Lab test, the operator confirmed the remaining **hardware UI checks** (four page keys, Chronons selection/detail and Somatic focus/zoom) were all correct. This establishes a successful physical interaction smoke test for the 0.2C joystick interpretation in the displayed four-page Chronoscope.

The operator then exercised the *actual mini-PC → Pico W* USB CDC port by a verified `/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00` identity (never the unrelated `ttyUSB0` IMU). The host log reported:

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

**Physical acceptance summary: PASS** for the 0.2C joystick map, K-page navigation, focus/zoom and 20-frame USB regression. These are **operator-provided hardware observations**: we did not independently instrument signal timing or certify frame delivery at 1 kHz. The bridge still sends synthetic DEMO fixtures and the Pico remains strictly non-actuating.

**Deliberately deferred:** hold-to-repeat with accelerating scrolling, dedicated visual focus feedback, >20-frame endurance specifically on 0.2C, 3/3 DTR reopen on 0.2C, color/pixel offset qualification and genuine read-only Somatic/IMU/Chronon/system measurement sources. These do not invalidate the functional 0.2C acceptance but must not be marked independently passed.

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
| GP18     | UP         |
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

| You physically push | Observed RAW PIN (operator) | Corrected FW MAP candidate |
|---|---|---|
| UP / SU | **GP18** | UP |
| DOWN / GIÙ | **GP20** | DOWN |
| LEFT / SINISTRA | **GP16** | LEFT |
| RIGHT / DESTRA | **GP19** | RIGHT |
| CENTER CLICK | **GP17** | CLICK |

Test K1, K2, K3 and K4 separately as well:

| Physical K key | Expected GPIO | Current FW MAP |
|---|---|---|
| K1 | GP8 | K1/LOOP |
| K2 | GP4 | K2/TIME |
| K3 | GP3 | K3/IMU |
| K4 | GP2 | K4/SYS |

**Important:** the new table derives from the operator's physical Input Lab results, not the earlier vendor-oriented guess. In the *new* firmware, the `FW MAP` column must show the same gesture as the physical movement, especially `UP / GP18` and `CLICK / GP17`. Recheck them because they differ from the published board direction labels. Only `landscape_action` changes; `raw_gpio`, the GPIO driver, display orientation and USB CDC protocol are unchanged.

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

- [x] K1+K4 chord used to access INPUT LAB; operator reports full hardware test working, including return to normal Chronoscope navigation (exact 1.2s timing not externally measured).
- [x] RAW PIN and FW MAP were readable for all reported physical input directions.
- [x] Five physical joystick gestures correctly mapped by on-device Input Lab: GP18=UP, GP20=DOWN, GP16=LEFT, GP19=RIGHT, GP17=CLICK.
- [x] K1–K4 physically reported correct; each remains bound to its designated page.
- [x] Corrected mapping and regression tests checked into the no_std Rust pulse-ui crate.
- [x] Physical Chronons navigation/detail and Somatic graph focus/zoom reported working after the corrected flash.
- [x] USB CDC regression on this 0.2C firmware: HELLO OK, 20/20 CRC-checked ACK, 0 unexpected frames (synthetic fixture only).
- [ ] Longer USB endurance / repeated reboot qualification and pixel/color precision belong to later test waves, not the 0.2C functional gate.
