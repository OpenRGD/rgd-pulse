/* RP2040/Pico W: 2 MiB external XIP flash, 264 KiB SRAM.
 *
 * The boot ROM reads exactly the first 256 bytes from flash at 0x10000000.
 * They MUST contain boot2, and the application vector table starts at
 * 0x10000100. Merely defining a BOOT2 region in MEMORY is insufficient.
 */
MEMORY
{
  BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
  FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
  RAM : ORIGIN = 0x20000000, LENGTH = 264K
}

SECTIONS
{
  .boot2 ORIGIN(BOOT2) :
  {
    KEEP(*(.boot2));
  } > BOOT2
} INSERT BEFORE .text;
