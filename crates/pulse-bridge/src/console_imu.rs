//! Console v0.5 IMU read-side schema (NOT the publisher POST schema).
//!
//! The only live transport is the existing localhost Console fan-out:
//! ws://127.0.0.1:8088/api/v1/stream?robot_id=paky-01
//! - console.v1 / type=imu / data.schema_version=console.imu.v0.1-candidate.
//! The GET /api/v1/robots/paky-01/imu is a read-only diagnostic alternative.
//!
//! Source allowlisting and loopback-only connection are NOT cryptographic
//! authentication; a malicious local process could spoof messages. Never
//! expose this unauthenticated Console gateway on the LAN. The POST token is
//! not used by this subscriber and must never be logged or sent over USB.

use pulse_protocol::imu_wire;
use pulse_ui::imu::{
    FrameQuality, ImuSample, ImuView, Origin, ROBOT_ID, SENSOR_FRAME_ID, SENSOR_ID, SOURCE_ID,
};
use serde::Deserialize;

pub const REST_LATEST_URL: &str = "http://127.0.0.1:8088/api/v1/robots/paky-01/imu";
pub const WS_STREAM_URL: &str = "ws://127.0.0.1:8088/api/v1/stream?robot_id=paky-01";
pub const CONSOLE_READ_SCHEMA: &str = "console.imu.v0.1-candidate";
pub const CONSOLE_STREAM_SCHEMA: &str = "console.v1";
const REFERENCE_FRAME: &str = "imu_sensor_axes_unverified";
const TIME_QUALITY: &str = "host_receive_only_not_chronograph";
const MAX_JSON_BYTES: usize = 16 * 1024;
const MAX_FORWARD_AGE_MS: u32 = 3_000;

#[derive(Debug, Deserialize)]
struct Envelope {
    schema_version: String,
    #[serde(rename = "type")]
    kind: String,
    robot_id: String,
    emitted_at: String,
    data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ConsoleSample {
    schema_version: String,
    robot_id: String,
    sensor_id: String,
    source_id: String,
    sensor_frame_id: String,
    session_id: String,
    sequence: u64,
    source_kind: String,
    host_receive_utc: String,
    host_monotonic_ns: u64,
    console_received_at: String,
    orientation_euler_deg: [f64; 3],
    angular_velocity_rad_s: [f64; 3],
    acceleration_m_s2: [f64; 3],
    temperature_c: Option<f64>,
    calibration_revision: serde_json::Value,
    reference_frame: String,
    time_quality: String,
    heading_calibrated: bool,
    decision_eligible: bool,
    observation_only: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Rejection {
    NotImu,
    InvalidJson,
    WrongSchema,
    WrongSource,
    InvalidProvenance,
    InvalidTimestamp,
    InvalidValue,
    Replay,
}

fn to_i16(value: f64, scale: f64, abs_max: f64) -> Result<i16, Rejection> {
    if !value.is_finite() || value.abs() > abs_max {
        return Err(Rejection::InvalidValue);
    }
    let scaled = (value * scale).round();
    if scaled < i16::MIN as f64 || scaled > i16::MAX as f64 {
        return Err(Rejection::InvalidValue);
    }
    Ok(scaled as i16)
}

fn to_i32(value: f64, scale: f64, abs_max: f64) -> Result<i32, Rejection> {
    if !value.is_finite() || value.abs() > abs_max {
        return Err(Rejection::InvalidValue);
    }
    let scaled = (value * scale).round();
    if scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        return Err(Rejection::InvalidValue);
    }
    Ok(scaled as i32)
}

fn parse_session_uuid(value: &str) -> Result<[u8; 16], Rejection> {
    if value.len() != 32 || !value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(Rejection::InvalidProvenance);
    }
    let mut out = [0; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| Rejection::InvalidProvenance)?;
    }
    if out == [0u8; 16] {
        // An all-zero session is used as a placeholder in the handoff, not
        // a real UUID4 publisher run.
        return Err(Rejection::InvalidProvenance);
    }
    Ok(out)
}

