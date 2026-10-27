//! Stable, bounded preferences shared by the embedded store and host tests.
//! Only display choices are persisted: skin and last K1..K4 page.
//! Focus, selected metrics, joystick diagnostics and sensor data are transient.
//! Color Lab is a hardware calibration fixture: validated NORM/BGR always boots.
//!
//! On-flash v1 record occupies ONE new 256-byte program page. Records use
//! magic/version/bounds/CRC; invalid or torn records are never restored.

use crate::{skins::SkinId, Page};

pub const RECORD_LEN: usize = 256;
pub const RECORD_MAGIC: [u8; 4] = *b"RGDP";
pub const RECORD_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub skin: SkinId,
    pub page: Page,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            skin: SkinId::Classic,
            page: Page::Somatic,
        }
    }
}

impl Preferences {
    pub const fn new(skin: SkinId, page: Page) -> Self {
        Self { skin, page }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub sequence: u32,
    pub preferences: Preferences,
}

impl Record {
    pub const fn new(sequence: u32, preferences: Preferences) -> Self {
        Self {
            sequence,
            preferences,
        }
    }

    pub fn encode(&self) -> [u8; RECORD_LEN] {
        let mut bytes = [0xff; RECORD_LEN];
        bytes[0..4].copy_from_slice(&RECORD_MAGIC);
        bytes[4] = RECORD_VERSION;
        bytes[5] = self.preferences.skin.index();
        bytes[6] = self.preferences.page as u8;
        bytes[7] = 0; // reserved: must be zero in v1
        bytes[8..12].copy_from_slice(&self.sequence.to_le_bytes());
        let checksum = crc16(&bytes[..12]);
        bytes[12..14].copy_from_slice(&checksum.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8; RECORD_LEN]) -> Option<Self> {
        if bytes[..4] != RECORD_MAGIC
            || bytes[4] != RECORD_VERSION
            || bytes[5] > 3
            || bytes[6] > 3
            || bytes[7] != 0
        {
            return None;
        }
        let expected = crc16(&bytes[..12]);
        let found = u16::from_le_bytes([bytes[12], bytes[13]]);
        if expected != found {
            return None;
        }
        // An erased/torn record may contain arbitrary padded bytes. Never
        // require untouched padding after the CRC; ignore future extensions.
        let page = match bytes[6] {
            0 => Page::Somatic,
            1 => Page::Chronons,
            2 => Page::Imu,
            3 => Page::System,
            _ => return None,
        };
        Some(Self {
            sequence: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            preferences: Preferences {
                skin: SkinId::from_index(bytes[5]),
                page,
            },
        })
    }
}

/// Sequence ordering that tolerates u32 rollover; avoid treating a stale
/// record as newest after sequence 0xffff_ffff -> 0.
pub const fn is_newer(candidate: u32, existing: u32) -> bool {
    let diff = candidate.wrapping_sub(existing);
    diff != 0 && diff < 0x8000_0000
}

pub fn is_erased(bytes: &[u8; RECORD_LEN]) -> bool {
    bytes.iter().all(|&byte| byte == 0xff)
}

/// CRC16-CCITT-FALSE over the twelve-byte record header.
fn crc16(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for byte in bytes {
        crc ^= (*byte as u16) << 8;
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

    #[test]
    fn all_skins_and_pages_roundtrip_without_sensor_fields() {
        for skin in 0..4 {
            for page in [Page::Somatic, Page::Chronons, Page::Imu, Page::System] {
                let prefs = Preferences::new(SkinId::from_index(skin), page);
                let original = Record::new(17, prefs);
                let bytes = original.encode();
                assert_eq!(Record::decode(&bytes), Some(original));
                assert_eq!(&bytes[14..], &[0xff; RECORD_LEN - 14]);
            }
        }
    }

    #[test]
    fn torn_unknown_and_invalid_records_fail_closed() {
        let mut record = Record::new(22, Preferences::new(SkinId::Italia, Page::System)).encode();
        assert!(Record::decode(&[0xff; RECORD_LEN]).is_none());
        record[9] ^= 0x40; // torn write / bit-flip
        assert!(Record::decode(&record).is_none());
        let mut record = Record::new(22, Preferences::default()).encode();
        record[4] = 9;
        assert!(Record::decode(&record).is_none());
        let mut record = Record::new(22, Preferences::default()).encode();
        record[5] = 4;
        assert!(Record::decode(&record).is_none());
        let mut record = Record::new(22, Preferences::default()).encode();
        record[6] = 4;
        assert!(Record::decode(&record).is_none());
        let mut record = Record::new(22, Preferences::default()).encode();
        record[7] = 1;
        assert!(Record::decode(&record).is_none());
    }

    #[test]
    fn wrap_safe_and_duplicate_safe_order() {
        assert!(is_newer(8, 7));
        assert!(!is_newer(7, 7));
        assert!(!is_newer(6, 7));
        assert!(is_newer(0, u32::MAX));
        assert!(!is_newer(u32::MAX, 0));
    }

    #[test]
    fn defaults_are_stable_and_color_lab_is_not_a_preference() {
        let p = Preferences::default();
        assert_eq!(p.skin, SkinId::Classic);
        assert_eq!(p.page, Page::Somatic);
        // Color Lab is deliberately not included in this schema.
        assert_eq!(
            Record::decode(&Record::new(1, p).encode())
                .unwrap()
                .preferences,
            p
        );
    }
}
