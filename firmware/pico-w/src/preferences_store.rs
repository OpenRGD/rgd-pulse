//! RP2040 persistent UI preferences with read-only on-device diagnostics.
//! Never writes CAN, motors, calibration, IMU or any robot safety parameter.
//! A 2MiB flash JEDEC capacity code is REQUIRED before journal access.
//! All flash operations are performed on CORE0 via Embassy RP 0.3.1 HAL,
//! not from an interrupt, and save is suppressed while CDC DTR is asserted.
//! Keep power for at least 20s after a preference change for first bench.
//!
//! Any flash failure is latched and visible in the local diagnostic overlay.
//! Failures disable subsequent writes for this boot: never silently retry
//! writes into a possibly torn NOR page.

use embassy_rp::{
    flash::{Blocking, Error as FlashError, Flash},
    peripherals::FLASH,
};
use pulse_ui::{
    preferences::{Preferences, RECORD_LEN},
    preferences_journal::{
        FlashStorage, Journal, JournalError, FLASH_BYTES, SAVE_IDLE_MS, SAVE_MIN_INTERVAL_MS,
        SECTOR_BYTES,
    },
};

type PicoFlash<'d> = Flash<'d, FLASH, Blocking, { FLASH_BYTES as usize }>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistState {
    Empty,
    Restored,
    Pending,
    WaitIdle,
    WaitUsb,
    WaitRate,
    Saved,
    CapacityMismatch,
    JedecError,
    ReadError,
    WriteError,
}

impl PersistState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Empty => "EMPTY",
            Self::Restored => "RESTORED",
            Self::Pending => "PENDING",
            Self::WaitIdle => "WAIT IDLE",
            Self::WaitUsb => "WAIT USB DTR",
            Self::WaitRate => "WAIT RATE",
            Self::Saved => "SAVED",
            Self::CapacityMismatch => "BAD FLASH ID",
            Self::JedecError => "JEDEC ERROR",
            Self::ReadError => "READ ERROR",
            Self::WriteError => "WRITE ERROR",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub state: PersistState,
    pub jedec_id: u32,
    pub sequence: u32,
    pub saved: Preferences,
    pub pending: Option<Preferences>,
    pub error_code: u8,
}

pub struct PreferencesStore<'d> {
    flash: PicoFlash<'d>,
    journal: Option<Journal>,
    pending: Option<Preferences>,
    last_changed_ms: u64,
    last_written_ms: Option<u64>,
    writes_disabled: bool,
    state: PersistState,
    jedec_id: u32,
    error_code: u8,
}

impl FlashStorage for PreferencesStore<'_> {
    type Error = FlashError;

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

fn error_code(error: &JournalError<FlashError>) -> u8 {
    match error {
        JournalError::Verify => 5,
        JournalError::Storage(FlashError::OutOfBounds) => 1,
        JournalError::Storage(FlashError::Unaligned) => 2,
        JournalError::Storage(FlashError::InvalidCore) => 3,
        JournalError::Storage(FlashError::Other) => 4,
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
            state: PersistState::Empty,
            jedec_id: 0,
            error_code: 0,
        };

        // JEDEC 9F returns manufacturer, memory type and capacity bytes;
        // standard 2MiB Pico W devices report capacity 0x15 = 2^21 bytes.
        // Do not attempt to read/write fixed end-of-chip offsets if a board
        // reports another size. Unknown devices fail CLOSED.
        match store.flash.blocking_jedec_id() {
            Ok(id) => {
                store.jedec_id = id;
                if id & 0xff != 0x15 {
                    store.state = PersistState::CapacityMismatch;
                    store.writes_disabled = true;
                    return store;
                }
            }
            Err(_) => {
                store.state = PersistState::JedecError;
                store.writes_disabled = true;
                store.error_code = 6;
                return store;
            }
        }

        match Journal::load(&mut store) {
            Ok(journal) => {
                store.state = if journal.has_saved_record() {
                    PersistState::Restored
                } else {
                    PersistState::Empty
                };
                store.journal = Some(journal);
            }
            Err(error) => {
                store.state = PersistState::ReadError;
                store.error_code = error_code(&error);
                store.writes_disabled = true;
            }
        }
        store
    }

    pub fn restored(&self) -> Preferences {
        self.journal.map(|j| j.preferences()).unwrap_or_default()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: self.state,
            jedec_id: self.jedec_id,
            sequence: self.journal.map(|j| j.sequence()).unwrap_or(0),
            saved: self.restored(),
            pending: self.pending,
            error_code: self.error_code,
        }
    }

    pub fn observe(&mut self, choices: Preferences, now_ms: u64) {
        if self.writes_disabled {
            return;
        }
        if self.restored() == choices {
            self.pending = None;
            self.state = if self.last_written_ms.is_some() {
                PersistState::Saved
            } else if self.journal.map(|j| j.has_saved_record()).unwrap_or(false) {
                PersistState::Restored
            } else {
                PersistState::Empty
            };
            return;
        }
        if self.pending != Some(choices) {
            self.pending = Some(choices);
            self.last_changed_ms = now_ms;
            self.state = PersistState::Pending;
        }
    }

    pub fn poll(&mut self, now_ms: u64, usb_cdc_active: bool) {
        if self.writes_disabled {
            return;
        }
        let Some(choices) = self.pending else {
            return;
        };
        if usb_cdc_active {
            self.state = PersistState::WaitUsb;
            return;
        }
        if now_ms.saturating_sub(self.last_changed_ms) < SAVE_IDLE_MS {
            self.state = PersistState::WaitIdle;
            return;
        }
        if self
            .last_written_ms
            .map(|previous| now_ms.saturating_sub(previous) < SAVE_MIN_INTERVAL_MS)
            .unwrap_or(false)
        {
            self.state = PersistState::WaitRate;
            return;
        }

        let Some(mut journal) = self.journal.take() else {
            self.writes_disabled = true;
            self.state = PersistState::ReadError;
            self.error_code = 7;
            return;
        };
        let result = journal.save(self, choices);
        self.journal = Some(journal);
        match result {
            Ok(_) => {
                self.pending = None;
                self.last_written_ms = Some(now_ms);
                self.state = PersistState::Saved;
                self.error_code = 0;
            }
            Err(error) => {
                // Never retry an interrupted write this power-on session.
                self.error_code = error_code(&error);
                self.writes_disabled = true;
                self.pending = None;
                self.state = PersistState::WriteError;
            }
        }
    }
}
