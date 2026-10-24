# Pico W / Seengreat 1.44 Rev 1.2 bring-up (candidate)

The initial target is **visible colored stripes and 9 buttons/joystick squares**, not full Chronoscope. No real robot communication or motor functionality exists.

## Requirements

Rust stable, `thumbv6m-none-eabi` target, `elf2uf2-rs` (or another RP2040 ELF -> UF2 conversion tool). CI cross-compiles and links the RP2040 ELF; **the Seengreat Rev 1.2 display and button behavior have not yet been physically validated**. Timing, backlight polarity and display offsets remain hardware-test candidates.

```sh
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs
cd firmware/pico-w
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/pulse-pico-w pulse-pico-w.uf2
```

Power only by Pico **Micro-USB**, completely disconnected from robot power. Hold **BOOTSEL on Pico**, connect Micro-USB to computer, confirm drive `RPI-RP2`, copy `pulse-pico-w.uf2` to the drive. The Pico should reboot automatically. BOOTSEL is **not** any of the LCD K1-K4 buttons.

The firmware first **pulses the backlight three times** (standalone execution marker; no LCD commands required), then initializes ST7735S and draws blue bands, a green strip, white input squares (orange when pressed) and a blinking indicator.

### White screen / PULSE-0.1B.1 triage

1. Use a powered **USB data** hub connected to the mini-PC; Pico only via Micro-USB, no robot power connections. Unplug before moving the Pico between hubs.
2. Use the BOOTSEL button on the **Pico** to expose **RPI-RP2**; confirm the correct FAT volume before flashing (other boards, e.g. STM32, may also expose drives).
3. Rebuild **after** `git pull --ff-only`, and recreate the UF2 so you do not unknowingly reflash the previous image.
4. On reboot: **three obvious backlight pulses?** If yes, the RP2040 runs and GP13/backlight is responsive. If no, next verify the UF2/boot2 execution, Pico position, and GP13/backlight circuit; white alone does not prove execution.
5. If pulses occur **but stripes/squares do not**, the next area is SPI command initialization, CS/DC/RESET contact, panel variant and pixel addressing. Buttons cannot give visual feedback without a working LCD.
6. If the screen shows stripes, verify nine inputs individually; do not assume button failure from a white LCD.

The original firmware held after SLPOUT for only ~32 ms; this revision uses a conservative >120 ms delay for the ST7735S sleep-exit stabilization. Busy-loop timing is approximate (default RP2040 clocks), and will be replaced by a timer once bring-up succeeds.

It currently has no text renderer or USB CDC; fonts/navigation/communication come later.

If screen stays blank: unplug, reconfirm Pico position/orientation, then check LCD offsets, backlight polarity and panel initialization before reflashing. **Do not apply 12V/24V.**

Pin mapping: [board profile](../../hardware/boards/seengreat-pico-144-rev12.md). This demonstration deliberately bit-bangs SPI pins at low speed for first bring-up only; production code will use hardware SPI DMA.

## No safety claims

No E-stop, motion safety, bus wiring, or motor calibration. No independent power buffer or black-box persistence.
