# PULSE-0.3A — IMU real-data model and Console handoff

**Status:** Phase A SOURCE MODEL + OFFLINE JSON SHAPE VALIDATION + CONTRACT HANDOFF ONLY. This branch adds a tested `no_std` IMU model and a host-side `pulse-bridge imu-preview` JSON validator. It does **not** connect the live Console feed to USB, alter flashed Pico firmware, modify the WIT reader, or display actual IMU values yet.

## Actual Paky-01 producer reviewed

Source: `italiarobotica/paky-01`, branch `feat/imu-console-bridge`, `scripts/imu_publish.py`; underlying WIT/JY901-compatible read-only decoder `scripts/imu_wit.py`.

- One exclusive hardware serial reader at the known CH340 by-path device, default **9600 baud**. Do **not** open a second `pyserial` handle on the physical IMU. This source device is unrelated to Pico's CDC port.
- `scripts/imu_publish.py` POSTs JSON to **`http://127.0.0.1:8088/api/v1/imu/observations`** using an auth token in `X-OpenRGD-IMU-Token`; the endpoint is an **ingress**, not a read/subscription API. Never send this token over Pico CDC or include it in log/files.
- Current publisher schema **`openrgd.imu-sample.v0.1-candidate`**, default robot `paky-01`, sensor `torso-imu-01`, source `paky01-wit-serial`, and `sensor_frame_id = paky01_imu_sensor_unverified`.
- Each observation includes a 32-character hex `session_id`, `sequence`, `host_receive_utc`, `host_monotonic_ns`, `acceleration_m_s2: [x,y,z]`, `angular_velocity_rad_s: [x,y,z]`, `orientation_euler_deg: [roll,pitch,yaw]`, optional `temperature_c` and `calibration_revision: null`.
- Source frames are WIT 11-byte CRC-like additive-checksum packets and correspond to **sensor-frame observations only**. The sensor-to-torso transform, gravity/world alignment and absolute yaw are **not calibrated**. Do not label Euler angles `robot orientation` or `balanced`.
- Producer batches acceleration + gyro + attitude within a bounded host receipt window. `host_monotonic_ns` is a host *receipt timestamp*, not sensor-origin absolute measurement time or a Chronograph global timestamp.

## Data route (must reuse existing source)

```text
Physical IMU WIT/JY901 compatible (9600 bps /dev/serial/by-path/...)
      |
One exclusive scripts/imu_publish.py reader
      |
Token-protected localhost Console POST ingress :8088
      |
Console accepted IMU observation + existing read-side fan-out/WS
      |
TO CONFIRM WITH IMU/CONSOLE CHAT: actual subscriber URL, auth and payload
      |
PULSE HOST ADAPTER (read-only) -> quantized IMU v1 candidate envelope
      |
Future separately versioned/negotiated USB telemetry frame
      |
Pico K3 IMU, live sensor axes + stale/provenance badges
```

This stage deliberately **does not repurpose `PULSE/1 Kind::DemoTicks`** as real IMU values. PULSE/1 v1 `HELLO`, `DEMO`, `DemoAck`, 18-byte framing, DTR and CRC must remain regression-green throughout the adapter rollout. A typed, versioned IMU wire message and explicit host/Pico capability negotiation will be separately reviewed.

## Implemented pure model

`crates/pulse-ui/src/imu.rs` is hardware-independent, no_std and bounded: `ImuSample`, `ImuView`, `ImuStatus`, `Origin`, `FrameQuality`.

| Model property | Representation / invariant |
|---|---|
| Source/provenance | Fixed `robot_id`, `sensor_id`, `source_id`, `sensor_frame_id` constants matching the real publisher |
| Source sequence/session | `source_sequence: u64`; `source_session: [u8;16]` binary form of publisher's 32-hex-digit session UUID |
| Angles | `roll_cdeg`, `pitch_cdeg`, `yaw_cdeg`: signed centidegrees, range -180.00..+180.00 |
| Gyro | `gyro_mrad_s: [i32; 3]`, quantized from publisher rad/s |
| Acceleration | `accel_mm_s2: [i32; 3]`, quantized from publisher m/s² |
| Temperature | `temp_cdeg: Option<i16>`; no invented values when not available |
| Origin | `ConsoleReadOnlyObservation`, `ImportedJsonUnverified`, or `SyntheticFixture` distinct at type level. Importing matching JSON does **not** authenticate it. A live host subscriber must check source/robot/session/auth before assigning the real origin |
| Frame | `SensorAxesUnverified` (no sensor-to-torso calibration or absolute yaw) |
| Freshness | `host_age_at_forward_ms: Option<u32>` supplied from host-monotonic age at forward time; on Pico it is summed with elapsed **Pico receive-relative time**. No comparison of two unrelated clocks |
| Valid status | NoSource / Synthetic / UntrustedImport / AgeUnknown / FreshSensorAxes / Stale |
| Stale gate | 800 ms **UI threshold**, not a control deadline or IMU calibration assertion |
| Replay protection | Duplicate/descending sequence rejected within same source session; new session ID permits sequence reset |
| Missing/invalid | Reject out-of-range angles and sequence 0; an absent host age can never be called fresh |

