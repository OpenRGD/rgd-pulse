//! PULSE/SYS-1: independent, observation-only host health channel.
//! Magic "SY" prevents confusion with PULSE/1 "RP" and PULSE/IMU-1 "IM".
//! Fixed 18-byte, CRC16-CCITT-FALSE LE frame; never a motor/CAN command.
//!
//! 0..2 magic SY; 2 version=1; 3 kind (1 Probe / 2 ProbeAck / 3 Sample)
//! 4 flags (Sample: 0x01 Linux host measurement); 5 reserved=0
//! 6..10 sequence u32 LE; 10..14 Linux host uptime seconds u32 LE
//! 14 physical host RAM used percent (0..100, MemTotal-MemAvailable)
//! 15 Console API health: 0 unavailable/unverified, 1 validated /healthz
//! 16..18 CRC16-CCITT-FALSE LE over first 16 bytes.
//! Console health NEVER means the robot, CAN, motors or power are ready.

use crate::crc16_ccitt_false;

pub const FRAME_LEN: usize = 18;
pub const VERSION: u8 = 1;
const MAGIC: [u8; 2] = *b"SY";
const FLAG_OBSERVED_LINUX_HOST: u8 = 0x01;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub sequence: u32,
    pub host_uptime_s: u32,
    pub ram_used_percent: u8,
    pub console_health_ok: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Frame {
    Probe,
    ProbeAck,
    Snapshot(Snapshot),
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
    InvalidSample,
}

pub fn encode(frame: Frame) -> [u8; FRAME_LEN] {
    let mut out = [0u8; FRAME_LEN];
    out[0..2].copy_from_slice(&MAGIC);
    out[2] = VERSION;
    match frame {
        Frame::Probe => out[3] = 1,
        Frame::ProbeAck => out[3] = 2,
        Frame::Snapshot(sample) => {
            out[3] = 3;
            out[4] = FLAG_OBSERVED_LINUX_HOST;
            out[6..10].copy_from_slice(&sample.sequence.to_le_bytes());
            out[10..14].copy_from_slice(&sample.host_uptime_s.to_le_bytes());
            out[14] = sample.ram_used_percent;
            out[15] = u8::from(sample.console_health_ok);
        }
    };
    let crc = crc16_ccitt_false(&out[..16]);
    out[16..18].copy_from_slice(&crc.to_le_bytes());
    out
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
    if raw[5] != 0 {
        return Err(Error::Reserved);
    }
    if u16::from_le_bytes([raw[16], raw[17]]) != crc16_ccitt_false(&raw[..16]) {
        return Err(Error::Checksum);
    }
    match raw[3] {
        1 | 2 => {
            if raw[4..16].iter().any(|b| *b != 0) {
                return Err(Error::Flags);
            }
            Ok(if raw[3] == 1 {
                Frame::Probe
            } else {
                Frame::ProbeAck
            })
        }
        3 => {
            if raw[4] != FLAG_OBSERVED_LINUX_HOST {
                return Err(Error::Flags);
            }
            let seq = u32::from_le_bytes([raw[6], raw[7], raw[8], raw[9]]);
            if seq == 0 || raw[14] > 100 || raw[15] > 1 {
                return Err(Error::InvalidSample);
            }
            Ok(Frame::Snapshot(Snapshot {
                sequence: seq,
                host_uptime_s: u32::from_le_bytes([raw[10], raw[11], raw[12], raw[13]]),
                ram_used_percent: raw[14],
                console_health_ok: raw[15] == 1,
            }))
        }
        _ => Err(Error::Kind),
    }
}

/// Streaming decoder handles fragmented packets, corruption and mixed
/// PULSE/1 + PULSE/IMU-1 frames. The decoder is allocation-free.
pub struct Decoder {
    buffer: [u8; FRAME_LEN],
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
            buffer: [0; FRAME_LEN],
            len: 0,
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
    }

    pub fn push(&mut self, byte: u8) -> Option<Result<Frame, Error>> {
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
            let mut offset = None;
            for index in 1..(FRAME_LEN - 1) {
                if self.buffer[index..index + 2] == MAGIC {
                    offset = Some(index);
                    break;
                }
            }
            if let Some(index) = offset {
                self.buffer.copy_within(index..FRAME_LEN, 0);
                self.len = FRAME_LEN - index;
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

    fn sample() -> Snapshot {
        Snapshot {
            sequence: 42,
            host_uptime_s: 432_100,
            ram_used_percent: 38,
            console_health_ok: true,
        }
    }

    #[test]
    fn probe_and_snapshot_roundtrip() {
        for packet in [Frame::Probe, Frame::ProbeAck, Frame::Snapshot(sample())] {
            let raw = encode(packet);
            assert_eq!(raw.len(), 18);
            assert_eq!(&raw[..2], b"SY");
            assert_eq!(decode(&raw), Ok(packet));
        }
    }

    #[test]
    fn crc_rejects_corruption_and_stream_recovers() {
        let mut bad = encode(Frame::Snapshot(sample()));
        bad[14] ^= 0x7f;
        assert_eq!(decode(&bad), Err(Error::Checksum));
        let mut dec = Decoder::new();
        let mut errors = 0;
        let mut good = 0;
        for b in bad.into_iter().chain(encode(Frame::Snapshot(sample()))) {
            if let Some(result) = dec.push(b) {
                if result.is_err() {
                    errors += 1;
                }
                if result == Ok(Frame::Snapshot(sample())) {
                    good += 1;
                }
            }
        }
        assert_eq!((errors, good), (1, 1));
    }

    #[test]
    fn fail_closed_on_bad_provenance_and_values() {
        let mut raw = encode(Frame::Snapshot(sample()));
        raw[4] = 0;
        let checksum = crc16_ccitt_false(&raw[..16]);
        raw[16..18].copy_from_slice(&checksum.to_le_bytes());
        assert_eq!(decode(&raw), Err(Error::Flags));

        raw = encode(Frame::Snapshot(sample()));
        raw[14] = 101;
        let checksum = crc16_ccitt_false(&raw[..16]);
        raw[16..18].copy_from_slice(&checksum.to_le_bytes());
        assert_eq!(decode(&raw), Err(Error::InvalidSample));

        raw = encode(Frame::Snapshot(sample()));
        raw[15] = 2;
        let checksum = crc16_ccitt_false(&raw[..16]);
        raw[16..18].copy_from_slice(&checksum.to_le_bytes());
        assert_eq!(decode(&raw), Err(Error::InvalidSample));
    }

    #[test]
    fn older_protocol_frames_are_ignored() {
        let mut sys = Decoder::new();
        let old = crate::encode(crate::Frame {
            kind: crate::Kind::DemoTicks,
            sequence: 1,
            uptime_ms: 10,
            value: 20,
        });
        let imu = crate::imu_wire::encode(crate::imu_wire::Frame::Probe);
        for b in old.into_iter().chain(imu) {
            assert!(sys.push(b).is_none());
        }
        assert_eq!(encode(Frame::Probe)[..2], [b'S', b'Y']);
    }

    #[test]
    fn streaming_fragmented_frame_and_noise() {
        let mut d = Decoder::new();
        for b in [0, b'R', b'I', b'S', 0xff] {
            assert!(d.push(b).is_none());
        }
        let mut got = None;
        for b in encode(Frame::Snapshot(sample())) {
            if let Some(v) = d.push(b) {
                got = Some(v);
            }
        }
        assert_eq!(got, Some(Ok(Frame::Snapshot(sample()))));
    }
}
