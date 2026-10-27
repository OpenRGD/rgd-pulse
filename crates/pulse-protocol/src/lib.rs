#![no_std]

//! PULSE/1: minimal telemetry transport frame for synthetic testing.
//! No motion commands exist. A receiver must never infer physical safety from a frame.

pub mod imu_wire;

pub const FRAME_LEN: usize = 18;
pub const VERSION: u8 = 1;
const MAGIC: [u8; 2] = *b"RP";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Hello = 1,
    LoopTicks = 2,
    Event = 3,
    /// Explicitly synthetic fixture: never present as real loop telemetry.
    DemoTicks = 4,
    /// Pico-to-host receipt of a valid, fresh DemoTicks frame (not real telemetry).
    DemoAck = 5,
}

impl Kind {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Hello),
            2 => Some(Self::LoopTicks),
            3 => Some(Self::Event),
            4 => Some(Self::DemoTicks),
            5 => Some(Self::DemoAck),
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

/// Incremental CDC byte-stream decoder. A USB packet is NOT a Pulse frame
/// boundary: bytes may arrive fragmented, concatenated or after garbage.
///
/// Accepts no commands, allocates no memory and recovers after CRC errors.
pub struct StreamDecoder {
    buffer: [u8; FRAME_LEN],
    len: usize,
}

impl Default for StreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamDecoder {
    pub const fn new() -> Self {
        Self {
            buffer: [0; FRAME_LEN],
            len: 0,
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
    }

    pub fn push(&mut self, byte: u8) -> Option<Result<Frame, DecodeError>> {
        if self.len == 0 {
            if byte == MAGIC[0] {
                self.buffer[0] = byte;
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

        self.buffer[self.len] = byte;
        self.len += 1;

        if self.len != FRAME_LEN {
            return None;
        }

        let result = decode(&self.buffer);
        if result.is_ok() {
            self.len = 0;
        } else {
            // Preserve an embedded RP prefix after a malformed frame so the
            // next valid frame is not lost. Handles interspersed noise too.
            let mut found = None;
            for offset in 1..(FRAME_LEN - 1) {
                if self.buffer[offset] == MAGIC[0] && self.buffer[offset + 1] == MAGIC[1] {
                    found = Some(offset);
                    break;
                }
            }
            if let Some(offset) = found {
                self.buffer.copy_within(offset..FRAME_LEN, 0);
                self.len = FRAME_LEN - offset;
            } else if self.buffer[FRAME_LEN - 1] == MAGIC[0] {
                self.buffer[0] = MAGIC[0];
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
    fn demo_ack_round_trip_preserves_sequence_and_value() {
        let ack = Frame {
            kind: Kind::DemoAck,
            sequence: 77,
            uptime_ms: 5000,
            value: 7700,
        };
        assert_eq!(decode(&encode(ack)), Ok(ack));
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
    fn streaming_decoder_handles_noise_and_fragmentation() {
        let expected = example();
        let bytes = encode(expected);
        let mut dec = StreamDecoder::new();
        for byte in [0u8, b'A', b'R', b'R', b'P', 0x7f] {
            assert!(dec.push(byte).is_none());
        }
        let mut received = None;
        for byte in bytes {
            if let Some(result) = dec.push(byte) {
                received = Some(result);
            }
        }
        assert_eq!(received, Some(Ok(expected)));
    }

    #[test]
    fn streaming_decoder_survives_bad_crc_and_resynchronizes() {
        let mut damaged = encode(example());
        damaged[12] ^= 0x01;
        let good = Frame {
            kind: Kind::DemoTicks,
            sequence: 43,
            uptime_ms: 1300,
            value: 123,
        };
        let good_bytes = encode(good);
        let mut dec = StreamDecoder::new();
        let mut outcomes = [None, None];
        let mut index = 0;
        for byte in damaged.into_iter().chain(good_bytes) {
            if let Some(result) = dec.push(byte) {
                outcomes[index] = Some(result);
                index += 1;
            }
        }
        assert_eq!(outcomes[0], Some(Err(DecodeError::Checksum)));
        assert_eq!(outcomes[1], Some(Ok(good)));
    }

    #[test]
    fn streaming_decoder_reassembles_two_packed_frames() {
        let mut dec = StreamDecoder::new();
        let first = example();
        let second = Frame {
            kind: Kind::DemoTicks,
            sequence: 1,
            uptime_ms: 100,
            value: 5,
        };
        let mut outcomes = [None, None];
        let mut index = 0;
        for byte in encode(first).into_iter().chain(encode(second)) {
            if let Some(result) = dec.push(byte) {
                outcomes[index] = Some(result);
                index += 1;
            }
        }
        assert_eq!(outcomes[0], Some(Ok(first)));
        assert_eq!(outcomes[1], Some(Ok(second)));
    }

    #[test]
    fn known_crc_vector() {
        assert_eq!(crc16_ccitt_false(b"123456789"), 0x29b1);
    }
}
