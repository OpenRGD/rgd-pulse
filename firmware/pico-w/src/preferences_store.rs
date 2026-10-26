//! Platform adapter: Embassy RP2040 blocking flash operations, with
//! persistent skin and last K-page recovery. No MCU flash preferences are
//! touched outside the dedicated last 8KiB of a Pico W 2MiB QSPI chip.
//!
//! HARDWARE SAFETY:
//! - This is firmware flash, never robot motor, CAN, IMU or safety storage.
//! - The Embassy 0.3.1 HAL runs erase/program from RAM under critical section,
//!   coordinates CORE1 and waits for XIP DMA access to finish.
//! - Calls must occur on CORE0, only from the main Embassy task and NEVER
//!   while USB CDC/DTR is active. Flash programming may stall USB timing.
//! - At most once after 3s idle and 15s since last committed save.
//! - On any flash error writes are disabled for this boot; old valid record
//!   remains available, or defaults are used after a read failure.
//! - No preference flash writes at initial boot unless UI choices change.
//! - Color Lab is hardware calibration: NORM/BGR remains a compiled default.

use embassy_rp::{
    flash::{Blocking, Flash},
    peripherals::FLASH,
};
use pulse_ui::{
    preferences::{Preferences, RECORD_LEN},
    preferences_journal::{
        FlashStorage, Journal, FLASH_BYTES, SAVE_IDLE_MS,
        SAVE_MIN_INTERVAL_MS, SECTOR_BYTES,
    },
};

type PicoFlash<'d> = Flash<'d, FLASH, Blocking, { FLASH_BYTES as usize }>;

pub struct PreferencesStore<'d> {
    flash: PicoFlash<'d>,
    journal: Option<Journal>,
    pending: Option<Preferences>,
    last_changed_ms: u64,
    last_written_ms: Option<u64>,
    writes_disabled: bool,
}

impl FlashStorage for PreferencesStore<'_> {
    type Error = embassy_rp::flash::Error;

    fn read_page(&mut self, offset: u32, out: &mut [u8; RECORD_LEN]) -> Result<(), Self::Error> {
        self.flash.blocking_read(offset, out)
    }

    fn erase_sector(&mut self, offset: u32) -> Result<(), Self::Error> {
        self.flash.blocking_erase(offset, offset + SECTOR_BYTES)
    }

    fn write_page(&mut self, offset: u32, record: &[u8; RECORD_LEN]) -> Result<(), Self::Error> {
        self.flash.blocking_write(offset, record)
    }
}

impl<'d> PreferencesStore<'d> {
    pub fn new(flash: FLASH) -> Self {
        let mut store = Self {
            flash: Flash::new_blocking(flash),
            journal: None,
            pending: None,
            last_changed_ms: 0,
            last_written_ms: None,
            writes_disabled: false,
        };

        match Journal::load(&mut store) {
            Ok(journal) => store.journal = Some(journal),
            Err(_) => store.writes_disabled = true,
        }

        store
    }

    pub fn restored(&self) -> Preferences {
        self.journal.map(|j| j.preferences()).unwrap_or_default()
    }

    /// Record only actual applied choices, never preview skins or transient
    /// controls. Identical already-saved choices cancel pending writes.
    pub fn observe(&mut self, choices: Preferences, now_ms: u64) {
        if self.writes_disabled {
            return;
        }
        if self.restored() == choices {
            self.pending = None;
            return;
        }
        if self.pending != Some(choices) {
            self.pending = Some(choices);
            self.last_changed_ms = now_ms;
        }
    }

    /// Called in the main task after handling controls and CDC. We decline
    /// to write while DTR is active; do not interrupt HELLO/DEMO/ACK or
    /// physical IMU stream to persist cosmetic settings.
    pub fn poll(&mut self, now_ms: u64, usb_cdc_active: bool) {
        if self.writes_disabled || usb_cdc_active {
            return;
        }
        let Some(choices) = self.pending else {
            return;
        };
        if now_ms.saturating_sub(self.last_changed_ms) < SAVE_IDLE_MS {
            return;
        }
        if self
            .last_written_ms
            .map(|previous| now_ms.saturating_sub(previous) < SAVE_MIN_INTERVAL_MS)
            .unwrap_or(false)
        {
            return;
        }

        let Some(mut journal) = self.journal.take() else {
            self.writes_disabled = true;
            return;
        };
        let result = journal.save(self, choices);
        match result {
            Ok(_) => {
                self.journal = Some(journal);
                self.pending = None;
                self.last_written_ms = Some(now_ms);
            }
            Err(_) => {
                // Do not attempt a second write in the same session after
                // possible torn programming. Next boot rescans both sectors.
                self.writes_disabled = true;
                self.pending = None;
                self.journal = Some(journal);
            }
        }
    }
}
