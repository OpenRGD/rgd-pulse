# Pico W / Seengreat 1.44 Rev 1.2 bring-up (candidate)

The initial target is **visible colored stripes and 9 buttons/joystick squares**, not full Chronoscope. No real robot communication or motor functionality exists.

## Requirements

Rust stable, `thumbv6m-none-eabi` target, `elf2uf2-rs` (or another RP2040 ELF -> UF2 conversion tool). This code has **not been cross-compiled or flashed in the author's environment**; compiler/API and physical offset corrections may be required.

```sh
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs
cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Power only by Pico **Micro-USB**, completely disconnected from robot power. Hold **BOOTSEL on Pico**, connect Micro-USB to computer, confirm drive `RPI-RP2`, copy `pulse-pico-w.uf2` to the drive. The Pico should reboot automatically. BOOTSEL is **not** any of the LCD K1-K4 buttons.

Display should show blue bands, green strip, white squares (orange when pressed) and a blinking indicator. It currently has no text renderer; fonts/navigation/USB CDC come later.

If screen stays blank: unplug, reconfirm Pico position/orientation, then check LCD offsets, backlight polarity and panel initialization before reflashing. **Do not apply 12V/24V.**

Pin mapping: [board profile](../../hardware/boards/seengreat-pico-144-rev12.md). This demonstration deliberately bit-bangs SPI pins at low speed for first bring-up only; production code will use hardware SPI DMA.

## No safety claims

No E-stop, motion safety, bus wiring, or motor calibration. No independent power buffer or black-box persistence.
