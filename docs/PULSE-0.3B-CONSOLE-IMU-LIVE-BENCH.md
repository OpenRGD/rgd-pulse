# PULSE-0.3B — Console IMU read-only live fan-out to Chronoscope K3

**Status: source implementation and automated test candidate; physical end-to-end on mini-PC/Pico W PENDING.** This branch is based on `feat/pulse-0.2d-color-lab`, preserving the physically accepted Seengreat **MADCTL=0x68 / INVOFF=0x20** profile and all four Winamp-inspired dashboard skins. PR #8's bounded IMU state and offline publisher-schema JSON validator are also retained. Do **not** build the 0.3A branch to test Color Lab or claim 0.3B live: it lacks the later calibrated color profile.

## Verified read-side contracts (IMU chat handoff, 2026-10-25)

The existing Console on the **same Paky-01 Ubuntu mini-PC** already exposes:

| Purpose | Exact existing endpoint | Notes |
|---|---|---|
| Latest accepted observation (diagnostics only) | `GET http://127.0.0.1:8088/api/v1/robots/paky-01/imu` | Aged-out/missing → `null`; Console TTL **3000ms** |
| Live event broadcast | `ws://127.0.0.1:8088/api/v1/stream?robot_id=paky-01` | Consumer subscribes to `console.v1` **type=`imu`**, others ignored |
| Publisher-ingress (do **not** use in Pulse) | `POST http://127.0.0.1:8088/api/v1/imu/observations` | Existing exclusive `scripts/imu_publish.py`; token stays in publisher |

Publisher's `openrgd.imu-sample.v0.1-candidate` **is not** the Console's **`console.imu.v0.1-candidate`** read-side JSON schema. The WebSocket has general `console.v1` envelopes: `schema_version`, `type`, `robot_id`, `emitted_at`, `data`. Non-IMU types `hello`, `telemetry`, `event`, `heartbeat` are ignored. `data` contains the Console observation.

**Trust boundary:** GET and WS do NOT authenticate individual users; the gateway's security is primarily `127.0.0.1:8088` loopback binding. The client refuses arbitrary URLs, uses literal loopback, validates all identities and source flags, and must run on the **same mini-PC**. This is not end-to-end cryptographic attestation of observations. Keep Console off LAN and never pass publisher `X-OpenRGD-IMU-Token` to Pulse.

### Console observation field validation

Only samples meeting **all** checks may be labelled `RAW`:

- `schema_version = console.imu.v0.1-candidate`, `robot_id = paky-01`, `sensor_id = torso-imu-01`, `source_id = paky01-wit-serial`, `sensor_frame_id = paky01_imu_sensor_unverified`;
- `source_kind = physical`, `reference_frame = imu_sensor_axes_unverified`, `time_quality = host_receive_only_not_chronograph`;
- `observation_only = true`, `decision_eligible = false`, `heading_calibrated = false`, `calibration_revision = null`;
- session UUID 32 hexadecimal digits (**all-zero handoff redaction is not a real session**), strictly increasing `sequence` within session, nondecreasing `host_monotonic_ns` (equal timestamps are permitted for distinct sequence numbers), bounded XYZ floats and temperatures;
- source age is calculated from `CLOCK_MONOTONIC` on Ubuntu, the **same** Linux clock domain as Python `time.monotonic_ns()`. Future timestamps are rejected, and a backlog >3s is not forwarded as a current observation. An actual acquisition timestamp and certified Chronograph sync are **not available**.

Exactly one existing WIT/CH340 serial reader owns the sensor. Neither `pulse-bridge` nor Pico opens that port. The Console's `broadcast::channel(256)` fans out to independent subscribers and may skip samples for lagging consumers; source rate measured at ~10Hz, not a hard real-time promise.

## Implementation in this branch

### 1. New, separate typed USB `PULSE/IMU-1` (56 bytes)

File: `crates/pulse-protocol/src/imu_wire.rs`.

- Separate magic `"IM"`, version `1`, CRC16/CCITT-FALSE over 54 bytes.
- `Probe` (host → Pico), `ProbeAck` (Pico → host), `SensorSample` (host → Pico) are the **only** message types.
- Host explicitly requires the prior, unchanged `PULSE/1 HELLO` **and** `IMU ProbeAck` before sending any real sensor frames. Older firmware fails closed, never interprets physical IMU data as old `DemoTicks`.
- Sample payload: source `session_id` as 16 bytes, `source_sequence` u64, `host_age_at_forward_ms` u32, 3× signed angles centidegrees, 3× gyro centiradians/second, 3× acceleration centimeters/second², optional temperature centidegrees. Field flags require **physical observation + unverified sensor axes**; no motor, calibration or safety commands.
- Total 56 bytes fits a 64-byte USB CDC packet, but decoders are byte-stream-safe and CRC-checked. Old 18-byte `"RP"` PULSE/1 DEMO/ACK protocol and tests stay intact. A corrupted/missing IMU packet is dropped, not replayed.

