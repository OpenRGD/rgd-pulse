#!/usr/bin/env python3
"""READ-ONLY decoder for a picotool 8KiB RP2040 preferences flash dump.

Acquire manually in BOOTSEL with:
  picotool save -r 0x101fe000 0x10200000 prefs-0.3c.bin -t bin
This decoder never opens the device and never modifies firmware or flash.
"""
from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

JOURNAL_SIZE = 8192
PAGE_SIZE = 256
MAGIC = b"RGDP"
SKINS = ["Classic", "Phosphor", "Chrome 99", "Italia"]
PAGES = ["K1 SOMATIC", "K2 CHRONONS", "K3 IMU", "K4 SYSTEM"]


def crc16_ccitt_false(data: bytes) -> int:
    crc = 0xffff
    for byte in data:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ (0x1021 if crc & 0x8000 else 0)) & 0xffff
    return crc


def newer(a: int, b: int) -> bool:
    diff = (a - b) & 0xffffffff
    return 0 < diff < 0x80000000


def inspect(data: bytes) -> int:
    if len(data) != JOURNAL_SIZE:
        raise ValueError(f"Expected {JOURNAL_SIZE} bytes; got {len(data)}")

    records: list[tuple[int, int, int, int]] = []
    non_empty: list[tuple[int, str]] = []
    invalid_magic = 0

    for slot in range(JOURNAL_SIZE // PAGE_SIZE):
        block = data[slot * PAGE_SIZE:(slot + 1) * PAGE_SIZE]
        if all(byte == 0xff for byte in block):
            continue
        if block[:4] != MAGIC:
            invalid_magic += 1
            non_empty.append((slot, "unknown/non-record"))
            continue
        ver, skin, page, reserved = block[4:8]
        seq = struct.unpack_from("<I", block, 8)[0]
        got = struct.unpack_from("<H", block, 12)[0]
        want = crc16_ccitt_false(block[:12])
        valid = ver == 1 and skin < 4 and page < 4 and reserved == 0 and got == want
        if valid:
            records.append((seq, slot, skin, page))
        else:
            non_empty.append((slot, f"invalid v={ver} skin={skin} page={page} crc={got:04X}/{want:04X}"))

    print("=== OPENRGD PULSE — PREFS FLASH READ-ONLY ===")
    print(f"Journal bytes: {len(data)}")
    print(f"Erased pages: {JOURNAL_SIZE // PAGE_SIZE - len(records) - len(non_empty)}")
    print(f"Valid records: {len(records)}")
    print(f"Invalid/non-record pages: {len(non_empty)}")
    if invalid_magic:
        print(f"Unknown header pages: {invalid_magic}")
    for seq, slot, skin, page in records[-8:]:
        sector, entry = divmod(slot, 16)
        print(f"VALID sector={sector} slot={entry} seq={seq} skin={SKINS[skin]} page={PAGES[page]}")
    for slot, reason in non_empty[:8]:
        print(f"NOT VALID sector={slot // 16} slot={slot % 16}: {reason}")

    if not records:
        if not non_empty:
            print("DIAGNOSIS: BOTH SECTORS ERASED — no confirmed settings write reached this region.")
        else:
            print("DIAGNOSIS: no decodable settings record; inspect chip/offsets or interrupted writes.")
        return 2

    newest = records[0]
    for record in records[1:]:
        if newer(record[0], newest[0]):
            newest = record
    seq, slot, skin, page = newest
    print(f"RESTORE TARGET: seq={seq} sector={slot // 16} slot={slot % 16} skin={SKINS[skin]} page={PAGES[page]}")
    print("Only a device reboot can prove whether firmware actually restores these values.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("flash_dump", type=Path)
    args = parser.parse_args()
    try:
        return inspect(args.flash_dump.read_bytes())
    except (OSError, ValueError) as exc:
        print(f"ABORT: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
