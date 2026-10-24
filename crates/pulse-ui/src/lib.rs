#![no_std]

//! PULSE-0.2B four-key Chronoscope navigation.
//!
//! Non-actuating state machine, independent from hardware, USB and rendering.
//! All controls are local. No joystick or K-key represents a motion command.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Page {
    Somatic = 0,
    Chronons = 1,
    Imu = 2,
    System = 3,
}

impl Page {
    pub const fn index(self) -> usize { self as usize }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Somatic => "SOMATIC",
            Self::Chronons => "CHRONONS",
            Self::Imu => "IMU",
            Self::System => "SYSTEM",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode { Browse, Focus }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Page(Page),
    Up,
    Down,
    Left,
    Right,
    Click,
}

/// Input bit order from existing, operator-proven board bring-up:
/// K1 GP8, K2 GP4, K3 GP3, K4 GP2,
/// joystick raw LEFT GP16, DOWN GP20, RIGHT GP17, CENTER GP18, UP GP19.
/// In operator landscape (+90 degrees clockwise physical board), the
/// physical joystick directions must be remapped to these logical actions.
/// This map requires final orientation verification with hardware.
pub const fn landscape_action(raw_index: usize) -> Option<Action> {
    match raw_index {
        0 => Some(Action::Page(Page::Somatic)),
        1 => Some(Action::Page(Page::Chronons)),
        2 => Some(Action::Page(Page::Imu)),
        3 => Some(Action::Page(Page::System)),
        4 => Some(Action::Up),    // original LEFT GP16
        5 => Some(Action::Left),  // original DOWN GP20
        6 => Some(Action::Down),  // original RIGHT GP17
        7 => Some(Action::Click), // center GP18
        8 => Some(Action::Right), // original UP GP19
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
}

impl Default for UiState {
    fn default() -> Self { Self::new() }
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
        }
    }

    pub const fn zoom_factor(&self) -> u8 { 1u8 << self.zoom }

    pub const fn selected(&self) -> u8 { self.selection[self.page.index()] }

    pub const fn window_index(&self) -> u8 { self.window[self.page.index()] }

    pub const fn window_secs(&self) -> u8 {
        match self.page {
            Page::Somatic => [5, 15, 30][self.window_index() as usize],
            Page::Imu => [10, 20, 30][self.window_index() as usize],
            _ => 0,
        }
    }

    pub const fn is_detail(&self) -> bool { matches!(self.mode, Mode::Focus) }

    /// Returns true if the screen state changed.
    pub fn handle(&mut self, action: Action) -> bool {
        if let Action::Page(page) = action {
            let changed = self.page != page || self.mode != Mode::Browse;
            self.page = page;
            self.mode = Mode::Browse;
            self.zoom = 0;
            self.pan = 0;
            self.detail_line = 0;
            // Selecting an already-open page closes details too.
            return changed;
        }

        if action == Action::Click {
            self.mode = if self.mode == Mode::Browse { Mode::Focus } else { Mode::Browse };
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
                        Page::Somatic => 2, // Hz / jitter / duration
                        Page::Chronons => 3, // four synthetic timeline rows in MVP
                        Page::Imu => 3, // roll / pitch / gyro / acceleration
                        Page::System => 3, // motors / CAN / power / host
                    };
                    self.selection[index] = self.selection[index].saturating_add(1).min(max);
                }
                Action::Left => self.window[index] = self.window[index].saturating_sub(1),
                Action::Right => {
                    let max = if self.page == Page::Chronons { 4 }
                              else if self.page == Page::System { 3 } else { 2 };
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
                    Action::Right => self.selection[index] = self.selection[index].saturating_add(1).min(3),
                    _ => {}
                },
                Page::System => match action {
                    Action::Up => self.detail_line = self.detail_line.saturating_sub(1),
                    Action::Down => self.detail_line = self.detail_line.saturating_add(1).min(3),
                    Action::Left => self.selection[index] = self.selection[index].saturating_sub(1),
                    Action::Right => self.selection[index] = self.selection[index].saturating_add(1).min(3),
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
    fn buttons_directly_select_all_four_pages() {
        let mut ui=UiState::new();
        for (raw, page) in [(0,Page::Somatic),(1,Page::Chronons),(2,Page::Imu),(3,Page::System)] {
            assert_eq!(landscape_action(raw),Some(Action::Page(page)));
            ui.handle(landscape_action(raw).unwrap());
            assert_eq!(ui.page,page);
        }
    }

    #[test]
    fn landscape_joystick_is_transformed_at_input_boundary() {
        for (raw, action) in [(4,Action::Up),(6,Action::Down),(5,Action::Left),
                              (8,Action::Right),(7,Action::Click)] {
            assert_eq!(landscape_action(raw),Some(action));
        }
        assert_eq!(landscape_action(9),None);
    }

    #[test]
    fn focus_zoom_and_pan_do_not_change_runtime() {
        let mut ui=UiState::new();
        assert_eq!(ui.zoom_factor(),1);
        ui.handle(Action::Click);
        assert_eq!(ui.mode,Mode::Focus);
        ui.handle(Action::Up);
        ui.handle(Action::Up);
        assert_eq!(ui.zoom_factor(),4);
        ui.handle(Action::Left);
        assert_eq!(ui.pan,4);
        ui.handle(Action::Right);
        assert_eq!(ui.pan,0);
        ui.handle(Action::Click);
        assert_eq!(ui.mode,Mode::Browse);
        assert_eq!(ui.zoom_factor(),1);
    }

    #[test]
    fn chronons_click_opens_and_closes_detail() {
        let mut ui=UiState::new();
        ui.handle(Action::Page(Page::Chronons));
        ui.handle(Action::Down);
        assert_eq!(ui.selected(),1);
        ui.handle(Action::Click);
        ui.handle(Action::Down);
        assert_eq!(ui.detail_line,1);
        ui.handle(Action::Right);
        assert_eq!(ui.selected(),2);
        ui.handle(Action::Click);
        assert_eq!(ui.mode,Mode::Browse);
    }

    #[test]
    fn system_has_four_bounded_tiles_and_navigation_keys_escape() {
        let mut ui=UiState::new();
        ui.handle(Action::Page(Page::System));
        for _ in 0..99 { ui.handle(Action::Down); }
        assert_eq!(ui.selected(),3);
        ui.handle(Action::Click);
        ui.handle(Action::Page(Page::Somatic));
        assert_eq!(ui.page,Page::Somatic);
        assert_eq!(ui.mode,Mode::Browse);
    }

    #[test]
    fn zoom_and_pan_saturate() {
        let mut ui=UiState::new();
        ui.handle(Action::Click);
        for _ in 0..200 { ui.handle(Action::Up);ui.handle(Action::Left); }
        assert_eq!(ui.zoom_factor(),8);
        assert_eq!(ui.pan,60);
        for _ in 0..200 { ui.handle(Action::Down);ui.handle(Action::Right); }
        assert_eq!(ui.zoom_factor(),1);
        assert_eq!(ui.pan,0);
    }

    #[test]
    fn view_windows_are_bounded_and_per_page() {
        let mut ui=UiState::new();
        for _ in 0..10 { ui.handle(Action::Right); }
        assert_eq!(ui.window_secs(),30);
        ui.handle(Action::Page(Page::Imu));
        assert_eq!(ui.window_secs(),10);
        ui.handle(Action::Right);
        assert_eq!(ui.window_secs(),20);
    }
}
