//! Strict host-only parser for *offline* candidate Console IMU JSON.
//!
//! A file on disk is NOT proof of an authenticated, live sensor stream.
//! Never connect to Paky-01's IMU serial port or transmit Pico CDC here.
//! The future Console read-only subscriber must authenticate and verify source
//! provenance before constructing Origin::ConsoleReadOnlyObservation.

use pulse_ui::imu::{
    FrameQuality, ImuSample, Origin, CONSOLE_OBSERVATION_SCHEMA, ROBOT_ID, SENSOR_FRAME_ID,
    SENSOR_ID, SOURCE_ID,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct ConsoleImuJson {
    schema_version: String,
    robot_id: String,
    sensor_id: String,
    source_id: String,
    sensor_frame_id: String,
    session_id: String,
    sequence: u64,
    host_receive_utc: String,
    host_monotonic_ns: u64,
    acceleration_m_s2: [f64; 3],
    angular_velocity_rad_s: [f64; 3],
    orientation_euler_deg: [f64; 3],
    temperature_c: Option<f64>,
    calibration_revision: serde_json::Value,
}

fn quantize_i16(value: f64, factor: f64, max_abs: f64) -> Result<i16, &'static str> {
    if !value.is_finite() || value.abs() > max_abs {
        return Err("nonfinite/out-of-range quantized value");
    }
    let quantized = (value * factor).round();
    if quantized < i16::MIN as f64 || quantized > i16::MAX as f64 {
        return Err("quantized i16 overflow");
    }
    Ok(quantized as i16)
}

fn quantize_i32(value: f64, factor: f64, max_abs: f64) -> Result<i32, &'static str> {
    if !value.is_finite() || value.abs() > max_abs {
        return Err("nonfinite/out-of-range quantized value");
    }
    let quantized = (value * factor).round();
    if quantized < i32::MIN as f64 || quantized > i32::MAX as f64 {
        return Err("quantized i32 overflow");
    }
    Ok(quantized as i32)
}

fn parse_session_uuid_hex(id: &str) -> Result<[u8; 16], &'static str> {
    if id.len() != 32 || !id.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err("session_id must be exactly 32 hexadecimal characters");
    }
    let mut bytes = [0u8; 16];
    for (i, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&id[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid session_id hex")?;
    }
    Ok(bytes)
}

