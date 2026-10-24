//! Candidate typed, read-only IMU observation model for Chronoscope.
//!
//! This module does NOT open serial ports, query the Console, transmit USB,
//! calibrate yaw/torso attitude, or control actuators. The future verified
//! host-side fan-out must produce these bounded values only after validating
//! the authenticated Console observation provenance and source times.
//!
//! Physical source: paky-01 WIT/JY901-compatible Hiwonder IM10A, published by
//! scripts/imu_publish.py into the local Console IMU observation ingress.
//! Do not mistake host receive time for sensor sampling time or Chronograph.

pub const ROBOT_ID: &str = "paky-01";
pub const SENSOR_ID: &str = "torso-imu-01";
pub const SOURCE_ID: &str = "paky01-wit-serial";
pub const SENSOR_FRAME_ID: &str = "paky01_imu_sensor_unverified";
pub const CONSOLE_OBSERVATION_SCHEMA: &str = "openrgd.imu-sample.v0.1-candidate";

/// HMI freshness threshold (not a sensor quality or real-time control limit).
pub const DISPLAY_STALE_AFTER_MS: u64 = 800;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin {
    /// Accepted source record from the Console read-only subscriber.
    /// Authentication, origin and allowlist validation happen on the host.
    ConsoleReadOnlyObservation,
    /// Deterministic local illustration, never represent as real measurement.
    SyntheticFixture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameQuality {
    /// Current Paky-01 IMU transform and absolute yaw are not calibrated.
    SensorAxesUnverified,
}

/// Quantized sensor-axis observation (NOT world or calibrated robot pose).
///
/// - angles: centidegrees: 1234 means 12.34 degrees
/// - gyro: milliradians per second
/// - acceleration: millimeters per second squared
/// - temperature: centi-Celsius, optional
///
/// The producer must preserve the original full source session UUID in its
/// audited host-side record; source_session is the decoded 16-byte binary
/// UUID. This is NOT an authentication token.
/// time age is host-monotonic observation age at USB-forward time (not sensor
/// hardware time). Host & MCU monotonic clocks are never compared directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImuSample {
    pub origin: Origin,
    pub frame_quality: FrameQuality,
    pub source_session: [u8; 16],
    pub source_sequence: u64,
    pub roll_cdeg: i16,
    pub pitch_cdeg: i16,
    pub yaw_cdeg: i16,
    pub gyro_mrad_s: [i32; 3],
    pub accel_mm_s2: [i32; 3],
    pub temp_cdeg: Option<i16>,
    /// None means origin age unknown: must NOT be rendered fresh.
    pub host_age_at_forward_ms: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImuStatus {
    NoSource,
    Synthetic,
    AgeUnknown,
    FreshSensorAxes,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImuError {
    InvalidAngle,
    InvalidSequence,
}

/// One fixed-size slot, no heap and no transport coupling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImuView {
    latest: Option<ImuSample>,
    received_on_pico_ms: u64,
}

impl Default for ImuView {
    fn default() -> Self {
        Self::new()
    }
}

impl ImuView {
    pub const fn new() -> Self {
        Self {
            latest: None,
            received_on_pico_ms: 0,
        }
    }

    pub const fn sample(&self) -> Option<&ImuSample> {
        self.latest.as_ref()
    }

    pub fn clear(&mut self) {
        self.latest = None;
    }

    /// This validates bounds/replay, not transport authorization or calibration.
    /// Call only from a separately reviewed, source-aware read-only bridge.
    pub fn observe(&mut self, sample: ImuSample, pico_receipt_ms: u64) -> Result<(), ImuError> {
        if !(-18000..=18000).contains(&sample.roll_cdeg)
            || !(-18000..=18000).contains(&sample.pitch_cdeg)
            || !(-18000..=18000).contains(&sample.yaw_cdeg)
        {
            return Err(ImuError::InvalidAngle);
        }
        if sample.source_sequence == 0 {
            return Err(ImuError::InvalidSequence);
        }
        if let Some(last) = self.latest {
            if last.source_session == sample.source_session
                && sample.source_sequence <= last.source_sequence
            {
                return Err(ImuError::InvalidSequence);
            }
        }
        self.latest = Some(sample);
        self.received_on_pico_ms = pico_receipt_ms;
        Ok(())
    }

    /// Returns age in ms as *host-observation age at transmission* plus
    /// MCU-receipt-relative elapsed time. Does not compare unrelated clocks.
    pub fn observed_age_ms(&self, now_pico_ms: u64) -> Option<u64> {
        let sample = self.latest?;
        let age_on_host = sample.host_age_at_forward_ms? as u64;
        Some(age_on_host.saturating_add(now_pico_ms.saturating_sub(self.received_on_pico_ms)))
    }

    pub fn status(&self, now_pico_ms: u64) -> ImuStatus {
        let sample = match self.latest {
            Some(s) => s,
            None => return ImuStatus::NoSource,
        };
        if sample.origin == Origin::SyntheticFixture {
            return ImuStatus::Synthetic;
        }
        match self.observed_age_ms(now_pico_ms) {
            None => ImuStatus::AgeUnknown,
            Some(age) if age <= DISPLAY_STALE_AFTER_MS => ImuStatus::FreshSensorAxes,
            Some(_) => ImuStatus::Stale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> ImuSample {
        ImuSample {
            origin: Origin::ConsoleReadOnlyObservation,
            frame_quality: FrameQuality::SensorAxesUnverified,
            source_session: [1; 16],
            source_sequence: 1,
            roll_cdeg: -460,
            pitch_cdeg: -483,
            yaw_cdeg: -14987,
            gyro_mrad_s: [0, 0, 0],
            accel_mm_s2: [0, 0, 9807],
            temp_cdeg: None,
            host_age_at_forward_ms: Some(50),
        }
    }

    #[test]
    fn no_source_never_claims_live_or_zero_degree_attitude() {
        let model = ImuView::new();
        assert_eq!(model.status(0), ImuStatus::NoSource);
        assert_eq!(model.sample(), None);
    }

    #[test]
    fn fresh_uncalibrated_sensor_sample_is_not_robot_attitude() {
        let mut view = ImuView::new();
        let s = candidate();
        view.observe(s, 5000).unwrap();
        assert_eq!(view.status(5000), ImuStatus::FreshSensorAxes);
        assert_eq!(view.observed_age_ms(5099), Some(149));
        assert_eq!(
            view.sample().unwrap().frame_quality,
            FrameQuality::SensorAxesUnverified
        );
    }

    #[test]
    fn stale_timeout_adds_receive_relative_elapsed_time() {
        let mut view = ImuView::new();
        view.observe(candidate(), 1000).unwrap();
        assert_eq!(view.status(1750), ImuStatus::FreshSensorAxes);
        assert_eq!(view.status(1751), ImuStatus::Stale);
        assert_eq!(view.status(900), ImuStatus::FreshSensorAxes); // no negative time
    }

    #[test]
    fn host_age_missing_must_never_be_called_fresh() {
        let mut view = ImuView::new();
        let mut s = candidate();
        s.host_age_at_forward_ms = None;
        view.observe(s, 0).unwrap();
        assert_eq!(view.status(0), ImuStatus::AgeUnknown);
    }

    #[test]
    fn synthetic_source_remains_synthetic_even_when_fresh() {
        let mut view = ImuView::new();
        let mut s = candidate();
        s.origin = Origin::SyntheticFixture;
        view.observe(s, 10).unwrap();
        assert_eq!(view.status(12), ImuStatus::Synthetic);
    }

    #[test]
    fn reject_bad_angle_and_replayed_samples_without_mutation() {
        let mut view = ImuView::new();
        let initial = candidate();
        view.observe(initial, 10).unwrap();
        assert_eq!(view.observe(initial, 20), Err(ImuError::InvalidSequence));
        let mut illegal = initial;
        illegal.source_sequence = 2;
        illegal.pitch_cdeg = 18100;
        assert_eq!(view.observe(illegal, 30), Err(ImuError::InvalidAngle));
        assert_eq!(view.sample().unwrap().source_sequence, 1);
        assert_eq!(view.observed_age_ms(31), Some(71));
    }

    #[test]
    fn source_session_change_can_start_new_sequence() {
        let mut view = ImuView::new();
        let mut s = candidate();
        s.source_sequence = 99;
        view.observe(s, 300).unwrap();
        s.source_session = [2; 16];
        s.source_sequence = 1;
        view.observe(s, 350).unwrap();
        assert_eq!(view.status(351), ImuStatus::FreshSensorAxes);
    }

    #[test]
    fn timestamp_and_age_saturate_without_false_clock_sync() {
        let mut view = ImuView::new();
        let mut s = candidate();
        s.host_age_at_forward_ms = Some(u32::MAX);
        view.observe(s, 10).unwrap();
        assert_eq!(view.status(u64::MAX), ImuStatus::Stale);
        assert_eq!(view.observed_age_ms(u64::MAX), Some(u64::MAX));
    }
}