fn validate_observation(
    obs: ConsoleSample,
    now_host_monotonic_ns: u64,
    origin: Origin,
) -> Result<(ImuSample, u64), Rejection> {
    if obs.schema_version != CONSOLE_READ_SCHEMA {
        return Err(Rejection::WrongSchema);
    }
    if obs.robot_id != ROBOT_ID
        || obs.sensor_id != SENSOR_ID
        || obs.source_id != SOURCE_ID
        || obs.sensor_frame_id != SENSOR_FRAME_ID
    {
        return Err(Rejection::WrongSource);
    }
    if obs.source_kind != "physical"
        || obs.reference_frame != REFERENCE_FRAME
        || obs.time_quality != TIME_QUALITY
        || obs.heading_calibrated
        || obs.decision_eligible
        || !obs.observation_only
        || !obs.calibration_revision.is_null()
    {
        return Err(Rejection::InvalidProvenance);
    }
    if obs.sequence == 0
        || obs.host_monotonic_ns == 0
        || obs.host_receive_utc.is_empty()
        || obs.console_received_at.is_empty()
    {
        return Err(Rejection::InvalidTimestamp);
    }
    // Same Ubuntu host CLOCK_MONOTONIC source as the Python IMU reader.
    // A future host receipt is never a valid live sample; fail closed.
    if obs.host_monotonic_ns > now_host_monotonic_ns {
        return Err(Rejection::InvalidTimestamp);
    }
    let age_ms = now_host_monotonic_ns.saturating_sub(obs.host_monotonic_ns) / 1_000_000;
    if age_ms > MAX_FORWARD_AGE_MS as u64 {
        // The Console itself does not claim an observation older than 3s
        // is available in REST latest. Reject stale WebSocket backlog.
        return Err(Rejection::InvalidTimestamp);
    }
    let source_session = parse_session_uuid(&obs.session_id)?;
    let angles = [
        to_i16(obs.orientation_euler_deg[0], 100.0, 180.0)?,
        to_i16(obs.orientation_euler_deg[1], 100.0, 180.0)?,
        to_i16(obs.orientation_euler_deg[2], 100.0, 180.0)?,
    ];
    let mut gyro_mrad_s = [0; 3];
    let mut accel_mm_s2 = [0; 3];
    for index in 0..3 {
        gyro_mrad_s[index] = to_i32(obs.angular_velocity_rad_s[index], 1000.0, 50.0)?;
        accel_mm_s2[index] = to_i32(obs.acceleration_m_s2[index], 1000.0, 160.0)?;
    }
    let sample = ImuSample {
        origin,
        frame_quality: FrameQuality::SensorAxesUnverified,
        source_session,
        source_sequence: obs.sequence,
        roll_cdeg: angles[0],
        pitch_cdeg: angles[1],
        yaw_cdeg: angles[2],
        gyro_mrad_s,
        accel_mm_s2,
        temp_cdeg: match obs.temperature_c {
            Some(t) => Some(to_i16(t, 100.0, 200.0)?),
            None => None,
        },
        host_age_at_forward_ms: if origin == Origin::ConsoleReadOnlyObservation {
            Some(age_ms as u32)
        } else {
            None
        },
    };
    Ok((sample, obs.host_monotonic_ns))
}

/// Validates the full read-side envelope; other events (hello, heartbeat,
/// telemetry, event) are ignored, never presented as IMU measurements.
/// The Origin passed here may be trusted-local only after validating the
/// existing Console loopback endpoint and its returned WS handshake.
pub fn parse_stream_frame(
    json: &str,
    now_host_monotonic_ns: u64,
    origin: Origin,
) -> Result<Option<(ImuSample, u64)>, Rejection> {
    if json.len() > MAX_JSON_BYTES {
        return Err(Rejection::InvalidJson);
    }
    let root: serde_json::Value = serde_json::from_str(json).map_err(|_| Rejection::InvalidJson)?;
    if root.get("type").and_then(serde_json::Value::as_str) != Some("imu") {
        return Ok(None);
    }
    let env: Envelope = serde_json::from_value(root).map_err(|_| Rejection::InvalidJson)?;
    if env.kind != "imu"
        || env.schema_version != CONSOLE_STREAM_SCHEMA
        || env.robot_id != ROBOT_ID
        || env.emitted_at.is_empty()
    {
        return Err(Rejection::WrongSchema);
    }
    let obs: ConsoleSample =
        serde_json::from_value(env.data).map_err(|_| Rejection::InvalidJson)?;
    let sample = validate_observation(obs, now_host_monotonic_ns, origin)?;
    Ok(Some(sample))
}

/// Strict parser for the documented read-only REST latest endpoint
/// (null means no fresh observation according to Console's 3s TTL).
pub fn parse_rest_latest(
    json: &str,
    now_host_monotonic_ns: u64,
) -> Result<Option<(ImuSample, u64)>, Rejection> {
    if json.len() > MAX_JSON_BYTES {
        return Err(Rejection::InvalidJson);
    }
    let root: serde_json::Value = serde_json::from_str(json).map_err(|_| Rejection::InvalidJson)?;
    if root.is_null() {
        return Ok(None);
    }
    let obs: ConsoleSample = serde_json::from_value(root).map_err(|_| Rejection::InvalidJson)?;
    let sample = validate_observation(obs, now_host_monotonic_ns, Origin::ImportedJsonUnverified)?;
    Ok(Some(sample))
}

