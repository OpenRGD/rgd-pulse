//! Pure, no_std physical input diagnostic state.
//!
//! K1+K4 held for 1200 ms toggles the local on-device Input Lab.
//! The lab reports raw pins independently of the assumed logical mapping:
//! an operator can move the real joystick and compare "GPIO" to "MAP".
//! No commands or USB messages are emitted.

pub const DIAGNOSTIC_CHORD: u16 = (1 << 0) | (1 << 3);
pub const HOLD_TO_TOGGLE_MS: u64 = 1_200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputLab {
    pub active: bool,
    pub last_raw: Option<u8>,
    pub events: u32,
    held_mask: u16,
    chord_since_ms: Option<u64>,
    chord_consumed: bool,
}

impl Default for InputLab {
    fn default() -> Self {
        Self::new()
    }
}

impl InputLab {
    pub const fn new() -> Self {
        Self {
            active: false,
            last_raw: None,
            events: 0,
            held_mask: 0,
            chord_since_ms: None,
            chord_consumed: false,
        }
    }

    pub const fn held_mask(&self) -> u16 {
        self.held_mask
    }

    pub const fn chord_held(&self) -> bool {
        self.held_mask & DIAGNOSTIC_CHORD == DIAGNOSTIC_CHORD
    }

    pub const fn last_held(&self) -> bool {
        match self.last_raw {
            Some(index) if index < 9 => self.held_mask & (1 << index) != 0,
            _ => false,
        }
    }

    /// Process a whole sampled GPIO state. Returns whether the screen needs
    /// a redraw. Holding keys does not generate repeated new press events.
    pub fn observe(&mut self, mask: u16, now_ms: u64) -> bool {
        let mask = mask & 0x01ff;
        let previous = self.held_mask;
        let chord = mask & DIAGNOSTIC_CHORD == DIAGNOSTIC_CHORD;
        if !chord {
            self.chord_since_ms = None;
            self.chord_consumed = false;
        } else if self.chord_since_ms.is_none() {
            self.chord_since_ms = Some(now_ms);
        }

        let mut changed = false;
        if chord
            && !self.chord_consumed
            && now_ms.saturating_sub(self.chord_since_ms.unwrap_or(now_ms)) >= HOLD_TO_TOGGLE_MS
        {
            self.active = !self.active;
            self.last_raw = None;
            self.events = 0;
            self.chord_consumed = true;
            changed = true;
        }

        if self.active && !chord {
            let pressed_edges = mask & !previous;
            for index in 0..9 {
                if (pressed_edges & (1 << index)) != 0 {
                    self.last_raw = Some(index as u8);
                    self.events = self.events.saturating_add(1);
                    changed = true;
                }
            }
        }

        if self.active && mask != previous {
            changed = true;
        }
        self.held_mask = mask;
        changed
    }
}

pub const fn raw_gpio(raw: u8) -> Option<u8> {
    match raw {
        0 => Some(8),  // K1
        1 => Some(4),  // K2
        2 => Some(3),  // K3
        3 => Some(2),  // K4
        4 => Some(16), // raw GP16, operator measured physical LEFT
        5 => Some(20), // raw GP20, operator measured physical DOWN
        6 => Some(17), // raw GP17, operator measured physical CLICK
        7 => Some(18), // raw GP18, operator measured physical UP
        8 => Some(19), // raw GP19, operator measured physical RIGHT
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_hold_toggles_on_once_and_never_repeats_without_release() {
        let mut d = InputLab::new();
        let chord = DIAGNOSTIC_CHORD;
        assert!(!d.observe(chord, 100));
        assert!(!d.observe(chord, 1299));
        assert!(!d.active);
        assert!(d.observe(chord, 1300));
        assert!(d.active);
        assert!(!d.observe(chord, 5000));
        assert!(d.active);
        assert!(d.observe(0, 5100));
        assert!(!d.chord_held());
        // Pressing the chord redraws the active input panel immediately,
        // but should not toggle it until the full 1200ms has elapsed.
        assert!(d.observe(chord, 5200));
        assert!(d.active);
        assert!(d.observe(chord, 6400));
        assert!(!d.active);
    }

    #[test]
    fn short_k1_k4_press_does_not_enter_diagnostics() {
        let mut d = InputLab::new();
        d.observe(DIAGNOSTIC_CHORD, 1000);
        d.observe(0, 1100);
        assert!(!d.active);
        assert_eq!(d.last_raw, None);
    }

    #[test]
    fn raw_edges_are_reported_without_changing_logical_mapping() {
        let mut d = InputLab::new();
        d.observe(DIAGNOSTIC_CHORD, 0);
        d.observe(DIAGNOSTIC_CHORD, 1200);
        d.observe(0, 1230);
        assert!(d.observe(1 << 4, 1300));
        assert_eq!(d.last_raw, Some(4));
        assert_eq!(raw_gpio(4), Some(16));
        assert!(d.last_held());
        assert_eq!(d.events, 1);
        assert!(!d.observe(1 << 4, 1400));
        assert!(d.observe(0, 1500));
        assert_eq!(d.last_raw, Some(4));
        assert!(!d.last_held());
        assert_eq!(d.events, 1);
        d.observe(1 << 7, 1650);
        assert_eq!(raw_gpio(d.last_raw.unwrap()), Some(18));
        assert_eq!(d.events, 2);
    }

    #[test]
    fn gpio_mapping_uses_all_nine_pins() {
        let observed = [8, 4, 3, 2, 16, 20, 17, 18, 19];
        for (raw, gpio) in observed.iter().enumerate() {
            assert_eq!(raw_gpio(raw as u8), Some(*gpio));
        }
        assert_eq!(raw_gpio(9), None);
    }

    #[test]
    fn chord_exit_does_not_count_as_a_raw_input_event() {
        let mut d = InputLab::new();
        d.observe(DIAGNOSTIC_CHORD, 0);
        d.observe(DIAGNOSTIC_CHORD, 1200);
        d.observe(0, 1240);
        d.observe(1 << 6, 1300);
        assert_eq!(d.events, 1);
        d.observe(0, 1400);
        d.observe(DIAGNOSTIC_CHORD, 1450);
        assert_eq!(d.events, 1);
        d.observe(DIAGNOSTIC_CHORD, 2650);
        assert!(!d.active);
        assert_eq!(d.events, 0);
        assert_eq!(d.last_raw, None);
    }
}
