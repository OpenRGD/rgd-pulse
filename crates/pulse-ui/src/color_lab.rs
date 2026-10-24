//! Physical LCD color-calibration fixture for ST7735S.
//!
//! This is a local, reversible **display-register-only** diagnostic. It never
//! touches Paky-01 IMU/CAN/motors, Pico XIP flash preferences, or USB frames.
//!
//! Operator reported native RGB565 [GREEN, WHITE, RED] showing as
//! [MAGENTA, BLACK, YELLOW]. That corresponds to bitwise complement plus an
//! R/B swap, but software must not assume the panel's actual transfer curve.
//! Physical Color Lab acceptance on 2026-10-24: operator confirmed
//! NORM / BGR (MADCTL=0x68, INVOFF=0x20) displays correct G/W/R.
//! Keep all four reversible test modes while using the measured good
//! configuration as the startup default on every Pico boot.

use crate::Action;

/// K1 + K2 together, held for 1.2 seconds, separate from
/// K1+K4 INPUT LAB and K2+K3 SETTINGS/SKINS.
pub const COLOR_LAB_CHORD: u16 = (1 << 0) | (1 << 1);
pub const COLOR_LAB_HOLD_MS: u64 = 1_200;
const LANDSCAPE_MX_MV: u8 = 0x60;
const BGR_BIT: u8 = 0x08;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ColorMode {
    /// Original firmware: inversion ON, RGB setting. Operator saw
    /// magenta/black/yellow instead of green/white/red.
    InvertedRgb = 0,
    /// Disable inversion, keep RGB setting.
    NormalRgb = 1,
    /// Keep inversion, flip the red/blue order bit.
    InvertedBgr = 2,
    /// Disable inversion, flip R/B order: physically verified on Paky-01.
    NormalBgr = 3,
}

/// Operator-verified display power-on configuration for the assembled
/// Seengreat Rev 1.2 (joystick LEFT / K1-K4 RIGHT) on 2026-10-24.
/// This constant is used by the LCD init, the Color Lab and the firmware
/// runtime tracker to prevent mismatch or initial wrong-color splash.
pub const VALIDATED_BOOT_MODE: ColorMode = ColorMode::NormalBgr;

