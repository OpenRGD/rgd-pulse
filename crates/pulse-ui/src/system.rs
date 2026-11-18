//! Non-actuating K4 SYSTEM observation view.
//! The only accepted remote fields in v0.4A are Linux host uptime,
//! Linux MemTotal/MemAvailable utilization and Console /healthz response.
//! Nothing here proves robot readiness, CAN bus status, motor state,
//! power voltage, or an authenticated operator identity.

pub const HOST_STALE_AFTER_MS: u64 = 2_500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostSnapshot {
    pub sequence: u32,
    pub uptime_s: u32,
    pub ram_used_percent: u8,
    /// A valid response from local Console /healthz, not robot readiness.
    pub console_health_ok: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostStatus {
    NoSource,
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidSnapshot,
    Replay,
}

/// No heap and no persistent storage: host observations are always ephemeral.
pub struct SystemView {
    last: Option<HostSnapshot>,
    received_ms: u64,
}

impl Default for SystemView {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemView {
    pub const fn new() -> Self {
        Self {
            last: None,
            received_ms: 0,
        }
    }

    pub fn clear(&mut self) {
        self.last = None;
        self.received_ms = 0;
    }

    pub fn observe(&mut self, snapshot: HostSnapshot, now_ms: u64) -> Result<(), Error> {
        if snapshot.sequence == 0 || snapshot.ram_used_percent > 100 {
            return Err(Error::InvalidSnapshot);
        }
        if let Some(previous) = self.last {
            let diff = snapshot.sequence.wrapping_sub(previous.sequence);
            if diff == 0 || diff >= 0x8000_0000 {
                return Err(Error::Replay);
            }
        }
        self.last = Some(snapshot);
        self.received_ms = now_ms;
        Ok(())
    }

    pub const fn latest(&self) -> Option<&HostSnapshot> {
        self.last.as_ref()
    }

    pub fn status(&self, now_ms: u64) -> HostStatus {
        if self.last.is_none() {
            HostStatus::NoSource
        } else if now_ms.saturating_sub(self.received_ms) <= HOST_STALE_AFTER_MS {
            HostStatus::Fresh
        } else {
            HostStatus::Stale
        }
    }

    pub fn age_ms(&self, now_ms: u64) -> Option<u64> {
        self.last?;
        Some(now_ms.saturating_sub(self.received_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(seq: u32) -> HostSnapshot {
        HostSnapshot {
            sequence: seq,
            uptime_s: 200,
            ram_used_percent: 32,
            console_health_ok: true,
        }
    }

    #[test]
    fn no_source_does_not_claim_host_is_healthy() {
        let s = SystemView::new();
        assert_eq!(s.status(20_000), HostStatus::NoSource);
        assert_eq!(s.latest(), None);
        assert_eq!(s.age_ms(20_000), None);
    }

    #[test]
    fn fresh_becomes_stale_and_preserves_last_reading() {
        let mut s = SystemView::new();
        s.observe(sample(7), 1_000).unwrap();
        assert_eq!(s.status(3_500), HostStatus::Fresh);
        assert_eq!(s.status(3_501), HostStatus::Stale);
        assert_eq!(s.latest(), Some(&sample(7)));
        s.clear();
        assert_eq!(s.status(3_502), HostStatus::NoSource);
    }

    #[test]
    fn replay_and_invalid_values_are_rejected_without_mutation() {
        let mut s = SystemView::new();
        s.observe(sample(10), 100).unwrap();
        assert_eq!(s.observe(sample(10), 101), Err(Error::Replay));
        assert_eq!(s.observe(sample(9), 102), Err(Error::Replay));
        let mut wrong = sample(11);
        wrong.ram_used_percent = 101;
        assert_eq!(s.observe(wrong, 103), Err(Error::InvalidSnapshot));
        assert_eq!(s.latest(), Some(&sample(10)));
        assert_eq!(s.age_ms(105), Some(5));
    }

    #[test]
    fn sequence_wrap_is_accepted_only_forward() {
        let mut s = SystemView::new();
        s.observe(sample(u32::MAX), 100).unwrap();
        assert_eq!(s.observe(sample(1), 200), Ok(()));
        assert_eq!(s.observe(sample(u32::MAX), 201), Err(Error::Replay));
    }
}
