//! PULSE/IMU-1: separate bounded, CRC-checked, *read-only* telemetry
//! messages over the same Pico USB CDC link as the older PULSE/1 DEMO
//! frames. The magic "IM" and explicit probe/ack handshake avoid silently
//! treating Kind::DemoTicks as physical sensor observations.
//!
//! 56 bytes per frame: fits one USB CDC 64-byte packet, but still decode as
//! a byte stream (fragments/noise/concatenation are possible). This
//! candidate has no motion, calibration or write-to-sensor commands.
//!
//! Bytes:
//! 0..2  "IM"; 2 version=1; 3 kind (1 Probe / 2 ProbeAck / 3 Sample)
//! 4 flags (Sample=0x05 | optional temperature 0x02); 5 reserved=0
//! 6..22 session UUID; 22..30 publisher sequence LE
//! 30..34 observation age at host forward, milliseconds LE
//! 34..40 roll/pitch/yaw signed centidegrees LE
//! 40..46 3-axis gyro signed centirad/s LE
//! 46..52 3-axis acceleration signed centim/s² LE
//! 52..54 temperature centi-C, or 0 when absent
//! 54..56 CRC16 CCITT-FALSE LE over first 54 bytes.
//!
//! All samples describe UNCALIBRATED SENSOR AXES, not torso/world pose;
//! yaw is not an absolute compass heading. Source validation is performed
//! by trusted-local pulse-bridge, not cryptographically authenticated
//! by this prototype transport. The Pico NEVER uses these for actuation.

use crate::crc16_ccitt_false;

pub const FRAME_LEN: usize = 56;
pub const VERSION: u8 = 1;
const MAGIC: [u8; 2] = *b"IM";
const FLAG_PHYSICAL: u8 = 0x01;
const FLAG_TEMP_PRESENT: u8 = 0x02;
const FLAG_SENSOR_AXES_UNVERIFIED: u8 = 0x04;
const FLAG_REQUIRED: u8 = FLAG_PHYSICAL | FLAG_SENSOR_AXES_UNVERIFIED;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Kind {
    Probe = 1,
    ProbeAck = 2,
    SensorSample = 3,
}

