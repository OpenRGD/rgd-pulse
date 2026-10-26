//! Two-sector, 256-byte-page, CRC-backed RP2040 UI preferences journal.
//!
//! This module is no_std and hardware-independent so power-failure cases
//! can be tested on the host. The firmware adapter is the only flash writer.
//! All offsets are from the start of the Pico W's verified 2MiB QSPI flash.
//! Region: [0x001fe000, 0x00200000), *outside* the linked firmware image.
//!
//! Each 4KiB sector holds 16 one-shot records. Appends never overwrite a
//! programmed page. When full, erase ONLY the *other* sector while keeping
//! the current valid record in place. Commit new data by writing one page.
//! Power loss during erase/write leaves the previous sector recoverable.
//! CRC errors are discarded rather than mistaken for a new preference.
//!
//! No IMU/CAN/motion data, flash commands, or runtime loop frequencies
//! are persisted.

use crate::preferences::{is_erased, is_newer, Preferences, Record, RECORD_LEN};

pub const FLASH_BYTES: u32 = 2 * 1024 * 1024;
pub const SECTOR_BYTES: u32 = 4096;
pub const JOURNAL_BYTES: u32 = 2 * SECTOR_BYTES;
pub const BASE_OFFSET: u32 = FLASH_BYTES - JOURNAL_BYTES;
pub const PAGES_PER_SECTOR: u8 = (SECTOR_BYTES as usize / RECORD_LEN) as u8;
pub const SAVE_IDLE_MS: u64 = 3_000;
pub const SAVE_MIN_INTERVAL_MS: u64 = 15_000;

