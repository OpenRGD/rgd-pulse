//! Compact 128x128 Chronoscope renderer.
//! All charts/events in this wave are unmistakably synthetic UI fixtures.
//! Never display fake IMU, loop, voltage or motor-online measurements.

use core::convert::Infallible;
use core::fmt::Write;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{
        ascii::{FONT_5X8, FONT_7X13_BOLD},
        MonoTextStyle,
    },
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use heapless::String;
use pulse_ui::{
    input_lab::{raw_gpio, InputLab},
    landscape_action,
    settings::Settings,
    skins::{ChromeStyle, SkinId},
    Action, Mode, Page, UiState,
};

pub const PIXELS: usize = 128 * 128;
const BG: Rgb565 = Rgb565::new(1, 2, 5);
const PANEL: Rgb565 = Rgb565::new(3, 7, 13);
const BORDER: Rgb565 = Rgb565::new(5, 15, 20);
const WHITE: Rgb565 = Rgb565::new(31, 62, 31);
const MUTED: Rgb565 = Rgb565::new(12, 26, 19);
const CYAN: Rgb565 = Rgb565::new(7, 58, 31);
const GREEN: Rgb565 = Rgb565::new(10, 59, 16);
const AMBER: Rgb565 = Rgb565::new(31, 38, 3);
const RED: Rgb565 = Rgb565::new(31, 13, 9);

// Literal, full-intensity RGB565 flag swatches: bypass visual role mapping.
// In particular, the third flag bar MUST look red on the physical ST7735S.
// A BLUE third bar indicates the panel's RGB/BGR bit (MADCTL bit 3)
// probably needs a dedicated hardware color-order calibration.
const ITALIA_GREEN: Rgb565 = Rgb565::new(0, 63, 0);
const ITALIA_WHITE: Rgb565 = Rgb565::new(31, 63, 31);
const ITALIA_RED: Rgb565 = Rgb565::new(31, 0, 0);

pub struct Frame<'a> {
    pub data: &'a mut [u16; PIXELS],
    /// Theme is visual only. All four pages keep identical data contracts.
    pub skin: SkinId,
}

/// These visual roles are mapped from the original RGD palette; warning and
/// error colors remain unchanged across every skin.
#[derive(Clone, Copy)]
struct SkinPalette {
    bg: Rgb565,
    panel: Rgb565,
    border: Rgb565,
    text: Rgb565,
    muted: Rgb565,
    accent: Rgb565,
    positive: Rgb565,
}

fn palette(id: SkinId) -> SkinPalette {
    match id {
        SkinId::Classic => SkinPalette {
            bg: BG, panel: PANEL, border: BORDER, text: WHITE,
            muted: MUTED, accent: CYAN, positive: GREEN,
        },
        SkinId::Phosphor => SkinPalette {
            bg: Rgb565::new(0, 2, 1),
            panel: Rgb565::new(1, 7, 2),
            border: Rgb565::new(4, 23, 6),
            text: Rgb565::new(24, 63, 24),
            muted: Rgb565::new(9, 27, 11),
            accent: Rgb565::new(11, 63, 12),
            positive: Rgb565::new(8, 58, 12),
        },
        SkinId::Chrome99 => SkinPalette {
            bg: Rgb565::new(3, 5, 10),
            panel: Rgb565::new(8, 13, 20),
            border: Rgb565::new(17, 29, 27),
            text: Rgb565::new(31, 63, 31),
            muted: Rgb565::new(15, 31, 25),
            accent: Rgb565::new(18, 48, 31),
            positive: Rgb565::new(11, 55, 17),
        },
        SkinId::Italia => SkinPalette {
            // Neutral grays (R/31 ~= G/63 ~= B/31): the former tinted
            // blue-black panels looked purple on the physical panel.
            // Green is now visually dominant instead of blue/teal.
            bg: Rgb565::new(2, 4, 2),
            panel: Rgb565::new(6, 12, 6),
            border: Rgb565::new(13, 26, 13),
            text: ITALIA_WHITE,
            muted: Rgb565::new(17, 34, 17),
            accent: ITALIA_GREEN,
            positive: Rgb565::new(1, 58, 1),
        },
    }
}