#[derive(Default)]
pub struct StreamGuard {
    pub imu: ImuView,
    last_receipt_ns: u64,
    source_session: Option<[u8; 16]>,
    source_sequence: u64,
}

impl StreamGuard {
    pub fn reset(&mut self) {
        self.imu.clear();
        self.last_receipt_ns = 0;
        self.source_sequence = 0;
        self.source_session = None;
    }

    /// Same-session seq strictly increases. Host-receive timestamp may stay
    /// identical across adjacent packet bursts, but may NEVER regress.
    pub fn accept(
        &mut self,
        sample: ImuSample,
        source_timestamp_ns: u64,
        now_host_monotonic_ns: u64,
    ) -> Result<(), Rejection> {
        if let Some(session) = self.source_session {
            if sample.source_session == session
                && (sample.source_sequence <= self.source_sequence
                    || source_timestamp_ns < self.last_receipt_ns)
            {
                return Err(Rejection::Replay);
            }
        }
        // Validation on actual hardware is separate from source validation.
        // Passing host-monotonic receipt time to this view is not used by Pico.
        self.imu
            .observe(sample, now_host_monotonic_ns / 1_000_000)
            .map_err(|_| Rejection::InvalidValue)?;
        self.source_session = Some(sample.source_session);
        self.source_sequence = sample.source_sequence;
        self.last_receipt_ns = source_timestamp_ns;
        Ok(())
    }
}

fn to_wire_i16(value_milli: i32) -> Result<i16, Rejection> {
    let scaled = (value_milli as f64 / 10.0).round();
    if scaled < i16::MIN as f64 || scaled > i16::MAX as f64 {
        return Err(Rejection::InvalidValue);
    }
    Ok(scaled as i16)
}