impl Kind {
    const fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Probe),
            2 => Some(Self::ProbeAck),
            3 => Some(Self::SensorSample),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sample {
    pub session: [u8; 16],
    pub sequence: u64,
    pub age_at_host_forward_ms: u32,
    pub angles_cdeg: [i16; 3],
    pub gyro_centirad_s: [i16; 3],
    pub accel_centims2: [i16; 3],
    pub temperature_cdeg: Option<i16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Frame {
    Probe,
    ProbeAck,
    SensorSample(Sample),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Length,
    Magic,
    Version,
    Kind,
    Flags,
    Reserved,
    Checksum,
    EmptySessionOrSequence,
}

pub fn encode(frame: Frame) -> [u8; FRAME_LEN] {
    let mut out = [0u8; FRAME_LEN];
    out[..2].copy_from_slice(&MAGIC);
    out[2] = VERSION;
    match frame {
        Frame::Probe => out[3] = Kind::Probe as u8,
        Frame::ProbeAck => out[3] = Kind::ProbeAck as u8,
        Frame::SensorSample(sample) => {
            out[3] = Kind::SensorSample as u8;
            out[4] = FLAG_REQUIRED;
            if sample.temperature_cdeg.is_some() {
                out[4] |= FLAG_TEMP_PRESENT;
            }
            out[6..22].copy_from_slice(&sample.session);
            out[22..30].copy_from_slice(&sample.sequence.to_le_bytes());
            out[30..34].copy_from_slice(&sample.age_at_host_forward_ms.to_le_bytes());
            for (i, value) in sample.angles_cdeg.iter().enumerate() {
                let start = 34 + 2 * i;
                out[start..start + 2].copy_from_slice(&value.to_le_bytes());
            }
            for (i, value) in sample.gyro_centirad_s.iter().enumerate() {
                let start = 40 + 2 * i;
                out[start..start + 2].copy_from_slice(&value.to_le_bytes());
            }
            for (i, value) in sample.accel_centims2.iter().enumerate() {
                let start = 46 + 2 * i;
                out[start..start + 2].copy_from_slice(&value.to_le_bytes());
            }
            if let Some(temp) = sample.temperature_cdeg {
                out[52..54].copy_from_slice(&temp.to_le_bytes());
            }
        }
    }
    let checksum = crc16_ccitt_false(&out[..54]);
    out[54..56].copy_from_slice(&checksum.to_le_bytes());
    out
}

fn get_i16(raw: &[u8], start: usize) -> i16 {
    i16::from_le_bytes([raw[start], raw[start + 1]])
}

pub fn decode(raw: &[u8]) -> Result<Frame, Error> {
    if raw.len() != FRAME_LEN {
        return Err(Error::Length);
    }
    if raw[..2] != MAGIC {
        return Err(Error::Magic);
    }
    if raw[2] != VERSION {
        return Err(Error::Version);
    }
    let kind = Kind::from_byte(raw[3]).ok_or(Error::Kind)?;
    if raw[5] != 0 {
        return Err(Error::Reserved);
    }
    if u16::from_le_bytes([raw[54], raw[55]]) != crc16_ccitt_false(&raw[..54]) {
        return Err(Error::Checksum);
    }
    match kind {
        Kind::Probe | Kind::ProbeAck => {
            if raw[4] != 0 || raw[6..54].iter().any(|b| *b != 0) {
                return Err(Error::Flags);
            }
            Ok(if kind == Kind::Probe {
                Frame::Probe
            } else {
                Frame::ProbeAck
            })
        }
        Kind::SensorSample => {
            if raw[4] & !0x07 != 0 || raw[4] & FLAG_REQUIRED != FLAG_REQUIRED {
                return Err(Error::Flags);
            }
            let sequence = u64::from_le_bytes(raw[22..30].try_into().unwrap());
            let mut session = [0u8; 16];
            session.copy_from_slice(&raw[6..22]);
            if sequence == 0 || session == [0u8; 16] {
                return Err(Error::EmptySessionOrSequence);
            }
            if raw[4] & FLAG_TEMP_PRESENT == 0 && get_i16(raw, 52) != 0 {
                return Err(Error::Flags);
            }
            let mut angles_cdeg = [0i16; 3];
            let mut gyro_centirad_s = [0i16; 3];
            let mut accel_centims2 = [0i16; 3];
            for i in 0..3 {
                angles_cdeg[i] = get_i16(raw, 34 + 2 * i);
                gyro_centirad_s[i] = get_i16(raw, 40 + 2 * i);
                accel_centims2[i] = get_i16(raw, 46 + 2 * i);
            }
            Ok(Frame::SensorSample(Sample {
                session,
                sequence,
                age_at_host_forward_ms: u32::from_le_bytes(raw[30..34].try_into().unwrap()),
                angles_cdeg,
                gyro_centirad_s,
                accel_centims2,
                temperature_cdeg: if raw[4] & FLAG_TEMP_PRESENT != 0 {
                    Some(get_i16(raw, 52))
                } else {
                    None
                },
            }))
        }
    }
}

/// Incremental, independent stream decoder; won't mistake RP/PULSE-1
/// bytes for IMU capability/observation frames.
pub struct Decoder {
    bytes: [u8; FRAME_LEN],
    len: usize,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            bytes: [0; FRAME_LEN],
            len: 0,
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
    }

    pub fn push(&mut self, byte: u8) -> Option<Result<Frame, Error>> {
        if self.len == 0 {
            if byte == MAGIC[0] {
                self.bytes[0] = byte;
                self.len = 1;
            }
            return None;
        }
        if self.len == 1 && byte != MAGIC[1] {
            if byte != MAGIC[0] {
                self.len = 0;
            }
            return None;
        }
        self.bytes[self.len] = byte;
        self.len += 1;
        if self.len != FRAME_LEN {
            return None;
        }
        let result = decode(&self.bytes);
        if result.is_ok() {
            self.len = 0;
        } else {
            // Preserve embedded IM prefix so damaged/fragmented packets do
            // not prevent later good frames from being decoded.
            let mut found = None;
            for offset in 1..FRAME_LEN - 1 {
                if self.bytes[offset..offset + 2] == MAGIC {
                    found = Some(offset);
                    break;
                }
            }
            if let Some(offset) = found {
                self.bytes.copy_within(offset..FRAME_LEN, 0);
                self.len = FRAME_LEN - offset;
            } else if self.bytes[FRAME_LEN - 1] == MAGIC[0] {
                self.bytes[0] = MAGIC[0];
                self.len = 1;
            } else {
                self.len = 0;
            }
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Sample {
        Sample {
            session: [1; 16],
            sequence: 106,
            age_at_host_forward_ms: 123,
            angles_cdeg: [-481, -736, -14810],
            gyro_centirad_s: [0, 0, 0],
            accel_centims2: [126, -82, 975],
            temperature_cdeg: Some(2401),
        }
    }

    #[test]
    fn probe_ack_and_imu_have_distinct_magic_from_pulse1() {
        for frame in [
            Frame::Probe,
            Frame::ProbeAck,
            Frame::SensorSample(example()),
        ] {
            let encoded = encode(frame);
            assert_eq!(&encoded[..2], b"IM");
            assert_ne!(&encoded[..2], b"RP");
            assert_eq!(decode(&encoded), Ok(frame));
            assert_eq!(encoded.len(), 56);
        }
    }

    #[test]
    fn checksum_catches_corruption_and_decoder_resynchronizes() {
        let mut bad = encode(Frame::SensorSample(example()));
        bad[37] ^= 0xFF;
        assert_eq!(decode(&bad), Err(Error::Checksum));
        let expected = Frame::SensorSample(example());
        let mut decoder = Decoder::new();
        let mut answers = [None, None];
        let mut n = 0;
        for b in bad.into_iter().chain(encode(expected)) {
            if let Some(result) = decoder.push(b) {
                if n < 2 {
                    answers[n] = Some(result);
                    n += 1;
                }
            }
        }
        assert_eq!(answers[0], Some(Err(Error::Checksum)));
        assert_eq!(answers[1], Some(Ok(expected)));
    }

    #[test]
    fn rejects_sample_without_physical_uncalibrated_flags() {
        let mut raw = encode(Frame::SensorSample(example()));
        raw[4] &= !FLAG_SENSOR_AXES_UNVERIFIED;
        let checksum = crc16_ccitt_false(&raw[..54]);
        raw[54..56].copy_from_slice(&checksum.to_le_bytes());
        assert_eq!(decode(&raw), Err(Error::Flags));
        raw = encode(Frame::SensorSample(example()));
        raw[22..30].fill(0);
        let checksum = crc16_ccitt_false(&raw[..54]);
        raw[54..56].copy_from_slice(&checksum.to_le_bytes());
        assert_eq!(decode(&raw), Err(Error::EmptySessionOrSequence));
    }

    #[test]
    fn no_temperature_does_not_invent_sensor_temperature() {
        let mut s = example();
        s.temperature_cdeg = None;
        assert_eq!(
            decode(&encode(Frame::SensorSample(s))),
            Ok(Frame::SensorSample(s))
        );
    }

    #[test]
    fn backward_compatibility_pulse1_decoder_still_receives_demo() {
        use crate::{encode as encode_old, Frame as OldFrame, Kind as OldKind, StreamDecoder};
        let frame = OldFrame {
            kind: OldKind::DemoTicks,
            sequence: 7,
            uptime_ms: 700,
            value: 700,
        };
        let mut imu = Decoder::new();
        let mut original = StreamDecoder::new();
        for b in encode_old(frame) {
            assert!(imu.push(b).is_none());
            if let Some(parsed) = original.push(b) {
                assert_eq!(parsed, Ok(frame));
            }
        }
    }
}