impl Frame<'_> {
    fn skin_color(&self, token: Rgb565) -> Rgb565 {
        let p = palette(self.skin);
        if token == BG { p.bg }
        else if token == PANEL { p.panel }
        else if token == BORDER { p.border }
        else if token == WHITE { p.text }
        else if token == MUTED { p.muted }
        else if token == CYAN { p.accent }
        else if token == GREEN { p.positive }
        else { token } // AMBER and RED keep their warning semantics.
    }

    fn clear(&mut self) {
        self.data.fill(palette(self.skin).bg.into_storage());
    }
}

impl OriginDimensions for Frame<'_> {
    fn size(&self) -> Size {
        Size::new(128, 128)
    }
}

impl DrawTarget for Frame<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(pos, color) in pixels {
            if (0..128).contains(&pos.x) && (0..128).contains(&pos.y) {
                self.data[pos.y as usize * 128 + pos.x as usize] = color.into_storage();
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbState { Disconnected, Open, DemoFresh, DemoStale }

pub struct RenderData {
    pub usb: UsbState,
    pub demo_sequence: u32,
    pub invalid_frames: u32,
}

fn fill(f: &mut Frame<'_>, x: i32, y: i32, w: u32, h: u32, c: Rgb565) {
    let _ = Rectangle::new(Point::new(x, y), Size::new(w, h))
        .into_styled(PrimitiveStyle::with_fill(f.skin_color(c))).draw(f);
}
fn stroke(f: &mut Frame<'_>, x: i32, y: i32, w: u32, h: u32, c: Rgb565) {
    let _ = Rectangle::new(Point::new(x, y), Size::new(w, h))
        .into_styled(PrimitiveStyle::with_stroke(f.skin_color(c), 1)).draw(f);
}
fn line(f: &mut Frame<'_>, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb565) {
    let _ = Line::new(Point::new(x0, y0), Point::new(x1, y1))
        .into_styled(PrimitiveStyle::with_stroke(f.skin_color(c), 1)).draw(f);
}
fn small(f: &mut Frame<'_>, text: &str, x: i32, y: i32, c: Rgb565) {
    let _ = Text::with_baseline(text, Point::new(x, y),
        MonoTextStyle::new(&FONT_5X8, f.skin_color(c)), Baseline::Top).draw(f);
}
fn big(f: &mut Frame<'_>, text: &str, x: i32, y: i32, c: Rgb565) {
    let _ = Text::with_baseline(text, Point::new(x, y),
        MonoTextStyle::new(&FONT_7X13_BOLD, f.skin_color(c)), Baseline::Top).draw(f);
}
fn badge(f: &mut Frame<'_>, text: &str, x: i32, y: i32, c: Rgb565) {
    fill(f, x - 2, y - 2, 28, 12, PANEL);
    small(f, text, x, y, c);
}
fn circle(f: &mut Frame<'_>, x: i32, y: i32, diameter: u32, c: Rgb565) {
    let _ = Circle::new(Point::new(x,y), diameter)
        .into_styled(PrimitiveStyle::with_fill(f.skin_color(c))).draw(f);
}

/// Deterministic *illustrative* trend samples; never host/runtime readings.
/// The graph itself is conspicuously labelled DEMO on every page.
fn demo_sample(i: usize, channel: u8) -> i32 {
    let phase = ((i * 7 + channel as usize * 11) % 37) as i32;
    let triangle = if phase < 18 { phase } else { 36 - phase };
    6 + triangle + (((i * 11 + channel as usize * 3) % 6) as i32)
}

fn demo_chart(f: &mut Frame<'_>, x: i32, y: i32, w: u32, h: u32, state: &UiState) {
    fill(f, x, y, w, h, PANEL);
    for i in 1..4 {
        let gy = y + ((i * h) / 4) as i32;
        line(f, x + 1, gy, x + w as i32 - 2, gy, BORDER);
    }
    let span = (64 / state.zoom_factor() as usize).max(2);
    let available = 64 - span;
    let end = 64usize.saturating_sub((state.pan as usize).min(available));
    let start = end.saturating_sub(span);
    let mut old: Option<(i32, i32)> = None;
    for px in 0..(w as usize - 2) {
        let sample = start + px * (span - 1) / ((w as usize - 3).max(1));
        let value = demo_sample(sample, state.selected());
        let yy = y + h as i32 - 3 - value * (h as i32 - 5) / 33;
        let xx = x + 1 + px as i32;
        if let Some((ox,oy)) = old {
            line(f, ox, oy, xx, yy, CYAN);
        }
        old = Some((xx,yy));
    }
    stroke(f, x, y, w, h, BORDER);
    // Chart styling changes with skin, chart values never change.
    match f.skin.manifest().style {
        ChromeStyle::Oscilloscope => {
            for dx in (3..w as usize - 3).step_by(12) {
                let sample = start + dx * (span - 1) / ((w as usize - 3).max(1));
                let value = demo_sample(sample, state.selected());
                let yy = y + h as i32 - 3 - value * (h as i32 - 5) / 33;
                fill(f, x + dx as i32, yy, 2, 2, CYAN);
            }
        }
        ChromeStyle::Beveled => {
            line(f, x + 1, y + 1, x + w as i32 - 2, y + 1, WHITE);
            line(f, x + 1, y + 2, x + 1, y + h as i32 - 2, MUTED);
        }
        ChromeStyle::Tricolore => {
            // Distinct flag colors, not translated green/status roles.
            fill(f, x + 1, y + 1, 4, 4, ITALIA_GREEN);
            fill(f, x + 5, y + 1, 4, 4, ITALIA_WHITE);
            fill(f, x + 9, y + 1, 4, 4, ITALIA_RED);
        }
        ChromeStyle::Clean => {}
    }
}

fn top_bar(f: &mut Frame<'_>, ui: &UiState, state: &RenderData) {
    fill(f, 0, 0, 128, 16, PANEL);
    fill(f, 0, 15, 128, 1, CYAN);
    if f.skin == SkinId::Italia {
        // Full-width, FOUR-pixel tricolore (not the barely visible 1px
        // stripe in the first skin). Literal swatches double as a check
        // of the panel's R/B ordering without changing all other skins.
        fill(f, 0, 12, 42, 4, ITALIA_GREEN);
        fill(f, 42, 12, 44, 4, ITALIA_WHITE);
        fill(f, 86, 12, 42, 4, ITALIA_RED);
    }
    if f.skin == SkinId::Chrome99 {
        line(f, 0, 0, 127, 0, WHITE);
    }
    small(f, "RGD", 4, 4, CYAN);
    small(f, ui.page.label(), 26, 4, WHITE);
    // One key letter + last digit of a press counter; advances even when a
    // selected row is already at the beginning/end.
    if let Some(action) = ui.last_action {
        let key = match action {
            Action::Up => "U",
            Action::Down => "D",
            Action::Left => "L",
            Action::Right => "R",
            Action::Click => "C",
            Action::Page(_) => "K",
        };
        let mut event: String<5> = String::new();
        let _ = write!(event, "{}{}", key, ui.input_count % 10);
        fill(f, 80, 2, 17, 11, BORDER);
        small(f, &event, 83, 4, WHITE);
    }
    badge(f, "DEMO", 98, 3, AMBER);
    circle(f, 122, 3, 4, if state.usb == UsbState::Disconnected { MUTED }
           else if state.usb == UsbState::DemoStale { RED } else { GREEN });
}

fn footer(f: &mut Frame<'_>, ui: &UiState, state: &RenderData) {
    // Separate verified transport activity from virtual chart data.
    if state.demo_sequence > 0 {
        let progress = (((state.demo_sequence - 1) % 100 + 1) * 128 / 100) as u32;
        fill(f, 0, 109, progress, 2, AMBER);
    } else {
        fill(f, 0, 109, 128, 1, BORDER);
    }
    for (i, tag) in ["1 LP","2 CH","3 IM","4 SY"].iter().enumerate() {
        let x = i as i32 * 32;
        let active = ui.page.index() == i;
        fill(f, x, 112, 31, 15, if active { CYAN } else { PANEL });
        small(f, tag, x + 3, 116, if active { BG } else { MUTED });
    }
}

fn render_somatic(f: &mut Frame<'_>, ui: &UiState) {
    let mut b: String<32> = String::new();
    if ui.mode == Mode::Focus {
        small(f, "FOCUS / GRAPH DEMO", 5, 20, AMBER);
        let _ = write!(b, "ZOOM x{}  PAN {}", ui.zoom_factor(), ui.pan);
        small(f, &b, 6, 33, WHITE);
        demo_chart(f, 5, 46, 118, 53, ui);
        small(f, "CLICK: BACK", 6, 101, MUTED);
        return;
    }
    small(f, "LOOP: NO REAL SOURCE", 5, 20, AMBER);
    big(f, "---", 8, 33, WHITE);
    big(f, "Hz", 48, 33, CYAN);
    small(f, "TARGET", 82, 34, MUTED);
    small(f, "1000", 87, 44, WHITE);
    let metric = ["HZ", "JITTER", "DURATION"][ui.selected() as usize];
    b.clear();
    let _ = write!(b, "{}  --  {}s", metric, ui.window_secs());
    small(f, &b, 6, 58, WHITE);
    small(f, "DEMO CHART", 67, 58, AMBER);
    demo_chart(f, 5, 69, 118, 35, ui);
}

fn sample_kind(idx: usize) -> &'static str {
    match idx { 0 => "OBS", 1 => "PLAN", 2 => "NOTE", _ => "ERR" }
}
fn sample_code(idx: usize) -> &'static str {
    match idx { 0 => "D-01", 1 => "D-02", 2 => "D-03", _ => "D-04" }
}
fn render_chronons(f: &mut Frame<'_>, ui: &UiState) {
    let filter = ui.window_index() as usize;
    if ui.mode == Mode::Focus {
        let i = if filter == 0 {ui.selected() as usize} else {filter - 1};
        small(f, "SYNTHETIC CHRONON", 5, 20, AMBER);
        big(f, sample_code(i), 7, 34, WHITE);
        small(f, sample_kind(i), 85, 38, CYAN);
        let lines = ["SOURCE: DEMO", "TIME: NOT SYNCED", "DATA: FIXTURE", "NO ROBOT EVENTS", "CLICK TO EXIT"];
        for row in 0..3 {
            let idx = (ui.detail_line as usize + row).min(lines.len() - 1);
            small(f, lines[idx], 6, 61 + row as i32 * 13, if row == 0 {WHITE} else {MUTED});
        }
        return;
    }

    let filters = ["ALL", "OBS", "PLAN", "NOTE", "ERR"];
    let mut buf: String<24> = String::new();
    let _ = write!(buf,"FIXTURE / {} ONLY", filters[filter]);
    small(f, &buf, 6, 20, AMBER);
    let row_height = 18;
    if filter != 0 {
        let i = filter - 1;
        fill(f, 4, 40, 120, 23, PANEL);
        stroke(f, 4, 40, 120, 23, CYAN);
        circle(f, 9, 49, 4, AMBER);
        small(f, sample_code(i), 20, 46, WHITE);
        small(f, sample_kind(i), 67, 46, CYAN);
        small(f, "DEMO", 98, 46, AMBER);
        small(f, "CLICK: DETAIL", 20, 82, MUTED);
    } else {
        for i in 0..4 {
            let y = 34 + i as i32 * row_height;
            let active = ui.selected() as usize == i;
            fill(f, 4, y, 120, 16, if active {BORDER} else {PANEL});
            if active {stroke(f, 4, y, 120, 16, CYAN);}
            circle(f, 10, y+6, 4, if active {CYAN} else {MUTED});
            small(f, sample_code(i), 22, y+4, WHITE);
            small(f, sample_kind(i), 64, y+4, if active {CYAN} else {MUTED});
            small(f, "DEMO", 99, y+4, AMBER);
        }
    }
}

