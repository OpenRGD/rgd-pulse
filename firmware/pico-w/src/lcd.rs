//! ST7735S Rev 1.2: 128x128 RGB565, SPI0 GP6 SCK / GP7 MOSI.
//! No device power, CAN or actuation commands are supported here.

use embassy_rp::{
    gpio::Output,
    peripherals::SPI0,
    spi::{Blocking, Spi},
};
use embassy_time::Timer;

const LCD_W: u16 = 128;
const LCD_H: u16 = 128;

// Panel RAM offsets measured on original portrait fixture are *provisional*
// after rotation. Verify the edges, RGB/BGR ordering and text orientation
// on the physical joystick-left / keys-right unit.
const X_OFFSET: u16 = 2;
const Y_OFFSET: u16 = 3;
// Operator 2026-10-24: 0xA0 (MY|MV) produced an upside-down UI while
// joystick was at left and K1–K4 at right. The opposite landscape
// orientation 0x60 (MX|MV) flips the displayed image by 180 degrees,
// without changing the page controls or the framebuffer coordinates.
// Verify on the physical ST7735S; X/Y RAM offsets remain provisional.
const MADCTL_LANDSCAPE_CANDIDATE: u8 = 0x60; // MX | MV

pub struct Lcd<'a> {
    spi: Spi<'a, SPI0, Blocking>,
    cs: Output<'a>,
    dc: Output<'a>,
    reset: Output<'a>,
    bl: Output<'a>,
}

impl<'a> Lcd<'a> {
    pub fn new(
        spi: Spi<'a, SPI0, Blocking>,
        cs: Output<'a>,
        dc: Output<'a>,
        reset: Output<'a>,
        bl: Output<'a>,
    ) -> Self {
        Self { spi, cs, dc, reset, bl }
    }

    fn cmd(&mut self, value: u8) {
        self.cs.set_low();
        self.dc.set_low();
        let _ = self.spi.blocking_write(&[value]);
        self.cs.set_high();
    }

    fn data(&mut self, bytes: &[u8]) {
        self.cs.set_low();
        self.dc.set_high();
        let _ = self.spi.blocking_write(bytes);
        self.cs.set_high();
    }

    pub async fn init(&mut self) {
        self.bl.set_low();
        self.cs.set_high();

        // Async waits allow USB enumeration while the TFT powers up.
        self.reset.set_high();
        Timer::after_millis(10).await;
        self.reset.set_low();
        Timer::after_millis(15).await;
        self.reset.set_high();
        Timer::after_millis(130).await;

        self.cmd(0x01); // software reset
        Timer::after_millis(140).await;
        self.cmd(0x11); // exit sleep, ST7735S needs >=120ms
        Timer::after_millis(140).await;
        self.cmd(0x3A); self.data(&[0x05]); // 16-bit RGB565
        self.cmd(0x36); self.data(&[MADCTL_LANDSCAPE_CANDIDATE]);
        self.cmd(0x21); // inversion on for this panel candidate
        self.cmd(0x13); // normal display mode
        self.cmd(0x29); // display on

        // Retain a recognizable physical startup marker without busy waiting.
        for _ in 0..3 {
            self.bl.set_high();
            Timer::after_millis(35).await;
            self.bl.set_low();
            Timer::after_millis(35).await;
        }
        self.bl.set_high();
    }

    fn window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let (x0, x1) = (x0 + X_OFFSET, x1 + X_OFFSET);
        let (y0, y1) = (y0 + Y_OFFSET, y1 + Y_OFFSET);
        self.cmd(0x2A);
        self.data(&[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8]);
        self.cmd(0x2B);
        self.data(&[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8]);
        self.cmd(0x2C);
    }

    /// Flush one bounded 128x128 screen. ~32KiB transmitted at 8MHz, rather
    /// than bit-banging every individual pixel. No unbounded LCD operations.
    pub fn flush_rgb565(&mut self, pixels: &[u16]) {
        if pixels.len() != (LCD_W as usize * LCD_H as usize) {
            return;
        }
        self.window(0, 0, LCD_W - 1, LCD_H - 1);
        self.cs.set_low();
        self.dc.set_high();
        let mut chunk = [0u8; 256];
        for row in pixels.chunks_exact(128) {
            for (i, px) in row.iter().enumerate() {
                chunk[i * 2] = (px >> 8) as u8;
                chunk[i * 2 + 1] = *px as u8;
            }
            let _ = self.spi.blocking_write(&chunk);
        }
        self.cs.set_high();
    }
}