impl ColorMode {
    pub const fn from_index(value: u8) -> Self {
        match value {
            1 => Self::NormalRgb,
            2 => Self::InvertedBgr,
            3 => Self::NormalBgr,
            _ => Self::InvertedRgb,
        }
    }

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::InvertedRgb => "INV  / RGB",
            Self::NormalRgb => "NORM / RGB",
            Self::InvertedBgr => "INV  / BGR",
            Self::NormalBgr => "NORM / BGR",
        }
    }

    /// Keep MX|MV 90-degree landscape rotation unchanged.
    pub const fn madctl(self) -> u8 {
        match self {
            Self::InvertedRgb | Self::NormalRgb => LANDSCAPE_MX_MV,
            Self::InvertedBgr | Self::NormalBgr => LANDSCAPE_MX_MV | BGR_BIT,
        }
    }

    pub const fn inversion_command(self) -> u8 {
        match self {
            Self::InvertedRgb | Self::InvertedBgr => 0x21, // INVON
            Self::NormalRgb | Self::NormalBgr => 0x20,     // INVOFF
        }
    }

    pub const fn next(self) -> Self {
        Self::from_index((self.index() + 1) & 0x03)
    }

    pub const fn previous(self) -> Self {
        Self::from_index((self.index() + 3) & 0x03)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorLab {
    pub open: bool,
    pub applied: ColorMode,
    pub preview: ColorMode,
    chord_since_ms: Option<u64>,
    consumed: bool,
}

impl Default for ColorLab {
    fn default() -> Self {
        Self::new()
    }
}

impl ColorLab {
    pub const fn new() -> Self {
        Self {
            open: false,
            applied: VALIDATED_BOOT_MODE,
            preview: VALIDATED_BOOT_MODE,
            chord_since_ms: None,
            consumed: false,
        }
    }

    pub const fn chord_held(&self, mask: u16) -> bool {
        mask & 0x0f == COLOR_LAB_CHORD
    }

    pub const fn visual_mode(&self) -> ColorMode {
        if self.open {
            self.preview
        } else {
            self.applied
        }
    }

    /// Enabled only when the other local overlays are closed.
    /// Chord is latched once per continuous hold to prevent oscillation.
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
            Some(since) if !self.consumed && now_ms.saturating_sub(since) >= COLOR_LAB_HOLD_MS => {
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

    pub fn cancel(&mut self) {
        self.open = false;
        self.preview = self.applied;
    }

    /// CLICK commits, held K1+K2 cancels; all operations remain on-device.
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

    #[test]
    fn physical_color_calibration_is_the_boot_default() {
        let c = ColorLab::new();
        assert_eq!(VALIDATED_BOOT_MODE, ColorMode::NormalBgr);
        assert_eq!(VALIDATED_BOOT_MODE.madctl(), 0x68);
        assert_eq!(VALIDATED_BOOT_MODE.inversion_command(), 0x20);
        assert_eq!(c.applied, VALIDATED_BOOT_MODE);
        assert_eq!(c.preview, VALIDATED_BOOT_MODE);
        assert_eq!(c.visual_mode(), VALIDATED_BOOT_MODE);
        assert!(!c.open);
        // After restoring the default (reboot), the old inverted mode is
        // still testable by joystick navigation, but never the default.
        assert_eq!(c.visual_mode().next(), ColorMode::InvertedRgb);
    }

    #[test]
    fn all_four_modes_keep_original_landscape_and_use_expected_commands() {
        for (mode, madctl, invert) in [
            (ColorMode::InvertedRgb, 0x60, 0x21),
            (ColorMode::NormalRgb, 0x60, 0x20),
            (ColorMode::InvertedBgr, 0x68, 0x21),
            (ColorMode::NormalBgr, 0x68, 0x20),
        ] {
            assert_eq!(mode.madctl(), madctl);
            assert_eq!(mode.inversion_command(), invert);
            assert!(mode.label().contains('/'));
        }
    }

    #[test]
    fn next_previous_wrapping_is_bounded() {
        assert_eq!(ColorMode::InvertedRgb.previous(), ColorMode::NormalBgr);
        assert_eq!(ColorMode::NormalBgr.next(), ColorMode::InvertedRgb);
        assert_eq!(ColorMode::from_index(255), ColorMode::InvertedRgb);
    }

    #[test]
    fn only_long_k1_k2_chord_enters_lab() {
        let mut c = ColorLab::new();
        c.observe(COLOR_LAB_CHORD, 50, true);
        assert!(!c.observe(COLOR_LAB_CHORD, 1249, true));
        assert!(!c.open);
        assert!(c.observe(COLOR_LAB_CHORD, 1250, true));
        assert!(c.open);
        assert!(!c.observe(COLOR_LAB_CHORD, 5000, true));
        c.observe(0, 5100, true);
        assert!(!c.observe(COLOR_LAB_CHORD, 5200, true));
        assert!(c.observe(COLOR_LAB_CHORD, 6400, true));
        assert!(!c.open);
    }

    #[test]
    fn preview_navigation_commit_and_cancel_are_reversible() {
        let mut c = ColorLab::new();
        c.observe(COLOR_LAB_CHORD, 0, true);
        c.observe(COLOR_LAB_CHORD, 1200, true);
        c.observe(0, 1201, true);
        c.handle(Action::Right);
        c.handle(Action::Right);
        c.handle(Action::Right);
        assert_eq!(c.preview, ColorMode::InvertedBgr);
        assert_eq!(c.applied, ColorMode::NormalBgr);
        c.cancel();
        assert_eq!(c.visual_mode(), ColorMode::NormalBgr);
        c.observe(COLOR_LAB_CHORD, 2000, true);
        c.observe(COLOR_LAB_CHORD, 3200, true);
        c.observe(0, 3201, true);
        c.handle(Action::Left);
        assert_eq!(c.preview, ColorMode::InvertedBgr);
        c.handle(Action::Click);
        assert_eq!(c.applied, ColorMode::InvertedBgr);
        assert!(!c.open);
        assert_eq!(c.visual_mode(), ColorMode::InvertedBgr);
    }

    #[test]
    fn never_arms_in_input_lab_or_skin_settings() {
        let mut c = ColorLab::new();
        assert!(!c.observe(COLOR_LAB_CHORD, 0, false));
        assert!(!c.observe(COLOR_LAB_CHORD, 3000, false));
        assert!(!c.open);
        assert!(!c.observe(0b0110, 3500, true)); // K2+K3 skin chord
        assert!(!c.observe(0b1001, 4900, true)); // K1+K4 input chord
        assert!(!c.open);
        assert!(!c.handle(Action::Page(Page::Somatic)));
    }
}