fn render_imu(f: &mut Frame<'_>, ui: &UiState) {
    if ui.mode == Mode::Focus {
        small(f, "SENSOR / GRAPH DEMO", 5, 20, AMBER);
        let mut b: String<32> = String::new();
        let _=write!(b,"ZOOM x{}  PAN {}",ui.zoom_factor(),ui.pan);
        small(f,&b,6,32,WHITE);
        demo_chart(f,5,45,118,55,ui);
        small(f,"NOT CALIBRATED",6,101,AMBER);
        return;
    }

    small(f,"SENSOR XYZ / NO FEED",5,20,AMBER);
    small(f,"ROLL",6,34,MUTED);
    big(f,"--.-",36,30,WHITE);
    small(f,"PITCH",6,49,MUTED);
    big(f,"--.-",36,45,WHITE);
    stroke(f,97,29,25,31,BORDER);
    line(f,109,34,109,54,MUTED);
    line(f,101,44,118,44,MUTED);
    circle(f,107,42,5,AMBER);
    let metric = ["ROLL","PITCH","GYRO","ACCEL"][ui.selected() as usize];
    small(f,metric,5,65,CYAN);
    small(f,"--    DEMO GRAPH",35,65,AMBER);
    demo_chart(f,5,76,118,30,ui);
}