/// Platform adapter. The RP2040 implementation uses embassy-rp's
/// blocking flash HAL, which pauses interrupts and cross-core XIP during
/// programming. No raw ROM calls are made by this journal.
pub trait FlashStorage {
    type Error;
    fn read_page(&mut self, offset: u32, bytes: &mut [u8; RECORD_LEN]) -> Result<(), Self::Error>;
    fn erase_sector(&mut self, offset: u32) -> Result<(), Self::Error>;
    fn write_page(&mut self, offset: u32, bytes: &[u8; RECORD_LEN]) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum JournalError<E> {
    Storage(E),
    Verify,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Location {
    sector: u8,
    slot: u8,
}

impl Location {
    const fn offset(self) -> u32 {
        BASE_OFFSET + self.sector as u32 * SECTOR_BYTES + self.slot as u32 * RECORD_LEN as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Journal {
    current: Option<(Record, Location)>,
}

impl Journal {
    pub fn load<F: FlashStorage>(storage: &mut F) -> Result<Self, JournalError<F::Error>> {
        let mut current: Option<(Record, Location)> = None;
        let mut page = [0xff; RECORD_LEN];
        for sector in 0..2u8 {
            for slot in 0..PAGES_PER_SECTOR {
                let location = Location { sector, slot };
                storage
                    .read_page(location.offset(), &mut page)
                    .map_err(JournalError::Storage)?;
                if let Some(record) = Record::decode(&page) {
                    if current
                        .map(|(existing, _)| is_newer(record.sequence, existing.sequence))
                        .unwrap_or(true)
                    {
                        current = Some((record, location));
                    }
                }
            }
        }
        Ok(Self { current })
    }

    pub fn preferences(&self) -> Preferences {
        self.current
            .map(|(record, _)| record.preferences)
            .unwrap_or_default()
    }

    pub fn has_saved_record(&self) -> bool {
        self.current.is_some()
    }

    fn first_erased_from<F: FlashStorage>(
        storage: &mut F,
        sector: u8,
        start: u8,
    ) -> Result<Option<Location>, JournalError<F::Error>> {
        let mut page = [0; RECORD_LEN];
        for slot in start..PAGES_PER_SECTOR {
            let location = Location { sector, slot };
            storage
                .read_page(location.offset(), &mut page)
                .map_err(JournalError::Storage)?;
            if is_erased(&page) {
                return Ok(Some(location));
            }
        }
        Ok(None)
    }

    /// Returns true only if a new record has been verified. Repeated
    /// equivalent choices are not written again (flash wear protection).
    ///
    /// On ANY error, reload the journal before attempting another write;
    /// the failed page may be partially programmed. The last good record
    /// remains available in the older location.
    pub fn save<F: FlashStorage>(
        &mut self,
        storage: &mut F,
        choices: Preferences,
    ) -> Result<bool, JournalError<F::Error>> {
        if self.current.map(|(r, _)| r.preferences) == Some(choices) {
            return Ok(false);
        }

        let target = if let Some((_, active)) = self.current {
            if let Some(blank) = Self::first_erased_from(storage, active.sector, active.slot + 1)? {
                blank
            } else {
                let next_sector = 1u8 - active.sector;
                // Keep the only confirmed record in the active sector.
                // Erase *only* the inactive sector, even if it is already
                // empty: this also discards any interrupted former writes.
                storage
                    .erase_sector(BASE_OFFSET + next_sector as u32 * SECTOR_BYTES)
                    .map_err(JournalError::Storage)?;
                Location {
                    sector: next_sector,
                    slot: 0,
                }
            }
        } else {
            if let Some(blank) = Self::first_erased_from(storage, 0, 0)? {
                blank
            } else if let Some(blank) = Self::first_erased_from(storage, 1, 0)? {
                blank
            } else {
                storage
                    .erase_sector(BASE_OFFSET)
                    .map_err(JournalError::Storage)?;
                Location { sector: 0, slot: 0 }
            }
        };

        let seq = self
            .current
            .map(|(r, _)| r.sequence.wrapping_add(1))
            .unwrap_or(1);
        let desired = Record::new(seq, choices);
        storage
            .write_page(target.offset(), &desired.encode())
            .map_err(JournalError::Storage)?;

        let mut check = [0; RECORD_LEN];
        storage
            .read_page(target.offset(), &mut check)
            .map_err(JournalError::Storage)?;
        if Record::decode(&check) != Some(desired) {
            return Err(JournalError::Verify);
        }
        self.current = Some((desired, target));
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{skins::SkinId, Page};

    struct MemoryFlash {
        bytes: [u8; JOURNAL_BYTES as usize],
        write_cutoff: Option<usize>,
        erase_cutoff: Option<usize>,
        erase_count: u32,
        write_count: u32,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum SimError {
        PowerCut,
        Bounds,
    }

    impl Default for MemoryFlash {
        fn default() -> Self {
            Self {
                bytes: [0xff; JOURNAL_BYTES as usize],
                write_cutoff: None,
                erase_cutoff: None,
                erase_count: 0,
                write_count: 0,
            }
        }
    }

    impl MemoryFlash {
        fn at(offset: u32, len: usize) -> Result<usize, SimError> {
            let idx = offset.checked_sub(BASE_OFFSET).ok_or(SimError::Bounds)? as usize;
            if idx.checked_add(len).map(|end| end <= JOURNAL_BYTES as usize) != Some(true) {
                return Err(SimError::Bounds);
            }
            Ok(idx)
        }
    }

    impl FlashStorage for MemoryFlash {
        type Error = SimError;

        fn read_page(&mut self, offset: u32, out: &mut [u8; RECORD_LEN]) -> Result<(), Self::Error> {
            if (offset.checked_sub(BASE_OFFSET).ok_or(SimError::Bounds)?) % RECORD_LEN as u32 != 0 { return Err(SimError::Bounds); }
            let i = Self::at(offset, RECORD_LEN)?;
            out.copy_from_slice(&self.bytes[i..i + RECORD_LEN]);
            Ok(())
        }

        fn erase_sector(&mut self, offset: u32) -> Result<(), Self::Error> {
            if (offset.checked_sub(BASE_OFFSET).ok_or(SimError::Bounds)?) % SECTOR_BYTES != 0 { return Err(SimError::Bounds); }
            self.erase_count += 1;
            let i = Self::at(offset, SECTOR_BYTES as usize)?;
            let until = self.erase_cutoff.take().unwrap_or(SECTOR_BYTES as usize);
            self.bytes[i..i + until].fill(0xff);
            if until != SECTOR_BYTES as usize { return Err(SimError::PowerCut); }
            Ok(())
        }

        fn write_page(&mut self, offset: u32, page: &[u8; RECORD_LEN]) -> Result<(), Self::Error> {
            if (offset.checked_sub(BASE_OFFSET).ok_or(SimError::Bounds)?) % RECORD_LEN as u32 != 0 { return Err(SimError::Bounds); }
            self.write_count += 1;
            let i = Self::at(offset, RECORD_LEN)?;
            let until = self.write_cutoff.take().unwrap_or(RECORD_LEN);
            for (slot, value) in self.bytes[i..i + until].iter_mut().zip(page) {
                *slot &= *value; // NOR programming can turn 1->0 only
            }
            if until != RECORD_LEN { return Err(SimError::PowerCut); }
            Ok(())
        }
    }

    fn choices(skin: SkinId, page: Page) -> Preferences {
        Preferences::new(skin, page)
    }

    #[test]
    fn cold_boot_defaults_save_and_reboot_restore() {
        let mut flash = MemoryFlash::default();
        let mut journal = Journal::load(&mut flash).unwrap();
        assert!(!journal.has_saved_record());
        assert_eq!(journal.preferences(), Preferences::default());
        let p = choices(SkinId::Italia, Page::Imu);
        assert!(journal.save(&mut flash, p).unwrap());
        assert!(Journal::load(&mut flash).unwrap().has_saved_record());
        assert_eq!(Journal::load(&mut flash).unwrap().preferences(), p);
        assert!(!journal.save(&mut flash, p).unwrap());
        assert_eq!(flash.write_count, 1);
    }

    #[test]
    fn journal_rolls_through_both_sectors_without_destroying_last_commit() {
        let mut flash = MemoryFlash::default();
        let mut journal = Journal::load(&mut flash).unwrap();
        for i in 0..80 {
            let p = choices(
                SkinId::from_index((i % 4) as u8),
                match (i / 4) % 4 {
                    0 => Page::Somatic,
                    1 => Page::Chronons,
                    2 => Page::Imu,
                    _ => Page::System,
                },
            );
            journal.save(&mut flash, p).unwrap();
            journal = Journal::load(&mut flash).unwrap();
            assert_eq!(journal.preferences(), p);
        }
        assert_eq!(flash.write_count, 80);
        assert!(flash.erase_count >= 3);
    }

    #[test]
    fn torn_program_skips_corrupt_page_without_erasing_good_page() {
        let mut flash = MemoryFlash::default();
        let mut journal = Journal::load(&mut flash).unwrap();
        let old = choices(SkinId::Phosphor, Page::Chronons);
        journal.save(&mut flash, old).unwrap();
        flash.write_cutoff = Some(9);
        assert_eq!(
            journal.save(&mut flash, choices(SkinId::Italia, Page::System)),
            Err(JournalError::Storage(SimError::PowerCut)),
        );
        journal = Journal::load(&mut flash).unwrap();
        assert_eq!(journal.preferences(), old);
        let new = choices(SkinId::Italia, Page::System);
        journal.save(&mut flash, new).unwrap();
        assert_eq!(Journal::load(&mut flash).unwrap().preferences(), new);
    }

    #[test]
    fn power_loss_during_cross_sector_erase_preserves_old() {
        let mut flash = MemoryFlash::default();
        let mut journal = Journal::load(&mut flash).unwrap();
        for i in 0..32 {
            let new = choices(SkinId::from_index((i % 4) as u8), Page::Imu);
            journal.save(&mut flash, new).unwrap();
        }
        let old = journal.preferences();
        flash.erase_cutoff = Some(2048);
        assert_eq!(
            journal.save(&mut flash, choices(SkinId::Italia, Page::System)),
            Err(JournalError::Storage(SimError::PowerCut)),
        );
        assert_eq!(Journal::load(&mut flash).unwrap().preferences(), old);
    }

    #[test]
    fn rejects_out_of_partition_offsets() {
        let mut flash = MemoryFlash::default();
        assert_eq!(
            flash.read_page(BASE_OFFSET - RECORD_LEN as u32, &mut [0; RECORD_LEN]),
            Err(SimError::Bounds)
        );
    }
}
