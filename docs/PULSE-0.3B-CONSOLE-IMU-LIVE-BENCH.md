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