fn render_system(f: &mut Frame<'_>, ui: &UiState, info: &RenderData) {
    let rows = ["MOTORS", "CAN BUS", "POWER", "HOST USB"];
    let data = ["22 CFG / ON --", "NO BUS DATA", "DRIVE --.- V", "CDC STATUS"];
    let i = ui.selected() as usize;
    if ui.mode == Mode::Focus {
        small(f, "SYSTEM / INSPECT", 5, 20, CYAN);
        big(f, rows[i], 6, 34, WHITE);
        if i == 3 {
            big(f, if info.usb == UsbState::Disconnected {"OFF"} else {"LINK"}, 6, 53, GREEN);
        } else {
            big(f, match i {0=>"22 CFG",1=>"--",_=>"--.- V"}, 6, 53, CYAN);
        }
        small(f, if i==0 {"ONLINE: UNKNOWN"} else if i==1 {"CAN: UNOBSERVED"}
              else if i==2 {"VOLTAGE: NO SENSOR"} else {"USB ONLY / NO MOTOR"},5,76,AMBER);
        small(f,"L/R: NEXT  CLICK: BACK",5,95,MUTED);
        return;
    }

    for index in 0..4 {
        let y=21 + index as i32 * 22;
        let active = index==i;
        fill(f,4,y,120,20,if active {BORDER} else {PANEL});
        if active {stroke(f,4,y,120,20,CYAN);}
        small(f,rows[index],9,y+3,if active {WHITE} else {MUTED});
        if index==3 {
            small(f,if info.usb==UsbState::Disconnected {"USB OFF"} else {"USB ON"},
                9,y+11,if info.usb==UsbState::Disconnected {RED} else {GREEN});
        } else {
            small(f,data[index],9,y+11,AMBER);
        }
        // Segmented motor reference inventory is a reference, NOT an online test.
        if index==0 {
            for n in 0..11 {
                fill(f,74+n*4, y+3, 2, 3, MUTED);
            }
        }
    }
}

