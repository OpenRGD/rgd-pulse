# SEENGREAT PICO-1.44inch-LCD-Display Rev 1.2

**Evidence:** board marking photographed 2026-10-23 (operator-provided); pinout from vendor's [PICO 1.44inch LCD Display wiki](https://seengreat.com/wiki/69/pico-1-44inch-lcd-display). The assembled 128x128 display and input mapping were subsequently verified physically. On 2026-10-27 the operator accepted the LCD Align offsets X=1, Y=2 as correcting the top/left pixel artifacts.

| Function | Pico GPIO |
|---|---|
| SPI0 SCK / MOSI | GP6 / GP7 |
| ST7735S CS / D-C / RESET / BL | GP5 / GP11 / GP12 / GP13 |
| Buttons K1 / K2 / K3 / K4 | GP8 / GP4 / GP3 / GP2 |
| Joystick LEFT / DOWN / CLICK / UP / RIGHT (operator mapping) | GP16 / GP20 / GP17 / GP18 / GP19 |

Display: ST7735S, 128×128 RGB565, nominal 3.3V logic. Pico W with soldered 40-pin male headers. The display has matching female headers; align the labeled Pico pin numbering and USB end before inserting, and do not hot-plug.

**Initial test:** external power/motors disconnected. Power Pico ONLY via Micro-USB. Do not connect 12V or 24V directly. Active-low key inputs use internal pull-ups; debounce/behavior need hardware verification. The assembled panel accepted landscape MADCTL=0x68 + INVOFF=0x20, X_OFFSET=1, Y_OFFSET=2, with joystick left and K1..K4 right. X/Y were physically verified in the RAM-only LCD ALIGN diagnostic; a follow-up firmware cold-boot regression must confirm the newly compiled defaults.
