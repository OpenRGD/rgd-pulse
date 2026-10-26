#![no_std]
#![no_main]

//! PULSE-0.2B — four-page, landscape Chronoscope UI.
//! USB CDC DEMO/ACK compatibility retained from PULSE-0.1C.1.
//! Read-only observer, never actuation, never a safety controller.

mod lcd;
mod preferences_store;
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
use pulse_protocol::{
    encode,
    imu_wire::{self, Decoder as ImuDecoder, Frame as ImuFrame},
    Frame, Kind, StreamDecoder,
};
use pulse_ui::{
    color_lab::{ColorLab, VALIDATED_BOOT_MODE},
    preferences::Preferences,
    imu::{FrameQuality, ImuSample, ImuStatus, ImuView, Origin},
    input_lab::InputLab,
    input_repeat::InputRepeater,
    landscape_action,
    settings::Settings,
    skins::SkinId,
    UiState,
};
use preferences_store::PreferencesStore;
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
    let mut image = Framebuffer {
        data: backing,
        skin: SkinId::Classic,
    };

    // Give the USB executor a turn before beginning non-critical LCD work.
    Timer::after_millis(10).await;
    lcd.init().await;
    render::splash(&mut image);
    lcd.flush_rgb565(image.data);
    Timer::after_millis(350).await;

    // Read the two reserved flash sectors before selecting the initial K page.
    // Corrupt/unknown records fall back to Classic + K1. This is read-only.
    let mut preference_store = PreferencesStore::new(p.FLASH);
    let restored_choices = preference_store.restored();
    let mut ui = UiState::new();
    ui.page = restored_choices.page; // K1 default or last persisted K page
    let mut input_lab = InputLab::new(); // K1+K4 >=1.2s: raw GPIO diagnostics
    let mut settings = Settings::new(); // K2+K3 >=1.2s: Winamp-style skin picker
    settings.applied = restored_choices.skin;
    settings.preview = restored_choices.skin;
    let mut color_lab = ColorLab::new(); // K1+K2 >=1.2s: LCD RGB/BGR x INV calibration
                                         // Track the exact mode that lcd.init() applied before the splash.
    let mut lcd_color_mode = VALIDATED_BOOT_MODE;
    let mut repeater = InputRepeater::new(); // Direction keys only, never CLICK/K1-K4
    let mut decoder = StreamDecoder::new(); // Existing PULSE/1 frames, unchanged
    let mut imu_decoder = ImuDecoder::new(); // Separate PULSE/IMU-1 read-only channel
    let mut imu_negotiated = false; // Must receive explicit Probe before real frames
    let mut imu_view = ImuView::new();
    let mut last_imu_status = ImuStatus::NoSource;
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
                imu_decoder.reset();
                imu_negotiated = false;
                imu_view.clear();
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
                    imu_decoder.reset();
                    imu_negotiated = false;
                    imu_view.clear();
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
                                        imu_decoder.reset();
                                        imu_negotiated = false;
                                        imu_view.clear();
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

                        // IMU wire is entirely separate from legacy PULSE/1
                        // DEMO handling. The Pico never treats DemoTicks as
                        // sensor data, and never accepts an IMU sample before
                        // an explicit per-CDC-session capability probe.
                        if let Some(Ok(packet)) = imu_decoder.push(byte) {
                            match packet {
                                ImuFrame::Probe => {
                                    let response = imu_wire::encode(ImuFrame::ProbeAck);
                                    if cdc.write_packet(&response).await.is_ok() {
                                        imu_negotiated = true;
                                    } else {
                                        usb_connected = false;
                                        imu_negotiated = false;
                                        imu_view.clear();
                                        break;
                                    }
                                }
                                ImuFrame::SensorSample(sample) if imu_negotiated => {
                                    let observation = ImuSample {
                                        origin: Origin::ConsoleReadOnlyObservation,
                                        frame_quality: FrameQuality::SensorAxesUnverified,
                                        source_session: sample.session,
                                        source_sequence: sample.sequence,
                                        roll_cdeg: sample.angles_cdeg[0],
                                        pitch_cdeg: sample.angles_cdeg[1],
                                        yaw_cdeg: sample.angles_cdeg[2],
                                        gyro_mrad_s: [
                                            sample.gyro_centirad_s[0] as i32 * 10,
                                            sample.gyro_centirad_s[1] as i32 * 10,
                                            sample.gyro_centirad_s[2] as i32 * 10,
                                        ],
                                        accel_mm_s2: [
                                            sample.accel_centims2[0] as i32 * 10,
                                            sample.accel_centims2[1] as i32 * 10,
                                            sample.accel_centims2[2] as i32 * 10,
                                        ],
                                        temp_cdeg: sample.temperature_cdeg,
                                        host_age_at_forward_ms: Some(sample.age_at_host_forward_ms),
                                    };
                                    if imu_view
                                        .observe(observation, Instant::now().as_millis())
                                        .is_ok()
                                    {
                                        // Redraw physical IMU readings only on K3;
                                        // avoid 10fps 32KiB SPI flushes on unrelated
                                        // pages while receiving the same USB stream.
                                        needs_draw |= ui.page == pulse_ui::Page::Imu;
                                    }
                                }
                                ImuFrame::SensorSample(_) | ImuFrame::ProbeAck => {
                                    // Ignore unsolicited sensor/ACK frames.
                                }
                            }
                        }
                    }
                }
                Either::First(Err(_)) => {
                    usb_connected = false;
                    last_demo_ms = None;
                    last_demo_sequence = None;
                    decoder.reset();
                    imu_decoder.reset();
                    imu_negotiated = false;
                    imu_view.clear();
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

        // Mark LIVE -> STALE even when the publisher stops and no more USB
        // packets arrive. Pico time is compared only with its own receipt;
        // the host computes source age in its own CLOCK_MONOTONIC domain.
        let new_imu_status = imu_view.status(now_ms);
        if new_imu_status != last_imu_status {
            last_imu_status = new_imu_status;
            if ui.page == pulse_ui::Page::Imu {
                needs_draw = true;
            }
        }

        // Sample the physically accepted GPIO mapping once. Three disjoint,
        // mutually exclusive *local* hold chords: K1+K4 INPUT, K2+K3 SKINS,
        // K1+K2 COLOR. A long hold never communicates a robot command.
        let mut mask = 0u16;
        for (index, key) in keys.iter().enumerate() {
            if key.is_low() {
                mask |= 1 << index;
            }
        }

        let was_in_lab = input_lab.active;
        let was_in_settings = settings.open;
        let was_in_color = color_lab.open;

        // Disabled overlays see no raw input, which resets chord timers and
        // prevents a held key from arming a second mode on exit.
        if !settings.open && !color_lab.open {
            needs_draw |= input_lab.observe(mask, now_ms);
        } else {
            needs_draw |= input_lab.observe(0, now_ms);
        }
        if !input_lab.active && !color_lab.open {
            needs_draw |= settings.observe(mask, now_ms, true);
        } else {
            needs_draw |= settings.observe(0, now_ms, false);
        }
        if !input_lab.active && !settings.open {
            needs_draw |= color_lab.observe(mask, now_ms, true);
        } else {
            needs_draw |= color_lab.observe(0, now_ms, false);
        }

        let overlay_switched = was_in_lab != input_lab.active
            || was_in_settings != settings.open
            || was_in_color != color_lab.open;
        if overlay_switched {
            repeater.reset();
            needs_draw = true;
        }

        let pressed = mask & !previous_mask;
        let repeated = if input_lab.active {
            repeater.reset();
            0u16
        } else {
            repeater.poll(mask, now_ms)
        };
        let chord_blocked =
            input_lab.chord_held() || settings.chord_held(mask) || color_lab.chord_held(mask);

        if !input_lab.active && !was_in_lab && !overlay_switched && !chord_blocked {
            let events = pressed | repeated;
            for index in 0..9 {
                if (events & (1 << index)) != 0 {
                    let auto_repeat = (repeated & (1 << index)) != 0;
                    let debounce_ok = auto_repeat
                        || last_key_ms[index] == 0
                        || now_ms.saturating_sub(last_key_ms[index]) >= BUTTON_DEBOUNCE_MS;
                    if debounce_ok {
                        last_key_ms[index] = now_ms.max(1);
                        if let Some(action) = landscape_action(index) {
                            if color_lab.open {
                                needs_draw |= color_lab.handle(action);
                            } else if settings.open {
                                needs_draw |= settings.handle(action);
                            } else {
                                needs_draw |= ui.handle(action);
                            }
                        }
                    }
                }
            }
        }
        previous_mask = mask;

        // Persist only APPLIED skin and selected K-page, never menu previews,
        // focus/zoom, synthetic telemetry or any sensor observation. A stable
        // choice is committed at most once per debounce window and only
        // when no USB CDC/DTR session is active.
        preference_store.observe(Preferences::new(settings.applied, ui.page), now_ms);
        preference_store.poll(now_ms, cdc.dtr());

        if needs_draw {
            // LCD command 0x36 (MADCTL R/B order) and 0x20/0x21
            // (INVOFF/INVON) are applied only when the preview changes.
            // No flash writes, no CPU reset and no change to orientation.
            let desired_color_mode = color_lab.visual_mode();
            if desired_color_mode != lcd_color_mode {
                lcd.apply_color_mode(desired_color_mode);
                lcd_color_mode = desired_color_mode;
            }

            if color_lab.open {
                image.skin = SkinId::Classic;
                render::render_color_lab(&mut image, &color_lab);
            } else if input_lab.active {
                image.skin = SkinId::Classic;
                render::render_input_lab(&mut image, &input_lab);
            } else if settings.open {
                image.skin = settings.visual_skin();
                render::render_skin_settings(&mut image, &settings);
            } else {
                image.skin = settings.applied;
                let info = RenderData {
                    usb: new_usb_state,
                    demo_sequence: last_demo_sequence.unwrap_or(0),
                    invalid_frames,
                };
                render::render(&mut image, &ui, &info, &imu_view, now_ms);
            }
            lcd.flush_rgb565(image.data);
            needs_draw = false;
        }
    }
}