### 2. Host subscriber and parsers

Files `crates/pulse-bridge/src/console_imu.rs`, `src/imu_live.rs`, `src/imu_import.rs`.

- Strict read-side Console JSON/parser and allowlist, session & source sequence/time checks, fresh-only bounded latest state, SI→fixed point conversion.
- Blocking WS client to the literal `ws://127.0.0.1:8088/api/v1/stream?robot_id=paky-01` endpoint, with a fixed 150ms receive timeout. Forward **at most 10Hz**, without the publisher's serial handle. No user-selected URL, no Console POST, no robot motor access.
- New CLI command: `pulse-bridge imu-live <exact-Pico-by-id> [seconds]` (1–600s, default 30). Console source is never considered cryptographically authenticated; only same-host/trusted-local with explicit source checks.
- Existing `pulse-bridge serial-demo` and `imu-preview` remain. The `imu-preview` parser handles the *publisher* schema and intentionally cannot claim live Console provenance.

### 3. Pico and K3 visualization

Files `firmware/pico-w/src/main.rs`, `src/render.rs`; shared `crates/pulse-ui/src/imu.rs`.

- Existing RP2040 USB CDC DTR handshake, 4 K buttons and joystick, Color Lab and skins are unchanged.
- Pico decodes `PULSE/1` and `PULSE/IMU-1` separately. A real observation is accepted only **after the per-session probe/ack handshake**, with valid CRC and source session+sequence; no parsed actuator command exists.
- Real K3 page renders `SENSOR XYZ / UNCAL` with **ROLL**, **PITCH**, **YAW* UNCAL**, selected gyro/acceleration X/Y/Z axis (LEFT/RIGHT), optional temperature and sample age. This is *physical sensor frame*, never verified torso/world pose.
- Badges: `RAW` when within **800ms**, `STALE` otherwise, `N/A` before any trusted sample. The Pico adds its **own elapsed time since USB reception** to `host_age_at_forward_ms`; it never compares two unrelated monotonic-clock epochs.
- On disconnected USB CDC, sample state clears. If publisher/WS stops while CDC remains open, `RAW → STALE` transitions automatically, even with no new frames.
- **Charts remain explicitly `DEMO`** until we have source-backed timestamped IMU history. Other dashboards (Somatic, Chronons, System) continue to display unavailable/DEMO as before.

## Physical integration test — Ubuntu + Paky-01

**Never run two programs that open the CH340/WIT sensor.** The existing IMU publisher must own it exclusively.

### Step A: verify Console availability (read-only)

With the existing `imu_publish.py` already running:

```bash
curl -fsS --max-time 3 \
  -H 'Cache-Control: no-cache' \
  http://127.0.0.1:8088/api/v1/robots/paky-01/imu \
  | python3 -m json.tool
```

A JSON object containing `schema_version: console.imu.v0.1-candidate` and `source_kind: physical` proves the current service has one accepted fresh record. `null` means no fresh record (publisher stopped or stale), **not sensor failure**. Do not publish or log POST credentials.

### Step B: host and firmware compile from combined color-calibrated branch

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.3b-console-imu-live
git pull --ff-only

cargo test --workspace
cargo fmt --all -- --check

cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Flash **only the Pico W** with USB BOOTSEL; verify its `RPI-RP2` drive. Never use the STM32 drive and never connect actuators/motor supply for this test:

```bash
PICO_MOUNT="$(findmnt -rn -S LABEL=RPI-RP2 -o TARGET | head -n 1)"
if [ -n "$PICO_MOUNT" ] && mountpoint -q "$PICO_MOUNT" &&
   [ -f pulse-pico-w.uf2 ]; then
    cp -v pulse-pico-w.uf2 "$PICO_MOUNT/"
    sync
    echo "PULSE 0.3B candidate copied to Pico W"
else
    echo "ABORT: verify Pico BOOTSEL RPI-RP2 and UF2"
fi
```

### Step C: physical IMU motion test (host WS → Pico)

