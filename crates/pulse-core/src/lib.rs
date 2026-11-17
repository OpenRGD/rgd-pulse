//! Read-only observer. A fresh frame is not proof of safe movement.

use pulse_protocol::{Frame, Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    Waiting,
    Live,
    Stale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopState {
    Unobserved,
    Advancing,
    Stalled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserveError {
    DuplicateOrOldSequence,
}

pub struct Observer {
    timeout_ms: u64,
    last_rx_ms: Option<u64>,
    last_sequence: Option<u32>,
    last_loop_ticks: Option<u32>,
    last_loop_advance_ms: Option<u64>,
    missed: u64,
}

impl Observer {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            last_rx_ms: None,
            last_sequence: None,
            last_loop_ticks: None,
            last_loop_advance_ms: None,
            missed: 0,
        }
    }

    pub fn observe(&mut self, frame: Frame, received_at_ms: u64) -> Result<(), ObserveError> {
        if let Some(previous) = self.last_sequence {
            let diff = frame.sequence.wrapping_sub(previous);
            if diff == 0 || diff >= 0x8000_0000 {
                return Err(ObserveError::DuplicateOrOldSequence);
            }
            self.missed = self.missed.saturating_add((diff - 1) as u64);
        }
        self.last_rx_ms = Some(received_at_ms);
        self.last_sequence = Some(frame.sequence);
        if frame.kind == Kind::LoopTicks {
            if self.last_loop_ticks != Some(frame.value) {
                self.last_loop_ticks = Some(frame.value);
                self.last_loop_advance_ms = Some(received_at_ms);
            }
        }
        Ok(())
    }

    pub fn link_state(&self, now_ms: u64) -> LinkState {
        match self.last_rx_ms {
            None => LinkState::Waiting,
            Some(last) if now_ms.saturating_sub(last) > self.timeout_ms => LinkState::Stale,
            _ => LinkState::Live,
        }
    }

    pub fn loop_state(&self, now_ms: u64) -> LoopState {
        if self.last_loop_ticks.is_none() || self.link_state(now_ms) != LinkState::Live {
            return LoopState::Unobserved;
        }
        match self.last_loop_advance_ms {
            Some(last) if now_ms.saturating_sub(last) > self.timeout_ms => LoopState::Stalled,
            Some(_) => LoopState::Advancing,
            None => LoopState::Unobserved,
        }
    }

    pub fn missed_frames(&self) -> u64 {
        self.missed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tick(sequence: u32, ticks: u32) -> Frame {
        Frame {
            kind: Kind::LoopTicks,
            sequence,
            uptime_ms: sequence.wrapping_mul(100),
            value: ticks,
        }
    }

    #[test]
    fn starts_unknown_then_times_out() {
        let mut observer = Observer::new(300);
        assert_eq!(observer.link_state(0), LinkState::Waiting);
        assert_eq!(observer.loop_state(0), LoopState::Unobserved);
        observer.observe(tick(1, 1200), 0).unwrap();
        assert_eq!(observer.link_state(300), LinkState::Live);
        assert_eq!(observer.link_state(301), LinkState::Stale);
        assert_eq!(observer.loop_state(301), LoopState::Unobserved);
    }

    #[test]
    fn does_not_confuse_living_bridge_with_stalled_loop() {
        let mut observer = Observer::new(300);
        observer.observe(tick(1, 1200), 0).unwrap();
        observer.observe(tick(2, 1200), 200).unwrap();
        observer.observe(tick(3, 1200), 400).unwrap();
        assert_eq!(observer.link_state(400), LinkState::Live);
        assert_eq!(observer.loop_state(400), LoopState::Stalled);
        observer.observe(tick(4, 1210), 500).unwrap();
        assert_eq!(observer.loop_state(500), LoopState::Advancing);
    }

    #[test]
    fn detects_lost_frames_and_replay() {
        let mut observer = Observer::new(1000);
        observer.observe(tick(2, 100), 0).unwrap();
        observer.observe(tick(5, 120), 100).unwrap();
        assert_eq!(observer.missed_frames(), 2);
        assert_eq!(
            observer.observe(tick(5, 150), 200),
            Err(ObserveError::DuplicateOrOldSequence)
        );
        assert_eq!(
            observer.observe(tick(4, 150), 300),
            Err(ObserveError::DuplicateOrOldSequence)
        );
    }

    #[test]
    fn sequence_wrap_is_valid() {
        let mut observer = Observer::new(1000);
        observer.observe(tick(u32::MAX, 99), 0).unwrap();
        observer.observe(tick(0, 100), 100).unwrap();
        assert_eq!(observer.missed_frames(), 0);
    }
}