pub fn render(f: &mut Frame<'_>, ui: &UiState, info: &RenderData) {
    f.clear();
    top_bar(f,ui,info);
    match ui.page {
        Page::Somatic => render_somatic(f,ui),
        Page::Chronons => render_chronons(f,ui),
        Page::Imu => render_imu(f,ui),
        Page::System => render_system(f,ui,info),
    }
    footer(f,ui,info);
}

/// A short opening splash: informative only, no inference about robot readiness.
pub fn splash(f: &mut Frame<'_>) {
    f.clear();
    stroke(f,8,8,112,112,CYAN);
    fill(f,15,20,98,2,AMBER);
    big(f,"OPENRGD",28,40,WHITE);
    small(f,"PULSE / CHRONOSCOPE",10,66,CYAN);
    small(f,"OBSERVATION ONLY",19,84,AMBER);
    small(f,"NO MOTOR COMMANDS",18,101,MUTED);
}


/// Diagnostic page shows RAW pressed GPIO and the CURRENT SOFTWARE MAP
/// separately. It does not infer physical joystick orientation.
/// The operator moves each direction and notes whether the reported map
/// matches the real motion. Data stays entirely on the Pico (no USB TX).
fn input_action_name(raw: u8) -> &'static str {
    match landscape_action(raw as usize) {
        Some(Action::Page(Page::Somatic)) => "K1/LOOP",
        Some(Action::Page(Page::Chronons)) => "K2/TIME",
        Some(Action::Page(Page::Imu)) => "K3/IMU",
        Some(Action::Page(Page::System)) => "K4/SYS",
        Some(Action::Up) => "UP",
        Some(Action::Down) => "DOWN",
        Some(Action::Left) => "LEFT",
        Some(Action::Right) => "RIGHT",
        Some(Action::Click) => "CLICK",
        None => "UNKNOWN",
    }
}