**In a new Ubuntu terminal** after flashing, while Console + existing publisher are active, open the verified Pico USB CDC port and subscribe live:

```bash
cd ~/openrgd/rgd-pulse
PICO_PORT="/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00"
test -e "$PICO_PORT" || { echo "Pico CDC by-id missing"; exit 1; }
cargo run -p pulse-bridge -- imu-live "$PICO_PORT" 30
```

Expected host lines:
```text
PULSE/1 HELLO OK
PULSE/IMU-1 PROBE/ACK OK
Console v0.5 WS connected (same-host, read-only)
IMU #1 seq=... SENSOR XYZ roll=... pitch=... yaw*=... age=...ms YAW_UNCAL
...
PULSE IMU SUMMARY: forwarded=N rejected=0 non_imu=...
```

Open **K3 IMU** on the small Chronoscope. Slowly rotate/tilt the actual sensor/robot while it is supported and unpowered by motors. ROLL/PITCH/YAW* must change; selected GYR/ACC X/Y/Z via joystick LEFT/RIGHT must update as the publisher sends observations. `YAW*` remains labelled uncalibrated. Only static orientation changes with properly initialized publisher are meaningful; **do not infer the IMU→torso transform** or motion safety.

Stop the existing IMU publisher or let the 30s bridge session end: K3 should no longer display an apparently fresh measurement; status becomes `STALE` on feed pause (or `N/A` after CDC closes), not invented zeros.

### Step D: legacy PULSE/1 USB synthetic ACK regression

**Do not run** `serial-demo` simultaneously with `imu-live`: both require exclusive Pico USB CDC ownership. After the IMU subscriber terminates:

```bash
cd ~/openrgd/rgd-pulse
PICO_PORT="/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_prototype-if00"
cargo run -p pulse-bridge -- serial-demo "$PICO_PORT" 20
cargo run -p pulse-bridge -- serial-demo "$PICO_PORT" 100
```

Expect `HELLO OK`, `PULSE ACK SUMMARY 20/20` and `100/100`, **0 unexpected frames**. A failed ACK test means the candidate is not physically accepted; collect logs without modifying hardware. Check RGB flag is still correct under ITALIA skin and Input Lab/K1–K4 still navigate.

## Acceptance and limits

- [ ] Source validation with on-device Console JSON: live GET returned a non-null `console.imu.v0.1-candidate` observation.
- [ ] Rust host test suite and Pico `thumbv6m-none-eabi` release compile / BOOT2/vector table **CI** green.
- [ ] Actual Pico `PULSE/1 HELLO`, `IMU ProbeAck` and `N` bounded real samples over USB.
- [ ] K3 real roll/pitch/yaw sensor-frame changes correctly with physical motion; status `RAW` while fresh, `STALE/N/A` after stoppage.
- [ ] Existing synthetic 20/20 and 100/100 CRC-checked ACK, joystick direction and display color regression.
- [ ] No duplicate reader of physical IMU CH340 serial; no CAN/motor/actuator changes.
- [ ] Never claim certified absolute heading, calibrated torso pose, Chronograph synchronization, 1kHz Somatic runtime or authenticated remote source.

The only **missing information after handoff** is fresh **runtime confirmation of the installed Console** and the end-to-end physical test results. The source schema and read-only endpoints are documented and sufficient to implement the code; there is no need to ask the IMU chat for another endpoint.

**Context note:** OpenRGD Console runtime source is local at `~/openrgd/repos/openrgd-console`; its matching GitHub repository commit was not accessible in the handoff. This implementation uses the handoff's actual v0.5 WS/GET contract and insists on explicit schema validation rather than inventing a Console SHA.

## Physical end-to-end operator result — 27 October 2026

**OPERATOR-REPORTED PASS:** On the actual Paky-01 Ubuntu mini-PC and RP2040
Chronoscope, the operator ran a bounded IMU publisher to the existing Console,
then launched `pulse-bridge imu-live` using the independently identified Pico
USB CDC port. The operator reports that **the live IMU values work correctly
on K3** following the preceding test instructions.

This constitutes physical user acceptance of the *functional forward path*:

```text
WIT/JY901 CH340 -> exclusive Python IMU publisher -> Console localhost:8088
    -> read-only Console WS -> Pulse Bridge Rust
    -> PULSE/IMU-1 USB CDC -> Pico W K3 real sensor-frame readout
```

