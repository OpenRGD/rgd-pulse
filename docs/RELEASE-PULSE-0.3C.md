# PULSE-0.3C — Chronoscope physical acceptance and source freeze

**Consolidated on:** 2026-11-17  
**Hardware acceptance by operator:** 2026-10-24 through 2026-10-27  
**Status:** physically accepted observation-only prototype; feature scope **FROZEN**  
**Repository:** `OpenRGD/rgd-pulse`

## Immutable source reference for the tested hardware

- **Frozen source branch:** `release/pulse-0.3c-physical-20261027`
- **Tested firmware source commit:** `cd2d640d0823e22a1b89e049d2e46a83dd5e2ce8`
- **Initial consolidated `main` merge commit:** `b61e85c4b5c3688984cb66807156eaddcb123206`
- The initial consolidated `main` was compared against the frozen tested source commit: **all differences were Markdown documentation only**. Rust source, memory map, protocol, hardware driver and firmware build inputs were identical.
- **No firmware image was pushed to the physical Pico during repository consolidation.** The source freeze branch identifies the source tree used for the physically accepted local UF2 build; it is not a GitHub binary asset or signed release artifact.
- Feature development must use a new post-0.3C branch. Change this baseline only for explicit bug/security fixes with a new physical acceptance gate. Do not force-update/delete the frozen source ref.

## Physically accepted scope

| Area | Hardware / software observation |
|---|---|
| RP2040 Pico W | Firmware boots on installed Pico W with Seengreat ST7735S 128×128 |
| Visual layout | Joystick at left, K1–K4 at right; all four pages accessible |
| Colors | Operator-validated `MADCTL=0x68`, `INVOFF=0x20` |
| LCD geometry | `DEFAULT_X=1`, `DEFAULT_Y=2` from first boot; no upper/left artefacts |
| Local skin | Four ROM presets; Italia applied and restored from cold power cycle |
| K page | Last selected K3 IMU restored after complete USB disconnect/reconnect |
| Flash journal | Last 8KiB in standard 2MiB chip; two sectors, 256-byte CRC16 records; read-only picotool dump confirmed four valid records, seq4 Italia/K3, zero corrupt |
| USB PULSE/1 | `serial-demo` CRC ACK **100/100**, **0 unexpected**; sequential 20-frame run did not abort |
| USB PULSE/IMU-1 | `PULSE/1 HELLO OK`, `PULSE/IMU-1 PROBE/ACK OK`, Console WS connected |
| Real physical K3 | WIT/JY901-compatible IMU -> one exclusive publisher -> Console loopback -> read-only Rust Pulse Bridge -> Pico K3; sensor-frame values observed |
| Loss of source | After publisher stop, K3 displayed `STALE`, kept and muted last sample; later returned to no-feed view when bridge session ended |

Representative real IMU transport log, *not* a calibration reference:

```text
PULSE/1 HELLO OK
PULSE/IMU-1 PROBE/ACK OK (Pico supports typed sensor frames)
Console v0.5 WS connected (same-host, read-only)
IMU #1 seq=181 SENSOR XYZ roll=-4.93 pitch=-8.45 yaw*=-160.60 age=96ms YAW_UNCAL
IMU #20 seq=219 SENSOR XYZ roll=-4.93 pitch=-8.44 yaw*=-160.60 age=4ms YAW_UNCAL
```

The `age` field is not a measured, certified end-to-end latency. The actual `RAW -> STALE` delay was visually observed but not timed independently.

## Explicitly out of scope / not yet verified

- **No motion authority.** This prototype is not a motor controller, E-stop, balance safety controller, certified emergency indicator, or a production security boundary.
- **No actual Somatic loop timing, motor safety, CAN/joint live status, power readings, verified Chronons or physical K4 host metrics.** K1, K2, K4 and graph histories retain explicit DEMO / unknown where a real source is absent.
- **No calibrated torso/world orientation or heading.** Raw sensor-frame roll/pitch/`YAW* UNCAL` must not be interpreted as calibrated body attitude or magnetic/absolute heading.
- **No authenticated LAN/remote Console WebSocket.** The validated connection is a trusted-local, loopback-only prototype on `127.0.0.1:8088`; do not expose it to LAN without new authorization review.
- No separately captured `PULSE IMU SUMMARY: forwarded/rejected/non_imu` terminal counters, exact sensor frequency, serial packet loss, precise display latency, or Console GET becoming `null` on its 3-second freshness boundary.
- The peripheral manager service auto-start, generic host permissions, credential distribution and central start/stop UX are separate projects, not part of the Pulse freeze.
- **No Console canonical contract promotion, boot-on-power hardware integration, remote actuation, or background bridge service.**

## Source safety and reproducible build

Use the frozen branch to reproduce the **tested source**, not `main` after future feature work:

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch --detach cd2d640d0823e22a1b89e049d2e46a83dd5e2ce8
cargo test --workspace
cargo fmt --all -- --check

cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w-0.3c.uf2
python3 ../../scripts/check-pulse-uf2-prefs.py pulse-pico-w-0.3c.uf2
```

**This builds only; it does not flash.** Use only on the verified Pico W and preserve the settings journal at `0x101FE000..0x10200000`. A UF2 address check does not verify the physical flash chip identity. Do not mass-erase preferences, select a generic `ttyACM0`, open the IMU CH340 twice, change the robot's CAN/safety configuration, or connect drive power during Pico-only commissioning. The Pulse Bridge itself does not open the WIT serial device.

## Consolidation history

Merged to `main` preserving merge ancestry, in dependency order:
`#1 -> #2 -> #4 -> #5 -> #6 -> #7 -> #9 -> #10 -> #11`.

PR `#8` was intentionally **closed as superseded** by the merged `#10/#11` implementations, rather than merged again. The original 0.3A `imu.rs` and `imu_import.rs` blobs were verified **byte-identical** to the files in `main`; its handoff is preserved at `docs/archive/PULSE-0.3A-IMU-SOURCE-HANDOFF.md`.

## Next work (new branches only)

1. Keep 0.3C unchanged as a reproducible physical benchmark.
2. Define a reviewed, **read-only K4 SYSTEM adapter** for real host, link and peripheral health. Show `N/A` when a measurement is missing; do not fabricate CAN or drive voltage.
3. After separate source contracts, add K1 SOMATIC and K2 CHRONONS observation adapters without touching the 0.3C motor-free firmware baseline.
4. Review dependency versioning, authorization, reproducible source hashes and hardware tests independently of this release.

Bench details:
- [PULSE-0.3B IMU live bench](PULSE-0.3B-CONSOLE-IMU-LIVE-BENCH.md)
- [PULSE-0.3C flash, LCD and persistence bench](PULSE-0.3C-PERSISTENT-PREFERENCES-BENCH.md)
- [Chronoscope roadmap](ROADMAP.md)
