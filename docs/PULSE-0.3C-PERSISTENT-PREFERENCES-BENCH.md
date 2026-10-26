# PULSE-0.3C — Persistent skin and last K-page

**Status: operator physical FAIL on 2026-10-26 for original 0.3C image.** Neither skin nor K page restored after power cycle. User also reported anomalous top/left LCD border pixels. Updated source adds read-only flash diagnostics and a reversible LCD-address calibration lab; these fixes still require a new hardware bench and MUST NOT be called physically accepted.

The Chronoscope must recover only the **applied skin** (Classic / Phosphor / Chrome 99 / Italia) and the **last K1..K4 page** after a complete power-off. Focus, zoom, pan, menu previews, demo ticks, IMU samples and calibration are never persisted. Color calibration remains operator-approved NORM/BGR mode 4 (MADCTL 0x68, INVOFF 0x20) on every boot.

## Pico W hardware and flash map

Standard Pico W RP2040 with **2 MiB** external QSPI flash; do not apply this partition to RP2350 or boards with unverified flash geometry.

- BOOT2: 0x10000000..0x100000ff
- Firmware: 0x10000100..0x101fdfff (shrunk by linker memory.x)
- Preferences sector A: 0x101fe000..0x101fefff (4096 B)
- Preferences sector B: 0x101ff000..0x101fffff (4096 B)

Two-sector append-only journal, 16 x 256-byte record pages per sector. Versioned record with CRC16, monotonic wrapping sequence. Rotate by erasing only the *other* sector while retaining the old valid record. Failed/interrupted data is ignored at boot; fallback is Classic + K1. The recorded record is verified by a readback. No motor, CAN, sensor, Somatic or safety data in preferences.

Write only from main CORE0 after 3 seconds of unchanged choices and at least 15 seconds after a previous save. **Do not write while USB CDC DTR is asserted**: no ACK or live IMU timeouts may be introduced by flash writes. This means a preference change made with serial-demo/imu-live running is saved *after the serial client disconnects* and the debounce interval has elapsed. Do not unplug immediately after a change. Keep power for >5s (or >20s following a previous write) before reboot.

The 0.3.1 Embassy-RP flash driver handles RAM-based write/erase under critical section. Future release engineering must ensure mass-erase/firmware updates do not destroy preferences; UF2 transfers normally write supplied flash blocks only. Recovery/mass-erase tools can still erase the preference partition.

## Build and safety gate (NO AUTOMATIC FLASH)

```bash
cd ~/openrgd/worktrees/rgd-pulse-prefs03c
git fetch origin
git switch feat/pulse-0.3c-pico-persistent-preferences
git pull --ff-only
cargo test --workspace
cargo fmt --all -- --check
cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w-0.3c.uf2
cd ../..
python3 scripts/check-pulse-uf2-prefs.py firmware/pico-w/pulse-pico-w-0.3c.uf2
```

Check CI release image sections and UF2 address range before flashing; if the checker fails, **STOP**. Flash via explicitly identified Pico W RPI-RP2 BOOTSEL only, using USB power and without motor power.

## Physical acceptance checklist

1. Confirm splash, color calibration, buttons, skins, Color Lab and Input Lab unchanged.
2. Re-test synthetic PULSE/1 HELLO / 20-of-20 / 100-of-100 DemoAck with no unexpected packets. Never run two CDC host clients simultaneously.
3. Close any DTR-holding serial client. Choose **Italia** via K2+K3/CLICK and **K3 IMU**. Keep USB power for >=5s (>=20s if a recent save occurred). Unplug and replug normally, without BOOTSEL. It should restore Italia + K3 while IMU publisher remains off, displaying N/A rather than fabricated data.
4. Repeat with **Phosphor + K4**, power-cycle, verify. Preview then cancel a different skin; it must not persist.
5. While active serial-demo / imu-live owns CDC, new choices must wait until DTR deasserts before writing. Regression-test that ACKs and live IMU framing remain correct.
6. No physical destructive power-cut test is requested. Unit-test simulation covers torn records and sector rollover; a physically clean power cycle establishes basic persistence.

**Not yet accepted:** physical persistence across real unplug/replug, IMU live retest, flash failure handling on a physical Pico. All are independent gates. This is a diagnostic HMI, not a robot motion controller or safety device.

## 0.3C.1 investigation — after physical FAIL

**Do not blindly flash again.** First inspect the on-device 8KiB region of the
previously flashed 0.3C image, preferably through the RP2040 ROM USB BOOTSEL,
without any write. This is optional if picotool is unavailable.

1. Enter BOOTSEL on the **Pico W only**, with motors disconnected.
2. Confirm a single `2e8a:0003 Raspberry Pi RP2 Boot`.
3. With Raspberry Pi `picotool` available:

```bash
picotool info
picotool save -r 0x101FE000 0x10200000 /tmp/pulse-prefs-before.bin -t bin
python3 scripts/inspect-pulse-preferences-dump.py /tmp/pulse-prefs-before.bin
```

The decoder is strictly read-only and distinguishes all-FF (no write), valid
records (write happened, restoration failed or wrong image/flash map), and
invalid records (partial/bad programming). Keep the dump private unless
specifically needed for debugging; only the summary is normally necessary.

### Diagnostic firmware candidate (source only until CI/UF2 PASS)

The updated firmware contains a read-only diagnostic overlay accessed with
**K3+K4 long hold (1.2s)**. While holding this chord no robot commands are
ever emitted. The first screen reports:

- **STATE**: EMPTY, RESTORED, PENDING, WAIT IDLE, WAIT USB DTR, WAIT RATE,
  SAVED, BAD FLASH ID, JEDEC ERROR, READ ERROR or WRITE ERROR.
- **JEDEC**: 24-bit flash chip identification returned by the Embassy RP flash HAL.
- **SEQ**: last confirmed journal record sequence (0 if empty).
- **STORED**: last fully verified skin and K page.
- **PEND**: pending unsaved choices (if any).
- **USB DTR**: active clients prevent writes by design.
- **ERR**: 1 bounds, 2 alignment, 3 wrong CPU core, 4 hardware error,
  5 verify readback, 6 JEDEC read, 7 missing journal.

Firmware **fails closed** before journal access if JEDEC capacity byte is
not `0x15` (standard 2MiB RP2040 Pico W); this is diagnostic evidence
and may require investigating a different flash chip. Do NOT defeat the
capacity check without physical verification.

### LCD geometry diagnostic

Within the K3+K4 overlay, press joystick **CLICK** to open `LCD ALIGN`.
This displays a true 1-pixel perimeter, distinct corner marks, and live
address offsets (starting from X=2, Y=3, both provisional):

- LEFT / RIGHT adjust **X** from 0 to 4.
- UP / DOWN adjust **Y** from 0 to 4.
- CLICK applies this LCD shift **in RAM until reboot**.
- Hold K3+K4 to cancel without applying.

This changes ONLY ST7735S CASET/RASET address-window offsets; the physically
verified NORM/BGR color profile, electrical GPIO mapping and USB CDC
protocols remain unchanged. Report the X,Y combination that eliminates the
top/left artifacts **while keeping four corners visible**. Those values can
then replace the provisional defaults after acceptance, rather than guessing.

The diagnostic overlay is not a substitute for a raw flash read: if it
reports a flash error, stop trying to save and retain the existing good
0.3B/0.3C baseline until corrected.