pub fn render_input_lab(f: &mut Frame<'_>, lab: &InputLab) {
    f.clear();
    fill(f, 0, 0, 128, 17, PANEL);
    fill(f, 0, 16, 128, 1, CYAN);
    small(f, "RGD / INPUT", 5, 4, WHITE);
    small(f, "RAW GPIO", 83, 4, GREEN);

    small(f, "MOVE JOYSTICK / KEYS", 7, 21, MUTED);
    fill(f, 4, 33, 120, 55, PANEL);
    stroke(f, 4, 33, 120, 55, BORDER);
    fill(f, 64, 35, 1, 51, BORDER);
    small(f, "RAW PIN", 10, 38, CYAN);
    small(f, "FW MAP", 71, 38, AMBER);

    if let Some(raw) = lab.last_raw {
        let mut gpio: String<12> = String::new();
        if let Some(pin) = raw_gpio(raw) {
            let _ = write!(gpio, "GP{}", pin);
        } else {
            let _ = write!(gpio, "N/A");
        }
        big(f, &gpio, 9, 51, WHITE);
        big(f, input_action_name(raw), 71, 51, CYAN);
        small(
            f,
            if lab.last_held() { "HELD" } else { "RELEASED" },
            10,
            73,
            if lab.last_held() { GREEN } else { MUTED },
        );
        small(f, "CHECK DIR", 72, 75, AMBER);
    } else {
        big(f, "----", 9, 51, MUTED);
        big(f, "----", 71, 51, MUTED);
        small(f, "PRESS ONE AT A TIME", 11, 73, AMBER);
    }

    let mut count: String<24> = String::new();
    let _ = write!(count, "PRESSES: {}", lab.events);
    small(f, &count, 8, 95, WHITE);
    small(f, "LAST INPUT PERSISTS", 8, 104, MUTED);
    fill(f, 0, 114, 128, 14, PANEL);
    small(f, "HOLD K1+K4: EXIT", 8, 117, AMBER);
}


/// Winamp-inspired local skin picker. The preview is immediately visible;
/// CLICK applies for this power-on session, K2+K3 long hold cancels.
pub fn render_skin_settings(f: &mut Frame<'_>, settings: &Settings) {
    f.skin = settings.visual_skin();
    f.clear();
    fill(f, 0, 0, 128, 16, PANEL);
    fill(f, 0, 15, 128, 1, CYAN);
    small(f, "RGD / SETTINGS", 4, 4, WHITE);
    small(f, "SKINS", 6, 20, CYAN);
    let mut number: String<20> = String::new();
    let _ = write!(number, "{}/4", settings.preview.index() + 1);
    small(f, &number, 106, 20, MUTED);

    fill(f, 4, 32, 120, 48, PANEL);
    stroke(f, 4, 32, 120, 48, CYAN);
    if f.skin.manifest().style == ChromeStyle::Beveled {
        line(f, 6, 34, 121, 34, WHITE);
        line(f, 6, 35, 6, 76, MUTED);
    }
    big(f, settings.preview.manifest().label, 11, 38, WHITE);
    small(f, "PREVIEW", 12, 57, MUTED);
    match f.skin.manifest().style {
        ChromeStyle::Oscilloscope => {
            for i in 0..9 {
                let x = 12 + i * 12;
                let y = if i % 3 == 0 { 70 } else { 67 };
                line(f, x, y, x + 9, y, CYAN);
            }
        }
        ChromeStyle::Beveled => {
            fill(f, 12, 69, 94, 7, BORDER);
            line(f, 13, 69, 105, 69, WHITE);
        }
        ChromeStyle::Tricolore => {
            // Three large literal GREEN / WHITE / RED swatches.
            // If the right swatch appears BLUE, do not guess a different
            // palette: verify MADCTL RGB/BGR bit on the actual hardware.
            fill(f, 12, 66, 30, 12, ITALIA_GREEN);
            fill(f, 42, 66, 30, 12, ITALIA_WHITE);
            fill(f, 72, 66, 30, 12, ITALIA_RED);
        }
        ChromeStyle::Clean => {
            for i in 0..9 {
                let x = 12 + i * 12;
                line(f, x, 73, x + 7, 65 + (i % 3) * 3, CYAN);
            }
        }
    }

    small(f, "PREV < > NEXT", 8, 84, WHITE);
    small(f, "CLICK: APPLY", 8, 96, GREEN);
    small(f, "RAM ONLY / NO FLASH", 8, 105, AMBER);
    fill(f, 0, 114, 128, 14, PANEL);
    small(f, "K2+K3 HOLD: CANCEL", 7, 117, MUTED);
}
