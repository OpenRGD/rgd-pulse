#![no_std]
#![no_main]

//! PULSE-0.1C bench-only USB CDC + previously verified ST7735S display.
//! Read-only diagnostic consumer. No motors, no CAN, no calibration, no E-stop.

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::{
    bind_interrupts,
    gpio::{Input, Level, Output, Pull},
    peripherals::USB,
    usb::{Driver, InterruptHandler},
    Peripherals,
};
use embassy_time::{Instant, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    UsbDevice,
};
use panic_halt as _;
use pulse_protocol::{encode, Frame, Kind, StreamDecoder};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type MyUsbDriver = Driver<'static, USB>;
type MyUsbDevice = UsbDevice<'static, MyUsbDriver>;

#[embassy_executor::task]
async fn usb_task(mut usb: MyUsbDevice) -> ! {
    usb.run().await
}

// Prototype-only identifier. Not a registered production VID/PID.
// Do not enable automatic actuator access based on the USB identity.
const PROTOTYPE_BOARD_ID: u32 = 1;
const DEMO_STALE_AFTER_MS: u64 = 900;

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

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p: Peripherals = embassy_rp::init(Default::default());

    // Keep the existing proven 9 inputs and bit-banged LCD driver untouched.
    let mut lcd = Lcd {
        sck: Output::new(p.PIN_6, Level::Low),
        mosi: Output::new(p.PIN_7, Level::Low),
        cs: Output::new(p.PIN_5, Level::High),
        dc: Output::new(p.PIN_11, Level::High),
        reset: Output::new(p.PIN_12, Level::High),
        bl: Output::new(p.PIN_13, Level::Low),
    };
    let keys = [
        Input::new(p.PIN_8, Pull::Up),
        Input::new(p.PIN_4, Pull::Up),
        Input::new(p.PIN_3, Pull::Up),
        Input::new(p.PIN_2, Pull::Up),
        Input::new(p.PIN_16, Pull::Up),
        Input::new(p.PIN_20, Pull::Up),
        Input::new(p.PIN_17, Pull::Up),
        Input::new(p.PIN_18, Pull::Up),
        Input::new(p.PIN_19, Pull::Up),
    ];

    // Independent hardware boot marker: three visible backlight pulses.
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
    lcd.rect(4, 28, 120, 7, GREEN);
    lcd.rect(0, 113, 128, 15, BLUE);

    // CDC ACM gives a host-visible serial port (e.g. /dev/ttyACM*).
    // The board ID is a prototype type, not a unique device serial number.
    let driver = Driver::new(p.USB, Irqs);
    let mut usb_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_config.manufacturer = Some("OpenRGD");
    usb_config.product = Some("RGD Pulse Pico W prototype");
    usb_config.serial_number = None;
    usb_config.max_power = 100;
    usb_config.max_packet_size_0 = 64;

    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static CDC_STATE: StaticCell<State> = StaticCell::new();

    let mut builder = embassy_usb::Builder::new(
        driver,
        usb_config,
        CONFIG_DESCRIPTOR.init([0u8; 256]),
        BOS_DESCRIPTOR.init([0u8; 256]),
        &mut [],
        CONTROL_BUF.init([0u8; 64]),
    );
    let mut cdc = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);
    let device = builder.build();
    spawner.spawn(usb_task(device)).unwrap();

    let mut decoder = StreamDecoder::new();
    let mut rx_packet = [0u8; 64];
    let mut usb_connected = false;
    let mut last_demo_ms: Option<u64> = None;
    let mut last_demo_sequence: Option<u32> = None;
    let mut invalid_frames: u32 = 0;
    let mut prior_mask: u16 = u16::MAX;
    let mut last_state: u8 = 255;
    let mut indicator = false;

    loop {
        // The CDC bulk endpoints remain enabled after the host closes the
        // serial port. Embassy wait_connection() only checks endpoint state,
        // NOT host DTR; it cannot detect close/reopen by itself.
        //
        // DTR gives each host serial session a new HELLO and clean decoder.
        // A bounded timer keeps all nine buttons and LCD rendering alive
        // even when the serial port is not open.
        if !cdc.dtr() {
            if usb_connected {
                usb_connected = false;
                decoder.reset();
                last_demo_ms = None;
                last_demo_sequence = None;
            }
            Timer::after_millis(75).await;
        } else if !usb_connected {
            match select(cdc.wait_connection(), Timer::after_millis(120)).await {
                Either::First(()) => {
                    usb_connected = true;
                    decoder.reset();
                    last_demo_ms = None;
                    last_demo_sequence = None;
                    let greeting = encode(Frame {
                        kind: Kind::Hello,
                        sequence: 1,
                        uptime_ms: Instant::now().as_millis() as u32,
                        value: PROTOTYPE_BOARD_ID,
                    });
                    if cdc.write_packet(&greeting).await.is_err() {
                        usb_connected = false;
                    }
                }
                Either::Second(()) => {}
            }
        } else {
            match select(cdc.read_packet(&mut rx_packet), Timer::after_millis(120)).await {
                Either::First(Ok(count)) => {
                    for byte in rx_packet[..count].iter().copied() {
                        match decoder.push(byte) {
                            Some(Ok(frame)) if frame.kind == Kind::DemoTicks => {
                                // Only explicit synthetic demo frames update the
                                // demo status; real Somatic integration is future work.
                                let newer = match last_demo_sequence {
                                    None => true,
                                    Some(previous) => {
                                        let diff = frame.sequence.wrapping_sub(previous);
                                        diff > 0 && diff < 0x8000_0000
                                    }
                                };
                                if newer {
                                    last_demo_sequence = Some(frame.sequence);
                                    last_demo_ms = Some(Instant::now().as_millis());

                                    // Explicit acknowledgement to the host:
                                    // only accepted, CRC-checked, fresh DEMO
                                    // frames receive DemoAck. Never ACK motors,
                                    // real Somatic telemetry, or unknown kinds.
                                    let receipt = encode(Frame {
                                        kind: Kind::DemoAck,
                                        sequence: frame.sequence,
                                        uptime_ms: Instant::now().as_millis() as u32,
                                        value: frame.value,
                                    });
                                    if cdc.write_packet(&receipt).await.is_err() {
                                        usb_connected = false;
                                        last_demo_ms = None;
                                        last_demo_sequence = None;
                                        decoder.reset();
                                        break;
                                    }

                                    // A monochrome progress bar makes received
                                    // frames evident despite uncalibrated LCD colors.
                                    // Update once per 10 frames to bound display I/O.
                                    if frame.sequence % 10 == 0 {
                                        let steps = (((frame.sequence - 1) % 100) / 10 + 1) as u8;
                                        lcd.rect(4, 102, 120, 5, BLACK);
                                        lcd.rect(4, 102, steps * 12, 5, WHITE);
                                    }
                                }
                            }
                            Some(Err(_)) => {
                                invalid_frames = invalid_frames.saturating_add(1);
                            }
                            _ => {}
                        }
                    }
                }
                Either::First(Err(_)) => {
                    usb_connected = false;
                    last_demo_ms = None;
                    last_demo_sequence = None;
                    decoder.reset();
                }
                Either::Second(()) => {}
            }
        }

        let now = Instant::now().as_millis();
        // Status colors: BLUE=USB not opened, CYAN=CDC connected but no demo,
        // GREEN=fresh synthetic frames, RED=demo stream stale.
        let state: u8 = if !usb_connected {
            0
        } else if let Some(last) = last_demo_ms {
            if now.saturating_sub(last) <= DEMO_STALE_AFTER_MS {
                2
            } else {
                3
            }
        } else {
            1
        };
        if state != last_state {
            lcd.rect(
                0,
                0,
                128,
                14,
                match state {
                    0 => BLUE,
                    1 => CYAN,
                    2 => GREEN,
                    _ => RED,
                },
            );
            // Explicit visual marker that this is a DEMO diagnostic, never
            // an actual measured loop rate.
            lcd.rect(4, 28, 120, 7, if state == 2 { ORANGE } else { BLACK });
            last_state = state;
        }

        let mut mask: u16 = 0;
        for (index, key) in keys.iter().enumerate() {
            if key.is_low() {
                mask |= 1 << index;
            }
        }
        if mask != prior_mask {
            for index in 0..9 {
                let active = (mask & (1 << index)) != 0;
                let x = 4 + (index % 3) * 40;
                let y = 43 + (index / 3) * 20;
                lcd.rect(x as u8, y as u8, 34, 14, if active { ORANGE } else { WHITE });
            }
            prior_mask = mask;
        }

        // Independent visible activity marker (does NOT certify a real-time loop).
        indicator = !indicator;
        let color = if invalid_frames > 0 && indicator {
            ORANGE
        } else if indicator {
            GREEN
        } else {
            RED
        };
        lcd.rect(118, 115, 6, 6, color);
    }
}
