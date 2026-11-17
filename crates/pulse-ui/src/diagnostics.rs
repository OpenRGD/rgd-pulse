//! On-device diagnostics for the Pico Chronoscope, without robot commands.
//! K3 + K4 (1.2s) opens the diagnostic overlay.
//! PREFS shows only journal status; CLICK switches to LCD GEOMETRY.
//! In GEOMETRY LEFT/RIGHT adjust X, UP/DOWN adjust Y (0..4), CLICK
//! applies the proposed address offsets for this power-on session ONLY.
//! Hold K3+K4 to cancel/close. Never writes flash, IMU, CAN or motors.

use crate::Action;

pub const DIAGNOSTIC_CHORD: u16 = (1 << 2) | (1 << 3);
pub const HOLD_MS: u64 = 1_200;
/// Operator-verified ST7735S landscape X origin, 27 Oct 2026.
pub const DEFAULT_X: u8 = 1;
/// Operator-verified ST7735S landscape Y origin, 27 Oct 2026.
pub const DEFAULT_Y: u8 = 2;
const LIMIT: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Preferences,
    Geometry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Diagnostics {
    pub open: bool,
    pub screen: Screen,
    pub preview_x: u8,
    pub preview_y: u8,
    applied_x: u8,
    applied_y: u8,
    since_ms: Option<u64>,
    consumed: bool,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl Diagnostics {
    pub const fn new() -> Self {
        Self {
            open: false,
            screen: Screen::Preferences,
            preview_x: DEFAULT_X,
            preview_y: DEFAULT_Y,
            applied_x: DEFAULT_X,
            applied_y: DEFAULT_Y,
            since_ms: None,
            consumed: false,
        }
    }

    pub const fn chord_held(&self, mask: u16) -> bool {
        mask & 0x0f == DIAGNOSTIC_CHORD
    }

    pub const fn lcd_offsets(&self) -> (u16, u16) {
        if self.open && matches!(self.screen, Screen::Geometry) {
            (self.preview_x as u16, self.preview_y as u16)
        } else {
            (self.applied_x as u16, self.applied_y as u16)
        }
    }

    pub fn observe(&mut self, mask: u16, now_ms: u64, enabled: bool) -> bool {
        if !enabled || !self.chord_held(mask) {
            self.since_ms = None;
            self.consumed = false;
            return false;
        }
        match self.since_ms {
            None => {
                self.since_ms = Some(now_ms);
                false
            }
            Some(start) if !self.consumed && now_ms.saturating_sub(start) >= HOLD_MS => {
                self.consumed = true;
                if self.open {
                    self.cancel();
                } else {
                    self.open = true;
                    self.screen = Screen::Preferences;
                    self.preview_x = self.applied_x;
                    self.preview_y = self.applied_y;
                }
                true
            }
            _ => false,
        }
    }

    pub fn cancel(&mut self) {
        self.open = false;
        self.screen = Screen::Preferences;
        self.preview_x = self.applied_x;
        self.preview_y = self.applied_y;
    }

    pub fn handle(&mut self, action: Action) -> bool {
        if !self.open {
            return false;
        }
        match self.screen {
            Screen::Preferences => {
                if action == Action::Click {
                    self.screen = Screen::Geometry;
                    return true;
                }
            }
            Screen::Geometry => {
                match action {
                    Action::Left => self.preview_x = self.preview_x.saturating_sub(1),
                    Action::Right => self.preview_x = self.preview_x.saturating_add(1).min(LIMIT),
                    Action::Up => self.preview_y = self.preview_y.saturating_sub(1),
                    Action::Down => self.preview_y = self.preview_y.saturating_add(1).min(LIMIT),
                    Action::Click => {
                        self.applied_x = self.preview_x;
                        self.applied_y = self.preview_y;
                        self.open = false;
                    }
                    Action::Page(_) => return false,
                }
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_is_exclusive_of_other_existing_labs() {
        let mut lab = Diagnostics::new();
        for other in [0b0011, 0b1001, 0b0110, 0b1111] {
            assert!(!lab.observe(other, 0, true));
            assert!(!lab.observe(other, 1600, true));
            assert!(!lab.open);
        }
        assert!(!lab.observe(DIAGNOSTIC_CHORD, 10, true));
        assert!(lab.observe(DIAGNOSTIC_CHORD, 1210, true));
        assert!(lab.open);
        assert!(!lab.observe(DIAGNOSTIC_CHORD, 3000, true));
        lab.observe(0, 3001, true);
        assert!(!lab.observe(DIAGNOSTIC_CHORD, 3200, true));
        assert!(lab.observe(DIAGNOSTIC_CHORD, 4400, true));
        assert!(!lab.open);
    }

    #[test]
    fn geometry_changes_are_preview_only_until_click() {
        let mut lab = Diagnostics::new();
        lab.observe(DIAGNOSTIC_CHORD, 0, true);
        lab.observe(DIAGNOSTIC_CHORD, 1200, true);
        lab.observe(0, 1201, true);
        assert_eq!(lab.lcd_offsets(), (1, 2));
        assert!(lab.handle(Action::Click));
        assert_eq!(lab.screen, Screen::Geometry);
        lab.handle(Action::Left);
        lab.handle(Action::Up);
        assert_eq!(lab.lcd_offsets(), (0, 1));
        lab.cancel();
        assert_eq!(lab.lcd_offsets(), (1, 2));
    }

    #[test]
    fn apply_is_ram_only_and_offsets_are_bounded() {
        let mut lab = Diagnostics::new();
        lab.open = true;
        lab.screen = Screen::Geometry;
        for _ in 0..20 {
            lab.handle(Action::Right);
            lab.handle(Action::Down);
        }
        assert_eq!(lab.lcd_offsets(), (4, 4));
        lab.handle(Action::Click);
        assert!(!lab.open);
        assert_eq!(lab.lcd_offsets(), (4, 4));
        assert_eq!(Diagnostics::new().lcd_offsets(), (1, 2));
    }
}
