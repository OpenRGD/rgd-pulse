#![no_std]

//! PULSE/1: minimal telemetry transport frame for synthetic testing.
//! No motion commands exist. A receiver must never infer physical safety from a frame.

pub const FRAME_LEN: usize = 18;
pub const VERSION: u8 = 1;
const MAGIC: [u8; 2] = *b"RP";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Hello = 1,
    LoopTicks = 2,
    Event = 3,
}

impl Kind {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Hello),
            2 => Some(Self::LoopTicks),
            3 => Some(Self::Event),
            _ => None,
        }
    }
}

/// The v0.1 layout is 18 bytes:
/// RP (2), version (1), kind (1), seq LE (4), sender uptime_ms LE (4),
/// scalar value LE (4), CRC16-CCITT-FALSE LE (2).
///
/// `value` for Kind::LoopTicks is the observed Somatic loop counter,
/// *not* an inferred frequency or a safety assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub kind: Kind,
    pub sequence: u32,
    pub uptime_ms: u32,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Length,
    Magic,
    Version,
    Kind,
    Checksum,
}

pub fn encode(frame: Frame) -> [u8; FRAME_LEN] {
    let mut out = [0u8; FRAME_LEN];
    out[0..2].copy_from_slice(&MAGIC);
    out[2] = VERSION;
    out[3] = frame.kind as u8;
    out[4..8].copy_from_slice(&frame.sequence.to_le_bytes());
    out[8..12].copy_from_slice(&frame.uptime_ms.to_le_bytes());
    out[12..16].copy_from_slice(&frame.value.to_le_bytes());
    let checksum = crc16_ccitt_false(&out[..16]);
    out[16..18].copy_from_slice(&checksum.to_le_bytes());
    out
}

pub fn decode(raw: &[u8]) -> Result<Frame, DecodeError> {
    if raw.len() != FRAME_LEN {
        return Err(DecodeError::Length);
    }
    if raw[0..2] != MAGIC {
        return Err(DecodeError::Magic);
    }
    if raw[2] != VERSION {
        return Err(DecodeError::Version);
    }
    let kind = Kind::from_u8(raw[3]).ok_or(DecodeError::Kind)?;
    let got = u16::from_le_bytes([raw[16], raw[17]]);
    if got != crc16_ccitt_false(&raw[..16]) {
        return Err(DecodeError::Checksum);
    }
    Ok(Frame {
        kind,
        sequence: u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        uptime_ms: u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]),
        value: u32::from_le_bytes([raw[12], raw[13], raw[14], raw[15]]),
    })
}

pub fn crc16_ccitt_false(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &byte in bytes {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Frame {
        Frame {
            kind: Kind::LoopTicks,
            sequence: 42,
            uptime_ms: 1234,
            value: 98000,
        }
    }

    #[test]
    fn round_trip() {
        assert_eq!(decode(&encode(example())), Ok(example()));
    }

    #[test]
    fn checksum_detects_modification() {
        let mut raw = encode(example());
        raw[12] ^= 1;
        assert_eq!(decode(&raw), Err(DecodeError::Checksum));
    }

    #[test]
    fn rejects_unknown_version_and_type() {
        let mut raw = encode(example());
        raw[2] = 99;
        assert_eq!(decode(&raw), Err(DecodeError::Version));
        raw = encode(example());
        raw[3] = 254;
        assert_eq!(decode(&raw), Err(DecodeError::Kind));
    }

    #[test]
    fn rejects_truncation_and_magic() {
        let raw = encode(example());
        assert_eq!(decode(&raw[..17]), Err(DecodeError::Length));
        let mut raw = raw;
        raw[0] = b'Z';
        assert_eq!(decode(&raw), Err(DecodeError::Magic));
    }

    #[test]
    fn known_crc_vector() {
        assert_eq!(crc16_ccitt_false(b"123456789"), 0x29b1);
    }
}
