//! Local read-only HMI settings overlay: Winamp-inspired skin switching.
//!
//! Hold K2+K3 for 1200ms to open/close. Joystick directions browse a live
//! *preview*; center CLICK applies that preset and closes the overlay.
//! Cancel/chord exit restores the previously applied skin. Skin selection
//! lives in SRAM for this wave; no RP2040 flash or host preference writes.

use crate::{skins::SkinId, Action};

pub const SETTINGS_CHORD: u16 = (1 << 1) | (1 << 2);
pub const SETTINGS_HOLD_MS: u64 = 1200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    pub open: bool,
    pub applied: SkinId,
    pub preview: SkinId,
    chord_since_ms: Option<u64>,
    consumed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    pub const fn new() -> Self {
        Self {
            open: false,
            applied: SkinId::Classic,
            preview: SkinId::Classic,
            chord_since_ms: None,
            consumed: false,
        }
    }

    pub const fn chord_held(&self, mask: u16) -> bool {
        mask & 0x0f == SETTINGS_CHORD
    }

    /// True only when entry/exit state changed. If another overlay is active,
    /// the settings chord is disabled and its hold timer is reset.
    pub fn observe(&mut self, mask: u16, now_ms: u64, enabled: bool) -> bool {
        if !enabled || !self.chord_held(mask) {
            self.chord_since_ms = None;
            self.consumed = false;
            return false;
        }
        match self.chord_since_ms {
            None => {
                self.chord_since_ms = Some(now_ms);
                false
            }
            Some(start) if !self.consumed && now_ms.saturating_sub(start) >= SETTINGS_HOLD_MS => {
                self.consumed = true;
                if self.open {
                    self.cancel();
                } else {
                    self.open = true;
                    self.preview = self.applied;
                }
                true
            }
            _ => false,
        }
    }

    /// Skin used by the renderer, including instant previews in settings.
    pub const fn visual_skin(&self) -> SkinId {
        if self.open {
            self.preview
        } else {
            self.applied
        }
    }

    pub fn cancel(&mut self) {
        self.open = false;
        self.preview = self.applied;
    }

    /// Local menu actions; never interpreted as a robot command.
    pub fn handle(&mut self, action: Action) -> bool {
        if !self.open {
            return false;
        }
        match action {
            Action::Left | Action::Up => self.preview = self.preview.previous(),
            Action::Right | Action::Down => self.preview = self.preview.next(),
            Action::Click => {
                self.applied = self.preview;
                self.open = false;
            }
            Action::Page(_) => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Page;

    fn open(x: &mut Settings, t: u64) {
        assert!(!x.observe(SETTINGS_CHORD, t, true));
        assert!(x.observe(SETTINGS_CHORD, t + SETTINGS_HOLD_MS, true));
        assert!(x.open);
        x.observe(0, t + SETTINGS_HOLD_MS + 1, true);
    }

    #[test]
    fn hold_only_not_short_chord_opens_settings() {
        let mut s = Settings::new();
        s.observe(SETTINGS_CHORD, 1000, true);
        assert!(!s.observe(SETTINGS_CHORD, 2199, true));
        assert!(!s.open);
        s.observe(0, 2200, true);
        open(&mut s, 3000);
    }

    #[test]
    fn previews_cycle_and_click_applies_until_reset() {
        let mut s = Settings::new();
        open(&mut s, 0);
        assert_eq!(s.visual_skin(), SkinId::Classic);
        s.handle(Action::Right);
        assert_eq!(s.visual_skin(), SkinId::Phosphor);
        assert_eq!(s.applied, SkinId::Classic);
        s.handle(Action::Down);
        assert_eq!(s.visual_skin(), SkinId::Chrome99);
        s.handle(Action::Click);
        assert!(!s.open);
        assert_eq!(s.applied, SkinId::Chrome99);
        assert_eq!(s.visual_skin(), SkinId::Chrome99);
    }

    #[test]
    fn cancellation_rolls_back_and_chord_latches() {
        let mut s = Settings::new();
        open(&mut s, 0);
        s.handle(Action::Left);
        assert_eq!(s.preview, SkinId::Italia);
        assert_eq!(s.applied, SkinId::Classic);
        assert!(!s.observe(SETTINGS_CHORD, 2000, true));
        assert!(s.observe(SETTINGS_CHORD, 3200, true));
        assert!(!s.open);
        assert_eq!(s.visual_skin(), SkinId::Classic);
        assert!(!s.observe(SETTINGS_CHORD, 7000, true));
    }

    #[test]
    fn cannot_enter_settings_while_input_lab_active() {
        let mut s = Settings::new();
        s.observe(SETTINGS_CHORD, 0, false);
        assert!(!s.observe(SETTINGS_CHORD, 2000, false));
        assert!(!s.open);
        open(&mut s, 3000);
    }

    #[test]
    fn k_page_press_does_not_change_the_skin() {
        let mut s = Settings::new();
        open(&mut s, 0);
        assert!(!s.handle(Action::Page(Page::Somatic)));
        assert_eq!(s.preview, SkinId::Classic);
    }

    #[test]
    fn three_or_four_page_keys_never_trigger_chord() {
        let mut s = Settings::new();
        assert!(!s.observe(0b1110, 0, true));
        assert!(!s.observe(0b1110, 2000, true));
        assert!(!s.open);
    }
}
