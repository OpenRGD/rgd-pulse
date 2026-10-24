# Pico W + Seengreat ST7735S firmware

**PULSE-0.1C candidate** — read-only synthetic USB CDC diagnostics. Display + all nine physical inputs were operator-tested successfully on 2026-10-24; USB is *not yet physically verified*. No motors, CAN, actuation, homing, calibration or E-stop are controlled here.

## Compile

```bash
cd ~/openrgd/rgd-pulse
git fetch origin
git switch feat/pulse-0.1c-usb-cdc
git pull --ff-only
cd firmware/pico-w

rustup target add thumbv6m-none-eabi
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Compiled for Raspberry Pi Pico W (RP2040), Seengreat PICO-1.44inch-LCD-Display Rev1.2. The RP2040 BOOT2 location is validated in CI.

## Flash / USB isolation

Power **only** via the Pico's Micro-USB socket from a real USB data hub, disconnected from all Paky-01 high-power electronics. Use BOOTSEL on the Pico (not K1–K4). Before copying UF2, confirm `RPI-RP2` is the Pico bootloader volume; another STM32 programmer on the mini-PC also exposes a removable drive.

On reboot the LCD keeps its previously tested diagnostic grid, three backlight pulses and all inputs; later a CDC serial port should appear.

## USB states on the LCD

| Top stripe | Meaning |
|---|---|
| Blue | CDC not opened |
| Cyan | CDC session opened, no valid synthetic `DemoTicks` |
| Green | Recent valid synthetic frames |
| Red | Synthetic stream stale for >900ms |

Orange thin stripe is a **DEMO-only** indicator, never real loop rate. RGB/BGR color order and panel offsets remain hardware candidates.

After locating the *exact* new USB CDC path with `udevadm info`, run:

```bash
cd ~/openrgd/rgd-pulse
cargo run -p pulse-bridge -- serial-demo /dev/serial/by-path/YOUR_REAL_PICO_PATH 20
```

No data is sent before a valid Pico `HELLO`. The device accepts only framed `DemoTicks` in this wave. Bad CRC and replayed/old sequence numbers cannot refresh the demo status. Buttons remain purely local read-only inputs, not USB commands.

See [the complete bench guide](../../docs/PULSE-0.1C-USB-BENCH.md) for discovery commands, limitations, and acceptance checklist.

## Implementation limits

The initial LCD transport still bit-bangs SPI for the first hardware proof, which briefly blocks the MCU's async executor during rectangle updates. The USB device stack runs as a separate Embassy task but is **not hard real time**. Production requires hardware SPI, bounded rendering, a unique USB device ID and independent timing characterization.

The command protocol is not authenticated, and CRC16 detects accidental corruption only. Nothing in this demo provides a robot safety function.