No heap, no sensor serial opens, no USB transmission, no actuation in the shared MCU model. The host-only JSON preview allocates bounded memory to parse a local file and is tested independently.

## Added host-only sample validator (no device access)

`crates/pulse-bridge/src/imu_import.rs` uses the exact known candidate producer schema. It checks all five source/identity constants, full 16-byte session UUID, nonzero sequence/host receipt timestamp, `calibration_revision: null`, three-element arrays, finite and bounded values, quantization and explicit **offline unverified provenance**.

For one **redacted observation JSON file** exported by the IMU/Console chat:

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.3a-imu-observation-model
git pull --ff-only
cargo test --workspace
cargo run -p pulse-bridge -- imu-preview /path/to/redacted-imu-sample.json
```

Output prints sensor-axis Euler angles, quantized gyro/accel and `age=UNKNOWN / UNVERIFIED / NO_USB / NO_MOTION`. This is **not** a live subscription, does not query Console's POST endpoint, does not open either USB serial port, and does not transmit or claim robot/world attitude. A local file may contain legitimate sensor-origin bytes, but the parser cannot authenticate its provenance, so the Rust `Origin::ImportedJsonUnverified` state must never become `FreshSensorAxes` on Pico.

## Must clarify with the already-running IMU/Console implementation

There is **not enough verified read-side API information yet** to connect a second consumer. The publisher's `POST` endpoint must **not** be mistaken for a readable stream. The local `OpenRGD/openrgd-console` repository cannot currently be accessed through the available GitHub connection; the IMU/Console chat or a non-secret snippet of the installed code is needed.

Ask the IMU/Console team for:

1. **Exact read-side contract**: current local `GET` latest observation and/or WebSocket subscribe endpoint (including port, URL, event/topic type, auth, sample JSON, whether multiple consumers are allowed). **Do not invent new endpoints.**
2. Which source in Console receives/publishes the IMU observation? File paths, function/type names, branch/commit after its recent IMU merge. Provide the *read-side* contract separate from ingress.
3. Freshness/time semantics: `host_monotonic_ns` is host-receipt; does the outgoing Console payload include monotonic value + session id + seq? Any device timestamp? What is the exact guaranteed publish frequency? How is `STALE` indicated?
4. `robot_id`, `sensor_id`, `source_id`, `sensor_frame_id`, `calibration_revision`, yaw validity and missing-value conventions; verify they're passed through losslessly.
5. Can an Ubuntu local `pulse-bridge` observer subscribe **read-only** and be decimated to 5–10 Hz with bounded buffering, without impacting Console latency and without opening the IMU serial again? What is the appropriate token/permission boundary and reconnection behavior?
6. Provide one **redacted real JSON sample** and the command to view its arrival using the existing local Console service. Do not send passwords, X-OpenRGD-IMU-Token, cookies, account secrets or other credentials.

## Next engineering step after contract

Implement a single host-side Console subscription, strict allowlist + JSON shape validation, quantization and freshness, and a separately negotiated typed CDC IMU message with on-Pico ring buffer and K3 graphics. The Pico may display `RAW SENSOR`, `YAW* UNCAL`, source age and live roll/pitch, but **not** inferred torso/world pose. On upstream disconnection show `STALE` and retain the last sample with age (or `--` if no source). Keep synthetic chart fixtures clearly segregated from real numeric values.

**Safety test matrix:** PULSE/1 3×20 + 100 synthetic ACK regression remains green; display navigation and skins work during stream; IMU update ≤10Hz; no second serial reader; stale transitions deterministic and no CAN/motor commands. Only then mark hardware source integration PASSED.

**Note:** prior local Console/hardware IMU tests are not proof that this new Pico adapter or typed USB wire schema has been physically commissioned. The architecture intentionally separates them.