pub fn parse_offline_candidate_json(raw: &str) -> Result<ImuSample, String> {
    if raw.len() > 16 * 1024 {
        return Err("IMU candidate JSON exceeds the 16KiB offline limit".into());
    }
    let obs: ConsoleImuJson =
        serde_json::from_str(raw).map_err(|error| format!("IMU candidate shape invalid: {error}"))?;
    if obs.schema_version != CONSOLE_OBSERVATION_SCHEMA
        || obs.robot_id != ROBOT_ID
        || obs.sensor_id != SENSOR_ID
        || obs.source_id != SOURCE_ID
        || obs.sensor_frame_id != SENSOR_FRAME_ID
    {
        return Err("unsupported IMU source/provenance identifier".into());
    }
    if !obs.calibration_revision.is_null() {
        return Err("calibrated/world-frame source unsupported: require reviewed transform".into());
    }
    if obs.sequence == 0 || obs.host_monotonic_ns == 0 || obs.host_receive_utc.is_empty() {
        return Err("IMU observation missing valid seq/host receipt timestamp".into());
    }
    let source_session = parse_session_uuid_hex(&obs.session_id)
        .map_err(str::to_string)?;
    let mut imu = ImuSample {
        origin: Origin::ImportedJsonUnverified,
        frame_quality: FrameQuality::SensorAxesUnverified,
        source_session,
        source_sequence: obs.sequence,
        roll_cdeg: quantize_i16(obs.orientation_euler_deg[0], 100.0, 180.0)
            .map_err(str::to_string)?,
        pitch_cdeg: quantize_i16(obs.orientation_euler_deg[1], 100.0, 180.0)
            .map_err(str::to_string)?,
        yaw_cdeg: quantize_i16(obs.orientation_euler_deg[2], 100.0, 180.0)
            .map_err(str::to_string)?,
        gyro_mrad_s: [0; 3],
        accel_mm_s2: [0; 3],
        temp_cdeg: None,
        // Not enough trust or clock context to calculate a live age from
        // a file; a real authenticated subscriber must supply source age.
        host_age_at_forward_ms: None,
    };
    for i in 0..3 {
        imu.gyro_mrad_s[i] =
            quantize_i32(obs.angular_velocity_rad_s[i], 1000.0, 1000.0)
                .map_err(str::to_string)?;
        imu.accel_mm_s2[i] =
            quantize_i32(obs.acceleration_m_s2[i], 1000.0, 1000.0)
                .map_err(str::to_string)?;
    }
    if let Some(temp) = obs.temperature_c {
        imu.temp_cdeg = Some(quantize_i16(temp, 100.0, 200.0).map_err(str::to_string)?);
    }
    Ok(imu)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulse_ui::imu::{ImuStatus, ImuView};

    const JSON: &str = r#"{
      "schema_version":"openrgd.imu-sample.v0.1-candidate",
      "robot_id":"paky-01",
      "sensor_id":"torso-imu-01",
      "source_id":"paky01-wit-serial",
      "sensor_frame_id":"paky01_imu_sensor_unverified",
      "session_id":"00112233445566778899aabbccddeeff",
      "sequence":42,
      "host_receive_utc":"2026-10-24T15:00:00+00:00",
      "host_monotonic_ns":1234567000000,
      "orientation_euler_deg":[-4.60,-4.83,-149.87],
      "angular_velocity_rad_s":[0.01,0.02,0.03],
      "acceleration_m_s2":[0.0,0.0,9.80665],
      "temperature_c":null,
      "calibration_revision":null
    }"#;

    #[test]
    fn publisher_exact_json_shape_is_quantized_without_world_transform() {
        let s = parse_offline_candidate_json(JSON).unwrap();
        assert_eq!(s.origin, Origin::ImportedJsonUnverified);
        assert_eq!(s.frame_quality, FrameQuality::SensorAxesUnverified);
        assert_eq!([s.roll_cdeg, s.pitch_cdeg, s.yaw_cdeg], [-460, -483, -14987]);
        assert_eq!(s.gyro_mrad_s, [10, 20, 30]);
        assert_eq!(s.accel_mm_s2, [0, 0, 9807]);
        assert_eq!(s.temp_cdeg, None);
        assert_eq!(s.host_age_at_forward_ms, None);
    }

    #[test]
    fn importing_a_file_cannot_launder_it_into_live_console_telemetry() {
        let mut view = ImuView::new();
        view.observe(parse_offline_candidate_json(JSON).unwrap(), 1000).unwrap();
        assert_eq!(view.status(1001), ImuStatus::UntrustedImport);
    }

    #[test]
    fn reject_source_spoofing_calibrated_claims_and_bad_session_id() {
        assert!(parse_offline_candidate_json(&JSON.replace(
            "paky01-wit-serial",
            "other-source"
        )).is_err());
        assert!(parse_offline_candidate_json(&JSON.replace(
            "\"calibration_revision\":null",
            "\"calibration_revision\":\"fake-world-calibration\""
        )).is_err());
        assert!(parse_offline_candidate_json(&JSON.replace(
            "00112233445566778899aabbccddeeff",
            "bad"
        )).is_err());
    }

    #[test]
    fn reject_overflow_shape_and_sequence_zero() {
        assert!(parse_offline_candidate_json(&JSON.replace(
            "-4.60,-4.83,-149.87",
            "-400.0,-4.83,-149.87"
        )).is_err());
        assert!(parse_offline_candidate_json(&JSON.replace(
            "\"sequence\":42",
            "\"sequence\":0"
        )).is_err());
        assert!(parse_offline_candidate_json(&JSON.replace(
            "[0.0,0.0,9.80665]",
            "[0.0,0.0]"
        )).is_err());
    }
}
