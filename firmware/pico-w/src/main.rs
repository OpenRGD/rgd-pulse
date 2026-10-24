#![no_std]
#![no_main]

//! Standalone LOCAL bring-up only. No USB telemetry, no motors and no power monitoring.
//! Slow bit-banged SPI0-compatible pins allow visual verification before optimizing DMA/SPI.

use cortex_m_rt::entry;
use embassy_rp::{
    gpio::{Input, Level, Output, Pull},
    Peripherals,
};
use panic_halt as _;

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const BLACK: u16 = 0x0000;
const BLUE: u16 = 0x045f;
const CYAN: u16 = 0x07ff;
const GREEN: u16 = 0x07e0;
const RED: u16 = 0xf800;
const WHITE: u16 = 0xffff;
const ORANGE: u16 = 0xfd20;

fn spin(count: u32) {
    cortex_m::asm::delay(count);
}

// Embassy's default RP2040 sysclock is around 125 MHz. The Cortex-M busy loop
// is deliberately conservative for the first hardware diagnostic. Replace it
// with a monotonic hardware-timer delay once LCD bring-up is verified.
// ST7735S requires >=120 ms after SLPOUT, unlike the previous ~32 ms hold.
const LCD_RESET_SETTLE: u32 = 25_000_000;
const LCD_SLEEP_EXIT_SETTLE: u32 = 25_000_000;
const BOOT_BLINK_HOLD: u32 = 25_000_000;

struct Lcd<'a> {
    sck: Output<'a>, mosi: Output<'a>, cs: Output<'a>,
    dc: Output<'a>, reset: Output<'a>, bl: Output<'a>,
}

impl<'a> Lcd<'a> {
    fn byte(&mut self, value: u8) {
        for bit in (0..8).rev() {
            self.sck.set_low();
            if ((value >> bit) & 1) != 0 { self.mosi.set_high(); }
            else { self.mosi.set_low(); }
            spin(20);
            self.sck.set_high();
            spin(20);
        }
        self.sck.set_low();
    }

    fn cmd(&mut self, value: u8) {
        self.cs.set_low();
        self.dc.set_low();
        self.byte(value);
        self.cs.set_high();
    }

    fn data(&mut self, bytes: &[u8]) {
        self.cs.set_low();
        self.dc.set_high();
        for &b in bytes { self.byte(b); }
        self.cs.set_high();
    }

    fn init(&mut self) {
        self.bl.set_low();
        self.cs.set_high();
        self.sck.set_low();
        self.reset.set_high();
        spin(2_000_000);
        self.reset.set_low();
        spin(2_000_000);
        self.reset.set_high();
        spin(LCD_RESET_SETTLE);
        self.cmd(0x01); // SWRESET
        spin(LCD_RESET_SETTLE);
        self.cmd(0x11); // SLPOUT
        spin(LCD_SLEEP_EXIT_SETTLE);
        self.cmd(0x3A); self.data(&[0x05]); // COLMOD RGB565
        self.cmd(0x36); self.data(&[0x00]); // MADCTL initial orientation
        self.cmd(0x21); // inversion ON (common ST7735S module)
        self.cmd(0x13); // NORON
        self.cmd(0x29); // DISPON
        self.bl.set_high();
    }

    fn window(&mut self, x0: u8, y0: u8, x1: u8, y1: u8) {
        // These offsets are provisional: revise after physical panel test.
        const X_OFFSET: u16 = 2;
        const Y_OFFSET: u16 = 3;
        let x0 = x0 as u16 + X_OFFSET;
        let x1 = x1 as u16 + X_OFFSET;
        let y0 = y0 as u16 + Y_OFFSET;
        let y1 = y1 as u16 + Y_OFFSET;
        self.cmd(0x2A); self.data(&[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8]);
        self.cmd(0x2B); self.data(&[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8]);
        self.cmd(0x2C);
    }

    fn rect(&mut self, x: u8, y: u8, w: u8, h: u8, color: u16) {
        if w == 0 || h == 0 || x >= 128 || y >= 128 { return; }
        let x1 = ((x as u16 + w as u16).min(128) - 1) as u8;
        let y1 = ((y as u16 + h as u16).min(128) - 1) as u8;
        self.window(x, y, x1, y1);
        self.cs.set_low();
        self.dc.set_high();
        for _ in 0..((x1 as u32 - x as u32 + 1) * (y1 as u32 - y as u32 + 1)) {
            self.byte((color >> 8) as u8);
            self.byte(color as u8);
        }
        self.cs.set_high();
    }
}

#[entry]
fn main() -> ! {
    let p: Peripherals = embassy_rp::init(Default::default());
    let mut lcd = Lcd {
        sck: Output::new(p.PIN_6, Level::Low),
        mosi: Output::new(p.PIN_7, Level::Low),
        cs: Output::new(p.PIN_5, Level::High),
        dc: Output::new(p.PIN_11, Level::High),
        reset: Output::new(p.PIN_12, Level::High),
        bl: Output::new(p.PIN_13, Level::Low),
    };
    // All keys are input pull-up; read-only. Their active-low polarity is provisional.
    let keys = [
        Input::new(p.PIN_8, Pull::Up), Input::new(p.PIN_4, Pull::Up),
        Input::new(p.PIN_3, Pull::Up), Input::new(p.PIN_2, Pull::Up),
        Input::new(p.PIN_16, Pull::Up), Input::new(p.PIN_20, Pull::Up),
        Input::new(p.PIN_17, Pull::Up), Input::new(p.PIN_18, Pull::Up),
        Input::new(p.PIN_19, Pull::Up),
    ];

    // First independent boot diagnostic: three visible backlight pulses.
    // This does not require working SPI or ST7735S initialization.
    // If no pulses can be seen, check that this firmware actually boots and
    // that GP13 can drive BL before investigating the LCD graphics.
    for _ in 0..3 {
        lcd.bl.set_high();
        spin(BOOT_BLINK_HOLD);
        lcd.bl.set_low();
        spin(BOOT_BLINK_HOLD);
    }

    lcd.init();
    lcd.rect(0, 0, 128, 128, BLACK);
    lcd.rect(0, 0, 128, 14, BLUE);
    lcd.rect(4, 18, 120, 4, CYAN);
    lcd.rect(4, 28, 120, 7, GREEN); // self-test marker
    lcd.rect(0, 113, 128, 15, BLUE);

    let mut prior_mask = u16::MAX;
    let mut blink = false;
    loop {
        let mut mask: u16 = 0;
        for (index, key) in keys.iter().enumerate() {
            if key.is_low() { mask |= 1 << index; }
        }
        if mask != prior_mask {
            // 9 on-screen squares: K1..K4 then left/down/right/center/up.
            for index in 0..9 {
                let active = (mask & (1 << index)) != 0;
                let x = 4 + (index % 3) * 40;
                let y = 43 + (index / 3) * 20;
                lcd.rect(x as u8, y as u8, 34, 14, if active { ORANGE } else { WHITE });
            }
            prior_mask = mask;
        }
        blink = !blink;
        lcd.rect(118, 115, 6, 6, if blink { GREEN } else { RED });
        spin(1_000_000);
    }
}