/// Conversion is allowed only after full Console schema/source validation
/// and trusted-local WS origin. No synthetic or offline data can be
/// transmitted as a physical sample.
pub fn to_wire(sample: ImuSample) -> Result<imu_wire::Sample, Rejection> {
    if sample.origin != Origin::ConsoleReadOnlyObservation
        || sample.frame_quality != FrameQuality::SensorAxesUnverified
    {
        return Err(Rejection::InvalidProvenance);
    }
    let age = sample
        .host_age_at_forward_ms
        .ok_or(Rejection::InvalidTimestamp)?;
    Ok(imu_wire::Sample {
        session: sample.source_session,
        sequence: sample.source_sequence,
        age_at_host_forward_ms: age,
        angles_cdeg: [sample.roll_cdeg, sample.pitch_cdeg, sample.yaw_cdeg],
        gyro_centirad_s: [
            to_wire_i16(sample.gyro_mrad_s[0])?,
            to_wire_i16(sample.gyro_mrad_s[1])?,
            to_wire_i16(sample.gyro_mrad_s[2])?,
        ],
        accel_centims2: [
            to_wire_i16(sample.accel_mm_s2[0])?,
            to_wire_i16(sample.accel_mm_s2[1])?,
            to_wire_i16(sample.accel_mm_s2[2])?,
        ],
        temperature_cdeg: sample.temp_cdeg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulse_protocol::imu_wire::{decode, encode, Frame};

    const OBS: &str = r#"{
      "schema_version": "console.imu.v0.1-candidate",
      "robot_id": "paky-01",
      "sensor_id": "torso-imu-01",
      "source_id": "paky01-wit-serial",
      "sensor_frame_id": "paky01_imu_sensor_unverified",
      "session_id": "00112233445566778899aabbccddeeff",
      "sequence": 106,
      "source_kind": "physical",
      "host_receive_utc": "2026-10-24T14:41:50.462850+00:00",
      "host_monotonic_ns": 96892402830994,
      "console_received_at": "2026-10-24T14:41:50.464912049+00:00",
      "orientation_euler_deg": [-4.81201171875,-7.3553466796875,-148.1011962890625],
      "angular_velocity_rad_s": [0,0,0],
      "acceleration_m_s2": [1.2593500732421874,-0.8188169677734375,9.749189160156249],
      "temperature_c": 24.01,
      "calibration_revision": null,
      "reference_frame": "imu_sensor_axes_unverified",
      "time_quality": "host_receive_only_not_chronograph",
      "heading_calibrated": false,
      "decision_eligible": false,
      "observation_only": true,
      "motion_hint": "low_angular_rate"
    }"#;
    const HOST_MONO_NOW: u64 = 96892502830994;

    fn envelope() -> String {
        format!(
            r#"{{"schema_version":"console.v1","type":"imu","robot_id":"paky-01","emitted_at":"2026-10-24T14:41:50Z","data":{OBS}}}"#
        )
    }

    #[test]
    fn actual_console_schema_quantizes_to_imu_wire_without_using_demo_ticks() {
        let (sample, _) = parse_stream_frame(
            &envelope(),
            HOST_MONO_NOW,
            Origin::ConsoleReadOnlyObservation,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (sample.roll_cdeg, sample.pitch_cdeg, sample.yaw_cdeg),
            (-481, -736, -14810)
        );
        assert_eq!(sample.accel_mm_s2, [1259, -819, 9749]);
        assert_eq!(sample.temp_cdeg, Some(2401));
        assert_eq!(sample.host_age_at_forward_ms, Some(100));
        let payload = to_wire(sample).unwrap();
        assert_eq!(
            decode(&encode(Frame::SensorSample(payload))),
            Ok(Frame::SensorSample(payload))
        );
        assert_eq!(payload.gyro_centirad_s, [0, 0, 0]);
    }

    #[test]
    fn nonimu_stream_events_are_ignored() {
        for t in ["heartbeat", "hello", "telemetry", "event"] {
            let message = format!(r#"{{"type":"{t}","data":{{}}}}"#);
            assert_eq!(
                parse_stream_frame(&message, HOST_MONO_NOW, Origin::ConsoleReadOnlyObservation),
                Ok(None)
            );
        }
    }

    #[test]
    fn reject_wrong_consumer_contract_source_and_eligibility_claims() {
        let obs = envelope();
        assert_eq!(
            parse_stream_frame(
                &obs.replace(
                    "console.imu.v0.1-candidate",
                    "openrgd.imu-sample.v0.1-candidate"
                ),
                HOST_MONO_NOW,
                Origin::ConsoleReadOnlyObservation
            ),
            Err(Rejection::WrongSchema)
        );
        assert_eq!(
            parse_stream_frame(
                &obs.replace("\"observation_only\": true", "\"observation_only\": false"),
                HOST_MONO_NOW,
                Origin::ConsoleReadOnlyObservation
            ),
            Err(Rejection::InvalidProvenance)
        );
        assert_eq!(
            parse_stream_frame(
                &obs.replace("paky01-wit-serial", "fake-imu"),
                HOST_MONO_NOW,
                Origin::ConsoleReadOnlyObservation
            ),
            Err(Rejection::WrongSource)
        );
    }

    #[test]
    fn allow_nonincreasing_host_timestamp_only_with_increasing_sequence() {
        let (s, t) = parse_stream_frame(
            &envelope(),
            HOST_MONO_NOW,
            Origin::ConsoleReadOnlyObservation,
        )
        .unwrap()
        .unwrap();
        let mut g = StreamGuard::default();
        g.accept(s, t, HOST_MONO_NOW).unwrap();
        assert_eq!(g.accept(s, t, HOST_MONO_NOW), Err(Rejection::Replay));
        let mut fresh = s;
        fresh.source_sequence += 1;
        g.accept(fresh, t, HOST_MONO_NOW).unwrap();
        fresh.source_sequence += 1;
        assert_eq!(
            g.accept(fresh, t - 1, HOST_MONO_NOW),
            Err(Rejection::Replay)
        );
    }

    #[test]
    fn rest_null_is_no_source_and_sample_untrusted_until_ws_subscription() {
        assert_eq!(parse_rest_latest("null", HOST_MONO_NOW), Ok(None));
        let (s, _) = parse_rest_latest(OBS, HOST_MONO_NOW).unwrap().unwrap();
        assert_eq!(s.origin, Origin::ImportedJsonUnverified);
        assert_eq!(to_wire(s), Err(Rejection::InvalidProvenance));
    }

    #[test]
    fn reject_stale_unknown_or_future_clock_source() {
        assert_eq!(
            parse_stream_frame(
                &envelope(),
                HOST_MONO_NOW + 3_001_000_000,
                Origin::ConsoleReadOnlyObservation
            ),
            Err(Rejection::InvalidTimestamp)
        );
        assert_eq!(
            parse_stream_frame(
                &envelope(),
                HOST_MONO_NOW - 150_000_000,
                Origin::ConsoleReadOnlyObservation
            ),
            Err(Rejection::InvalidTimestamp)
        );
    }
}
