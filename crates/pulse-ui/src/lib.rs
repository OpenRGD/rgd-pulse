#![no_std]

//! PULSE-0.2B four-key Chronoscope navigation.
//!
//! Non-actuating state machine, independent from hardware, USB and rendering.
//! All controls are local. No joystick or K-key represents a motion command.

pub mod color_lab;
pub mod imu;
pub mod input_lab;
pub mod preferences;
pub mod input_repeat;
pub mod settings;
pub mod skins;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Page {
    Somatic = 0,
    Chronons = 1,
    Imu = 2,
    System = 3,
}

impl Page {
    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Somatic => "SOMATIC",
            Self::Chronons => "CHRONONS",
            Self::Imu => "IMU",
            Self::System => "SYSTEM",
        }
    }

    /// Physical K identity: a page button is K1..K4, never a rolling count.
    /// Independent from the diagnostic input-event counter.
    pub const fn physical_key_label(self) -> &'static str {
        match self {
            Self::Somatic => "K1",
            Self::Chronons => "K2",
            Self::Imu => "K3",
            Self::System => "K4",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Browse,
    Focus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Page(Page),
    Up,
    Down,
    Left,
    Right,
    Click,
}

/// Physical INPUT LAB measurements supplied by the operator on 2026-10-24,
/// board held with joystick LEFT, K1..K4 RIGHT:
///
///   physical UP=GP18; DOWN=GP20; LEFT=GP16; RIGHT=GP19; CLICK=GP17.
///   K1=GP8; K2=GP4; K3=GP3; K4=GP2 remain unchanged.
///
/// These observations deliberately override the earlier orientation-derived
/// guess and differ from the vendor's nominal GP17/GP18 right/center labels.
/// Re-verify UP/CLICK in the next physical regression; do not alter raw GPIO
/// wiring or infer hardware damage from the unexpected pin association.
pub const fn landscape_action(raw_index: usize) -> Option<Action> {
    match raw_index {
        0 => Some(Action::Page(Page::Somatic)),  // K1 GP8
        1 => Some(Action::Page(Page::Chronons)), // K2 GP4
        2 => Some(Action::Page(Page::Imu)),      // K3 GP3
        3 => Some(Action::Page(Page::System)),   // K4 GP2
        4 => Some(Action::Left),                 // GP16, physical LEFT
        5 => Some(Action::Down),                 // GP20, physical DOWN
        6 => Some(Action::Click),                // GP17, physical CLICK
        7 => Some(Action::Up),                   // GP18, physical UP
        8 => Some(Action::Right),                // GP19, physical RIGHT
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiState {
    pub page: Page,
    pub mode: Mode,
    /// Per-page selection: Somatic=metric, Chronons=entry,
    /// IMU=axis, System=tile.
    pub selection: [u8; 4],
    /// Per-page view window or filter index.
    pub window: [u8; 4],
    /// Focus/zoom levels 1x, 2x, 4x, 8x for graph pages.
    pub zoom: u8,
    /// History offset, bounded; 0 shows newest.
    pub pan: u8,
    /// Detail scroll line for event/system inspection.
    pub detail_line: u8,
    /// Recent local UI event, for one-character header feedback.
    pub last_action: Option<Action>,
    /// Advances even at selection boundaries: physical input remains visible.
    pub input_count: u16,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

impl UiState {
    pub const fn new() -> Self {
        Self {
            page: Page::Somatic,
            mode: Mode::Browse,
            selection: [0; 4],
            window: [0; 4],
            zoom: 0,
            pan: 0,
            detail_line: 0,
            last_action: None,
            input_count: 0,
        }
    }

    pub const fn zoom_factor(&self) -> u8 {
        1u8 << self.zoom
    }

    pub const fn selected(&self) -> u8 {
        self.selection[self.page.index()]
    }

    pub const fn window_index(&self) -> u8 {
        self.window[self.page.index()]
    }

    pub const fn window_secs(&self) -> u8 {
        match self.page {
            Page::Somatic => [5, 15, 30][self.window_index() as usize],
            Page::Imu => [10, 20, 30][self.window_index() as usize],
            _ => 0,
        }
    }

    pub const fn is_detail(&self) -> bool {
        matches!(self.mode, Mode::Focus)
    }

    /// Returns true if the screen state changed.
    pub fn handle(&mut self, action: Action) -> bool {
        // Show a tiny key indicator and rotating count on *every* real
        // joystick action, even when selection/zoom is already at a limit.
        // This is UI feedback only; never infer Somatic or motor activity.
        self.last_action = Some(action);
        self.input_count = self.input_count.wrapping_add(1);
        if let Action::Page(page) = action {
            self.page = page;
            self.mode = Mode::Browse;
            self.zoom = 0;
            self.pan = 0;
            self.detail_line = 0;
            // Selecting an already-open page closes details too.
            return true;
        }

        if action == Action::Click {
            self.mode = if self.mode == Mode::Browse {
                Mode::Focus
            } else {
                Mode::Browse
            };
            self.zoom = 0;
            self.pan = 0;
            self.detail_line = 0;
            return true;
        }

        let index = self.page.index();
        if self.mode == Mode::Browse {
            match action {
                Action::Up => self.selection[index] = self.selection[index].saturating_sub(1),
                Action::Down => {
                    let max = match self.page {
                        Page::Somatic => 2,  // Hz / jitter / duration
                        Page::Chronons => 3, // four synthetic timeline rows in MVP
                        Page::Imu => 3,      // roll / pitch / gyro / acceleration
                        Page::System => 3,   // motors / CAN / power / host
                    };
                    self.selection[index] = self.selection[index].saturating_add(1).min(max);
                }
                Action::Left => self.window[index] = self.window[index].saturating_sub(1),
                Action::Right => {
                    let max = if self.page == Page::Chronons {
                        4
                    } else if self.page == Page::System {
                        3
                    } else {
                        2
                    };
                    self.window[index] = self.window[index].saturating_add(1).min(max);
                }
                _ => {}
            }
        } else {
            match self.page {
                Page::Somatic | Page::Imu => match action {
                    Action::Up => self.zoom = self.zoom.saturating_add(1).min(3),
                    Action::Down => self.zoom = self.zoom.saturating_sub(1),
                    Action::Left => self.pan = self.pan.saturating_add(4).min(60),
                    Action::Right => self.pan = self.pan.saturating_sub(4),
                    _ => {}
                },
                Page::Chronons => match action {
                    Action::Up => self.detail_line = self.detail_line.saturating_sub(1),
                    Action::Down => self.detail_line = self.detail_line.saturating_add(1).min(4),
                    Action::Left => self.selection[index] = self.selection[index].saturating_sub(1),
                    Action::Right => {
                        self.selection[index] = self.selection[index].saturating_add(1).min(3)
                    }
                    _ => {}
                },
                Page::System => match action {
                    Action::Up => self.detail_line = self.detail_line.saturating_sub(1),
                    Action::Down => self.detail_line = self.detail_line.saturating_add(1).min(3),
                    Action::Left => self.selection[index] = self.selection[index].saturating_sub(1),
                    Action::Right => {
                        self.selection[index] = self.selection[index].saturating_add(1).min(3)
                    }
                    _ => {}
                },
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_feedback_uses_physical_key_not_press_counter() {
        for (page, label) in [
            (Page::Somatic, "K1"),
            (Page::Chronons, "K2"),
            (Page::Imu, "K3"),
            (Page::System, "K4"),
        ] {
            assert_eq!(page.physical_key_label(), label);
            let mut ui = UiState::new();
            for _ in 0..20 {
                ui.handle(Action::Page(page));
                assert_eq!(ui.page.physical_key_label(), label);
            }
        }
    }

    #[test]
    fn buttons_directly_select_all_four_pages() {
        let mut ui = UiState::new();
        for (raw, page) in [
            (0, Page::Somatic),
            (1, Page::Chronons),
            (2, Page::Imu),
            (3, Page::System),
        ] {
            assert_eq!(landscape_action(raw), Some(Action::Page(page)));
            ui.handle(landscape_action(raw).unwrap());
            assert_eq!(ui.page, page);
        }
    }

    #[test]
    fn landscape_joystick_is_transformed_at_input_boundary() {
        // Operator's five physical Input Lab observations from 2026-10-24.
        // Keep index=>GPIO conversion independent from the direction map.
        for (raw, gpio, action) in [
            (7, 18, Action::Up),
            (5, 20, Action::Down),
            (4, 16, Action::Left),
            (8, 19, Action::Right),
            (6, 17, Action::Click),
        ] {
            assert_eq!(input_lab::raw_gpio(raw as u8), Some(gpio));
            assert_eq!(landscape_action(raw), Some(action));
        }
        assert_eq!(landscape_action(9), None);
    }

    #[test]
    fn physically_reported_click_gp17_opens_and_closes_chronon_detail() {
        // Prior prototype software treated GP18 as CLICK. On this observed
        // assembly the operator reports GP17 is the joystick center press.
        let mut ui = UiState::new();
        ui.handle(Action::Page(Page::Chronons));
        let raw_for_gp17 = 6;
        assert_eq!(input_lab::raw_gpio(raw_for_gp17), Some(17));
        assert_eq!(landscape_action(raw_for_gp17 as usize), Some(Action::Click));
        ui.handle(landscape_action(raw_for_gp17 as usize).unwrap());
        assert_eq!(ui.mode, Mode::Focus);
        ui.handle(landscape_action(raw_for_gp17 as usize).unwrap());
        assert_eq!(ui.mode, Mode::Browse);
    }

    #[test]
    fn physically_reported_up_gp18_selects_previous_chronon() {
        let mut ui = UiState::new();
        ui.handle(Action::Page(Page::Chronons));
        ui.handle(Action::Down);
        assert_eq!(ui.selected(), 1);
        let raw_for_gp18 = 7;
        assert_eq!(input_lab::raw_gpio(raw_for_gp18), Some(18));
        assert_eq!(landscape_action(raw_for_gp18 as usize), Some(Action::Up));
        ui.handle(landscape_action(raw_for_gp18 as usize).unwrap());
        assert_eq!(ui.selected(), 0);
    }

    #[test]
    fn input_feedback_advances_even_at_selection_bounds() {
        let mut ui = UiState::new();
        assert_eq!(ui.selected(), 0);
        assert!(ui.handle(Action::Up)); // first selection already bounded at 0
        assert_eq!(ui.selected(), 0);
        assert_eq!(ui.last_action, Some(Action::Up));
        assert_eq!(ui.input_count, 1);
        assert!(ui.handle(Action::Page(Page::Somatic)));
        assert_eq!(ui.input_count, 2);
    }

    #[test]
    fn focus_zoom_and_pan_do_not_change_runtime() {
        let mut ui = UiState::new();
        assert_eq!(ui.zoom_factor(), 1);
        ui.handle(Action::Click);
        assert_eq!(ui.mode, Mode::Focus);
        ui.handle(Action::Up);
        ui.handle(Action::Up);
        assert_eq!(ui.zoom_factor(), 4);
        ui.handle(Action::Left);
        assert_eq!(ui.pan, 4);
        ui.handle(Action::Right);
        assert_eq!(ui.pan, 0);
        ui.handle(Action::Click);
        assert_eq!(ui.mode, Mode::Browse);
        assert_eq!(ui.zoom_factor(), 1);
    }

    #[test]
    fn chronons_click_opens_and_closes_detail() {
        let mut ui = UiState::new();
        ui.handle(Action::Page(Page::Chronons));
        ui.handle(Action::Down);
        assert_eq!(ui.selected(), 1);
        ui.handle(Action::Click);
        ui.handle(Action::Down);
        assert_eq!(ui.detail_line, 1);
        ui.handle(Action::Right);
        assert_eq!(ui.selected(), 2);
        ui.handle(Action::Click);
        assert_eq!(ui.mode, Mode::Browse);
    }

    #[test]
    fn system_has_four_bounded_tiles_and_navigation_keys_escape() {
        let mut ui = UiState::new();
        ui.handle(Action::Page(Page::System));
        for _ in 0..99 {
            ui.handle(Action::Down);
        }
        assert_eq!(ui.selected(), 3);
        ui.handle(Action::Click);
        ui.handle(Action::Page(Page::Somatic));
        assert_eq!(ui.page, Page::Somatic);
        assert_eq!(ui.mode, Mode::Browse);
    }

    #[test]
    fn zoom_and_pan_saturate() {
        let mut ui = UiState::new();
        ui.handle(Action::Click);
        for _ in 0..200 {
            ui.handle(Action::Up);
            ui.handle(Action::Left);
        }
        assert_eq!(ui.zoom_factor(), 8);
        assert_eq!(ui.pan, 60);
        for _ in 0..200 {
            ui.handle(Action::Down);
            ui.handle(Action::Right);
        }
        assert_eq!(ui.zoom_factor(), 1);
        assert_eq!(ui.pan, 0);
    }

    #[test]
    fn view_windows_are_bounded_and_per_page() {
        let mut ui = UiState::new();
        for _ in 0..10 {
            ui.handle(Action::Right);
        }
        assert_eq!(ui.window_secs(), 30);
        ui.handle(Action::Page(Page::Imu));
        assert_eq!(ui.window_secs(), 10);
        ui.handle(Action::Right);
        assert_eq!(ui.window_secs(), 20);
    }
}