The firmware physically tested was the successor **PULSE-0.3C** build
(recoverable preferences, fixed LCD X=1/Y=2) based on this 0.3B source
branch, not necessarily the older standalone 0.3B ELF. The accepted
color/persistence/display features and synthetic USB 100/100 ACK had
already passed before the live IMU test.

**Evidence granularity:** This is a direct operator functional report, **not**
an attached terminal transcript of the actual `PULSE/IMU-1 PROBE/ACK`,
`forwarded=N`, `rejected=0` counters or sample measurements. Do not
assert exact live Hz, latency, loss rate, values, or IMU calibration.

### Separate outstanding hardware verification

- [x] Operator observes functional real IMU feed on Chronoscope K3 using
      the existing Console WS and USB read-only adapter.
- [x] Prior PULSE/1 100/100 synthetic CRC ACK, zero unexpected frames
      on the physically flashed 0.3C Chronoscope.
- [ ] Capture or summarize host `PULSE/IMU-1 PROBE/ACK OK` and forwarding
      counters for reproducible traceability (no secrets or raw tokens).
- [ ] Specifically test `RAW -> STALE` by stopping the **sole existing
      publisher** while the USB bridge remains connected; then expect
      Console GET to age to `null` after ~3 seconds. When the CDC client
      closes K3 may instead go to `N/A`; do not call that stale test.
- [ ] Document bounds/gyro/accel and calibration disclaimers as before.

**Authority is unchanged:** the display is sensor-frame, uncalibrated
observation only, not torso pose, balance/safety state or a motor command.
The publisher remains the sole IMU/CH340 serial reader. No additional
service installation, hidden auto-start, or robot actuation occurred in
this Pulse acceptance report.


## 27 October 2026 — physical handshake and stale-transition evidence

The operator supplied the following **actual on-device end-to-end host output**,
rather than a hypothetical protocol example:

```text
PULSE/1 HELLO OK
PULSE/IMU-1 PROBE/ACK OK (Pico supports typed sensor frames)
Console v0.5 WS connected (same-host, read-only)
IMU #1 seq=181 SENSOR XYZ roll=-4.93 pitch=-8.45 yaw*=-160.60 age=96ms YAW_UNCAL
IMU #2 seq=182 SENSOR XYZ roll=-4.93 pitch=-8.45 yaw*=-160.60 age=4ms YAW_UNCAL
IMU #3 seq=184 SENSOR XYZ roll=-4.93 pitch=-8.45 yaw*=-160.60 age=4ms YAW_UNCAL
IMU #10 seq=199 SENSOR XYZ roll=-4.93 pitch=-8.45 yaw*=-160.60 age=1ms YAW_UNCAL
IMU #20 seq=219 SENSOR XYZ roll=-4.93 pitch=-8.44 yaw*=-160.60 age=4ms YAW_UNCAL
```

**Physical PASS:** Both negotiated USB protocol handshakes, local Console
WebSocket read-only subscription, forwarded numbered samples, and operator-
observed K3 real IMU values.

**Physical PASS — publisher cessation:** The operator explicitly stopped
the IMU publisher while watching K3. K3 changed to **STALE** and rendered
the last sensor readings greyed out, rather than continuing to claim that
measurements were fresh. After a further interval the visual returned to
the preceding baseline/no-feed screen. This proves the stale indication
worked; it does **not** measure the precise number of milliseconds.

**Source-level explanation of the later fallback:** Embedded main.rs
clears `ImuView` when Pico USB CDC DTR drops, then K3's renderer changes to
`SENSOR XYZ / NO FEED`, with unavailable numerical fields and an explicitly
DEMO chart. Stopping the publisher alone normally leaves K3 in the stale
state until the bridge exits/closes CDC; `UiState.page` itself is never
automatically changed by the source-disconnect handler. Do not assert
that an automatic navigation between K1/K2/K3/K4 occurred.

**Limits:** No attached end-of-run `forwarded/rejected/non_imu` summary,
no independently measured real rate or total latency, no captured
Console GET aging to `null`, and no verified sensor-frame to robot-body
extrinsic transformation. The logged `age` fields are source-age
estimates at forwarding, not calibrated latency benchmarks.
`YAW_UNCAL` remains explicitly uncalibrated. This acceptance provides
**observation-only** data, never actuator or balance authority.

**Review gate status:** physical K3 feed and publisher-stop STALE PASS;
console TTL/null and optional per-session summary remain future regressions.
The physically tested RP2040 image was built from stacked PULSE-0.3C,
which incorporates this PULSE-0.3B adapter.
