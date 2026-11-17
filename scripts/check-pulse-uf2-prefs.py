#!/usr/bin/env python3
"""Fail closed if a RP2040 UF2 would overwrite the persistent settings partition.

Used AFTER elf2uf2-rs and BEFORE copying any UF2 to the RPI-RP2 drive.
Read-only check; the file is never modified.
"""
import argparse
import struct
from pathlib import Path

BOOT_START = 0x10000000
PREFS_START = 0x101FE000
FLASH_END = 0x10200000
UF2_BLOCK_SIZE = 512
UF2_START = (0x0A324655, 0x9E5D5157)
UF2_END = 0x0AB16F30
RP2040_FAMILY = 0xE48BFF56

def check(path: Path):
    size = path.stat().st_size
    if size == 0 or size % UF2_BLOCK_SIZE:
        raise ValueError("UF2 empty or not a multiple of 512 bytes")

    count = size // UF2_BLOCK_SIZE
    with path.open("rb") as stream:
        for i in range(count):
            block = stream.read(UF2_BLOCK_SIZE)
            magic_a, magic_b, flags, address, length, index, total, family = struct.unpack_from("<8I", block)
            tail = struct.unpack_from("<I", block, 508)[0]
            if (magic_a, magic_b) != UF2_START or tail != UF2_END:
                raise ValueError(f"Invalid UF2 header/trailer at block {i}")
            if length <= 0 or length > 476 or index >= count or total != count:
                raise ValueError(f"Invalid UF2 payload/count at block {i}")
            if not (flags & 0x2000) or family != RP2040_FAMILY:
                raise ValueError(f"Missing/incorrect RP2040 UF2 family ID at block {i}")
            if address < BOOT_START or address + length > PREFS_START:
                raise ValueError(
                    f"ABORT: UF2 block {i} [0x{address:08X},0x{address+length:08X}) "
                    "overlaps preferences partition or is out of range"
                )
            if address + length > FLASH_END:
                raise ValueError(f"Out of flash range: 0x{address:08X}")
    print(f"UF2 SAFE: {count} blocks, all below persistent region 0x{PREFS_START:08X}")
    print("This validates the file addresses, not physical chip identity or full power-loss behavior.")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("uf2", type=Path)
    args = parser.parse_args()
    try:
        check(args.uf2)
    except (OSError, ValueError) as err:
        parser.exit(1, f"ERROR: {err}\n")

if __name__ == "__main__":
    main()
