#!/usr/bin/env python3
"""Renders the app icon (same design as src/tray.rs::icon_rgba) as a 1024x1024 PNG."""
import struct, sys, zlib

def render(size):
    px = bytearray(size * size * 4)
    c = (size - 1) / 2
    r = size / 2 - 0.5 - size * 0.06          # margin like macOS icons
    chip, pin = size * 0.22 * 0.88, size * 0.32 * 0.88
    seg = 2 * chip / 13                        # 6 evenly spaced pins per side
    for y in range(size):
        for x in range(size):
            dx, dy = x - c, y - c
            d = (dx * dx + dy * dy) ** 0.5
            if d > r + 1:
                continue
            a = int(max(0.0, min(1.0, r - d + 0.5)) * 255)
            is_chip = abs(dx) < chip and abs(dy) < chip
            kx, ky = int((dx + chip) // seg), int((dy + chip) // seg)
            is_pin = ((abs(dx) < pin and abs(dy) < chip and ky % 2 == 1) or
                      (abs(dy) < pin and abs(dx) < chip and kx % 2 == 1))
            col = (30, 22, 40) if (is_chip or is_pin) else (247, 147, 26)
            i = (y * size + x) * 4
            px[i:i + 4] = bytes((*col, a))
    return px

def write_png(path, size, px):
    raw = b"".join(b"\0" + bytes(px[y * size * 4:(y + 1) * size * 4]) for y in range(size))
    ch = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
                + ch(b"IDAT", zlib.compress(raw, 9)) + ch(b"IEND", b""))

if __name__ == "__main__":
    write_png(sys.argv[1], 1024, render(1024))
