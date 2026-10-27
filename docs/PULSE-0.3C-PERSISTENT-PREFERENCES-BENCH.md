# PULSE-0.3C — Persistent skin and last K-page

**Current status:** skin Italia + K3 IMU persisted and restored after a normal power cycle (physical PASS 2026-10-26). The top/left LCD artifact was eliminated physically via LCD ALIGN (2026-10-27); operator validated **X=1, Y=2**. These offsets are now compiled as the defaults in the pending firmware build, but a **fresh cold-boot check after flashing this revision remains necessary**. Earlier failures are documented below.

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

## Physical flash dump: 2026-10-26 (read-only picotool evidence)

The operator installed picotool, entered RP2040 ROM BOOTSEL, and read precisely
8192 bytes [0x101FE000,0x10200000). The offline journal decoder reported
**three valid records, 29 erased pages, zero invalid records**:

- sector0 slot0: seq1, Classic + K3 IMU
- sector0 slot1: seq2, Classic + K1 SOMATIC
- sector0 slot2: seq3, Classic + K3 IMU

This proves on-device NOR writes to the expected 8KiB region survived ROM
BOOTSEL reboot and CRC decoding. It does **not** prove the application reads
or restores the last record correctly, nor does it show any non-Classic skin
was committed. **Do not erase these records before observing a normal app
boot**. The next nondestructive test is to unplug from BOOTSEL, reboot the
Pico normally without touching any keys, and check whether it starts on
K3 IMU (the latest stored page) or K1 SOMATIC. Then apply Italia with a
confirmed center CLICK (not just preview), remain on K3, wait >=20s with
no DTR client, and take a SECOND picotool read-only snapshot to determine
whether a new Italia record was appended. Only after this evidence is
collected should the 0.3C.1 diagnostic firmware be flashed. The UI
previously showed an obsolete RAM-only label from 0.2D; the candidate
diagnostic branch corrects that label to describe the new delayed save.

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
address offsets (originally X=2, Y=3; operator-verified X=1, Y=2 now compiled as startup defaults):

- LEFT / RIGHT adjust **X** from 0 to 4.
- UP / DOWN adjust **Y** from 0 to 4.
- CLICK applies this LCD shift **in RAM until reboot**.
- Hold K3+K4 to cancel without applying.

This changes ONLY ST7735S CASET/RASET address-window offsets; the physically
verified NORM/BGR color profile, electrical GPIO mapping and USB CDC
protocols remain unchanged. Report the X,Y combination that eliminates the
top/left artifacts **while keeping four corners visible**. Those values can
now confirmed as X=1/Y=2 by operator; verify the permanent compiled default on the next cold boot.

The diagnostic overlay is not a substitute for a raw flash read: if it
reports a flash error, stop trying to save and retain the existing good
0.3B/0.3C baseline until corrected.


## Physical acceptance update — 26 October 2026

**PASS: skin Italia + K3 IMU persisted across complete USB disconnect/reconnect, no BOOTSEL and no buttons pressed.**

ROM picotool read-only snapshots captured in the 8KiB persistence partition:
- Before manual Italia commit: 3 CRC-valid records, latest seq=3 Classic / K3 IMU.
- After manually selecting and applying Italia (center CLICK) while on K3: 4 CRC-valid records, latest **seq=4 Italia / K3 IMU**, 28 erased pages and zero invalid entries.
- Operator then cold-booted normally and explicitly confirmed **Italia + K3** on the first visible dashboard.

Together these establish end-to-end embedded persistence of both requested fields on the physically connected standard Pico W: UI action -> journal write -> valid flash readback -> ordinary cold boot -> restored UI. These observations refer to the original 0.3C image that was in use at the time; the subsequent K3+K4 diagnostic + LCD ALIGN additions remain **separate, not yet physically commissioned**.

**Remaining independent gates:** verify the newly compiled X=1/Y=2 startup geometry on physical cold boot, complete 20/20 and 100/100 USB CDC ACK after flashing that firmware, and test PULSE/IMU-1 live source when publisher is available. Avoid touching the stored journal via mass erase or unbounded firmware tools.


## LCD ALIGN physical acceptance — 27 October 2026

Operator tested the `K3+K4 -> LCD ALIGN` diagnostic firmware on the actual
assembled Pico W + Seengreat ST7735S and reports that **moving alignment
by 1 pixel eliminated the stray top/left border pixels**. Overall visible
UI is correct again. This confirms the reversible on-device geometry lab
can correct the observed fault without changing accepted colors.

**Follow-up operator measurement (2026-10-27): X=1, Y=2**. The former
X=2/Y=3 values were incorrect in this physical landscape orientation.
The accepted pair is now the source default in
`crates/pulse-ui/src/diagnostics.rs`, which is consumed by
`firmware/pico-w/src/lcd.rs` from boot, including the splash.
`MADCTL=0x68 / INVOFF=0x20` and button mappings remain unchanged.

Status: **visual geometry PASS on hardware; permanent compiled default is
implemented but physical confirmation after next firmware flash is pending**.
Complete a normal unplug/replug test and CRC ACK regressions after flashing.


## 27 October 2026 — final 0.3C LCD and USB bench evidence

**Operator-reported physical PASS on the assembled Paky-01 Chronoscope after
flashing the build with default X=1 and Y=2.**

- The Seengreat ST7735S panel is visually correct immediately after flash:
  no stray top or left border pixels, no need for manual LCD ALIGN.
- Colors remain correct in the validated NORM/BGR profile.
- Previously saved Italia skin and K3 IMU view restore as expected.
- Pico's `PULSE/1` USB CDC returned **100/100 CRC-valid DEMO ACK,
  zero unexpected frames**.
- The operator's shell completed the full sequential USB regression script;
  no explicit 20/20 summary line was separately supplied in this report.
  The script had `set -e` and progressed to `USB REGRESSION COMPLETE`,
  so the 20-frame stage did not cause it to exit.

**Scope of PASS:** 0.3C preferences + UI + physical display geometry +
synthetic CDC transport. These synthetic ACKs are *not* real Somatic loop
measurements, robot safety evidence, or physical IMU telemetry.

**Still pending and owned by 0.3B's source adapter:** the actual
Console WebSocket -> `pulse-bridge imu-live` -> PULSE/IMU-1 Probe/ACK ->
Pico K3 RAW/STALE behavior while the managed IMU publisher is active.
No second reader of WIT/CH340 sensor serial may be started in Pulse.

PR remains stacked/draft until the dependent 0.3B integration gate and
review/merge order are addressed.
