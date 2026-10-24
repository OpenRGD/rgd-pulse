#![no_std]
#![no_main]

//! PULSE-0.2B — four-page, landscape Chronoscope UI.
//! USB CDC DEMO/ACK compatibility retained from PULSE-0.1C.1.
//! Read-only observer, never actuation, never a safety controller.

mod lcd;
mod render;

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::{
    bind_interrupts,
    gpio::{Input, Level, Output, Pull},
    peripherals::USB,
    spi::{Config as SpiConfig, Spi},
    usb::{Driver, InterruptHandler},
    Peripherals,
};
use embassy_time::{Instant, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    UsbDevice,
};
use lcd::Lcd;
use panic_halt as _;
use pulse_protocol::{encode, Frame, Kind, StreamDecoder};
use pulse_ui::{input_lab::InputLab, landscape_action, UiState};
use render::{Frame as Framebuffer, RenderData, UsbState, PIXELS};
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

const PROTOTYPE_BOARD_ID: u32 = 1;
const DEMO_STALE_AFTER_MS: u64 = 900;
const BUTTON_DEBOUNCE_MS: u64 = 140;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p: Peripherals = embassy_rp::init(Default::default());

    // Start USB before the LCD startup animation. Otherwise the earlier
    // blocking portrait-mode display boot could delay enumeration by seconds.
    // Prototype-only VID/PID, not a production hardware identity.
    let driver = Driver::new(p.USB, Irqs);
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("OpenRGD");
    config.product = Some("RGD Pulse Pico W prototype");
    config.serial_number = None;
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static CDC_STATE: StaticCell<State> = StaticCell::new();

    let mut builder = embassy_usb::Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0u8; 256]),
        BOS_DESCRIPTOR.init([0u8; 256]),
        &mut [],
        CONTROL_BUF.init([0u8; 64]),
    );
    let mut cdc = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);
    let device = builder.build();
    spawner.spawn(usb_task(device)).unwrap();

    let mut spi_config = SpiConfig::default();
    spi_config.frequency = 8_000_000;
    let spi = Spi::new_blocking_txonly(p.SPI0, p.PIN_6, p.PIN_7, spi_config);
    let mut lcd = Lcd::new(
        spi,
        Output::new(p.PIN_5, Level::High),
        Output::new(p.PIN_11, Level::High),
        Output::new(p.PIN_12, Level::High),
        Output::new(p.PIN_13, Level::Low),
    );

    // RAW scan index -> electrical pin is unchanged since 0.1B.
    // The separately tested pulse_ui::landscape_action() interprets these
    // in the FINAL physical orientation; reported on 2026-10-24:
    // GP18=UP, GP20=DOWN, GP16=LEFT, GP19=RIGHT, GP17=CLICK.
    // Keep raw scanning independent of the operator-facing mapping.
    let keys = [
        Input::new(p.PIN_8, Pull::Up),  // index 0: K1
        Input::new(p.PIN_4, Pull::Up),  // index 1: K2
        Input::new(p.PIN_3, Pull::Up),  // index 2: K3
        Input::new(p.PIN_2, Pull::Up),  // index 3: K4
        Input::new(p.PIN_16, Pull::Up), // index 4: observed LEFT
        Input::new(p.PIN_20, Pull::Up), // index 5: observed DOWN
        Input::new(p.PIN_17, Pull::Up), // index 6: observed CLICK
        Input::new(p.PIN_18, Pull::Up), // index 7: observed UP
        Input::new(p.PIN_19, Pull::Up), // index 8: observed RIGHT
    ];

    static FRAME: StaticCell<[u16; PIXELS]> = StaticCell::new();
    let backing = FRAME.init_with(|| [0u16; PIXELS]);
    let mut image = Framebuffer { data: backing };

    // Give the USB executor a turn before beginning non-critical LCD work.
    Timer::after_millis(10).await;
    lcd.init().await;
    render::splash(&mut image);
    lcd.flush_rgb565(image.data);
    Timer::after_millis(350).await;

    let mut ui = UiState::new(); // K1 SOMATIC default
    let mut input_lab = InputLab::new(); // Hold K1+K4 >=1.2s to toggle raw GPIO diagnostics
    let mut decoder = StreamDecoder::new();
    let mut rx_packet = [0u8; 64];
    let mut usb_connected = false;
    let mut last_demo_ms: Option<u64> = None;
    let mut last_demo_sequence: Option<u32> = None;
    let mut invalid_frames = 0u32;
    let mut previous_mask = 0u16;
    let mut last_key_ms = [0u64; 9];
    let mut old_usb_state = UsbState::Disconnected;
    let mut needs_draw = true;

    loop {
        // Embassy wait_connection() sees enabled endpoints, not host DTR.
        // Respect DTR to re-arm HELLO for every host open/close cycle.
        if !cdc.dtr() {
            if usb_connected {
                usb_connected = false;
                decoder.reset();
                last_demo_ms = None;
                last_demo_sequence = None;
                needs_draw = true;
            }
            Timer::after_millis(35).await;
        } else if !usb_connected {
            match select(cdc.wait_connection(), Timer::after_millis(55)).await {
                Either::First(()) => {
                    usb_connected = true;
                    decoder.reset();
                    last_demo_ms = None;
                    last_demo_sequence = None;
                    let hello = encode(Frame {
                        kind: Kind::Hello,
                        sequence: 1,
                        uptime_ms: Instant::now().as_millis() as u32,
                        value: PROTOTYPE_BOARD_ID,
                    });
                    if cdc.write_packet(&hello).await.is_err() {
                        usb_connected = false;
                    }
                    needs_draw = true;
                }
                Either::Second(()) => {}
            }
        } else {
            match select(cdc.read_packet(&mut rx_packet), Timer::after_millis(55)).await {
                Either::First(Ok(count)) => {
                    for byte in rx_packet[..count].iter().copied() {
                        match decoder.push(byte) {
                            Some(Ok(frame)) if frame.kind == Kind::DemoTicks => {
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
                                    // ACK only accepted CRC-checked, fresh synthetic
                                    // DEMO frames. Never echo commands/actuator data.
                                    let receipt = encode(Frame {
                                        kind: Kind::DemoAck,
                                        sequence: frame.sequence,
                                        uptime_ms: Instant::now().as_millis() as u32,
                                        value: frame.value,
                                    });
                                    if cdc.write_packet(&receipt).await.is_err() {
                                        usb_connected = false;
                                        decoder.reset();
                                        last_demo_ms = None;
                                        last_demo_sequence = None;
                                        break;
                                    }
                                    // Progress updates every ten frames, as
                                    // operator-verified in the 0.1C benchmark.
                                    if frame.sequence % 10 == 0 {
                                        needs_draw = true;
                                    }
                                }
                            }
                            Some(Err(_)) => {
                                invalid_frames = invalid_frames.saturating_add(1);
                                needs_draw = true;
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
                    needs_draw = true;
                }
                Either::Second(()) => {}
            }
        }

        let now_ms = Instant::now().as_millis();
        let new_usb_state = if !usb_connected {
            UsbState::Disconnected
        } else if let Some(last) = last_demo_ms {
            if now_ms.saturating_sub(last) <= DEMO_STALE_AFTER_MS {
                UsbState::DemoFresh
            } else {
                UsbState::DemoStale
            }
        } else {
            UsbState::Open
        };
        if new_usb_state != old_usb_state {
            old_usb_state = new_usb_state;
            needs_draw = true;
        }

        // Sample all nine raw GPIO inputs. Do not infer the physical axis
        // from navigation output: INPUT LAB shows pin identity vs software map
        // side by side. Hold K1+K4 for 1200ms to enter/leave the lab.
        let mut mask = 0u16;
        for (index, key) in keys.iter().enumerate() {
            if key.is_low() {
                mask |= 1 << index;
            }
        }
        let previously_in_lab = input_lab.active;
        needs_draw |= input_lab.observe(mask, now_ms);

        // Edge-triggered UI gestures retain the proven original semantics.
        // While INPUT LAB is active (or a toggle chord is held), no normal
        // screen navigation happens; raw joystick sampling still continues.
        let pressed = mask & !previous_mask;
        if !input_lab.active && !previously_in_lab && !input_lab.chord_held() {
            for index in 0..9 {
                if (pressed & (1 << index)) != 0
                    && (last_key_ms[index] == 0
                        || now_ms.saturating_sub(last_key_ms[index]) >= BUTTON_DEBOUNCE_MS)
                {
                    last_key_ms[index] = now_ms.max(1);
                    if let Some(action) = landscape_action(index) {
                        needs_draw |= ui.handle(action);
                    }
                }
            }
        }
        previous_mask = mask;

        if needs_draw {
            if input_lab.active {
                render::render_input_lab(&mut image, &input_lab);
            } else {
                let info = RenderData {
                    usb: new_usb_state,
                    demo_sequence: last_demo_sequence.unwrap_or(0),
                    invalid_frames,
                };
                render::render(&mut image, &ui, &info);
            }
            lcd.flush_rgb565(image.data);
            needs_draw = false;
        }
    }
}
