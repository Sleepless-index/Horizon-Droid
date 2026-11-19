#!/usr/bin/env python3
"""Renders the Horizon Android launcher icon: a pixel-block "H" on a charcoal plate.

The H is built from square cells on a 7 x 9 grid, with a diagonal gradient from
#976AB0 (top left) to #F7A3CA (bottom right) and a thin seam between cells so the
pixels read at any size. The plate is a dark charcoal squircle with a soft
vertical sheen and a hairline border; the corners are transparent.

    python3 tools/render_icon.py            # writes the launcher icon
    python3 tools/render_icon.py out.png    # writes somewhere else

Needs numpy; nothing else.
"""

import struct
import sys
import zlib
from pathlib import Path

import numpy as np

SIZE = 1024
GRADIENT_START = (0x97, 0x6A, 0xB0)
GRADIENT_END = (0xF7, 0xA3, 0xCA)
PLATE_TOP = (0x24, 0x24, 0x2B)
PLATE_BOTTOM = (0x13, 0x13, 0x18)
PLATE_RADIUS = 0.225  # fraction of the icon width
CELL = 72  # px per H cell at 1024
SEAM = 3  # px darkened between cells

# 7 x 9 H: two 2-cell pillars joined by a 3-row bar.
H_ROWS = [
    "XX...XX",
    "XX...XX",
    "XX...XX",
    "XXXXXXX",
    "XXXXXXX",
    "XXXXXXX",
    "XX...XX",
    "XX...XX",
    "XX...XX",
]


def rounded_box_alpha(size, radius):
    """Anti-aliased coverage of a rounded square filling the whole canvas."""
    ys, xs = np.mgrid[0:size, 0:size].astype(np.float64)
    px = xs + 0.5 - size / 2
    py = ys + 0.5 - size / 2
    half = size / 2
    qx = np.abs(px) - half + radius
    qy = np.abs(py) - half + radius
    dist = np.minimum(np.maximum(qx, qy), 0.0) + np.hypot(np.maximum(qx, 0.0), np.maximum(qy, 0.0)) - radius
    return np.clip(0.5 - dist, 0.0, 1.0), dist


def lerp(a, b, t):
    return a + (b - a) * t


def render(size=SIZE):
    scale = size / 1024
    radius = PLATE_RADIUS * size
    plate_alpha, plate_dist = rounded_box_alpha(size, radius)

    ys = np.arange(size, dtype=np.float64)[:, None] / (size - 1)
    xs = np.arange(size, dtype=np.float64)[None, :] / (size - 1)
    sheen = np.broadcast_to(ys, (size, size))

    img = np.zeros((size, size, 3), dtype=np.float64)
    for c in range(3):
        img[..., c] = lerp(PLATE_TOP[c], PLATE_BOTTOM[c], sheen)

    # Hairline border, just inside the plate edge.
    border = np.clip(1.0 - np.abs(plate_dist + 2.0 * scale) / (1.5 * scale), 0.0, 1.0) * 0.10
    img += border[..., None] * 255.0

    cell = CELL * scale
    seam = max(1.0, SEAM * scale)
    cols = len(H_ROWS[0])
    rows = len(H_ROWS)
    gw, gh = cols * cell, rows * cell
    gx0 = (size - gw) / 2
    gy0 = (size - gh) / 2

    cy_grid, cx_grid = np.mgrid[0:size, 0:size].astype(np.float64)
    gx = cx_grid + 0.5 - gx0
    gy = cy_grid + 0.5 - gy0
    col = np.floor(gx / cell).astype(int)
    row = np.floor(gy / cell).astype(int)
    inside = (col >= 0) & (col < cols) & (row >= 0) & (row < rows)
    on = np.zeros((size, size), dtype=bool)
    for r, line in enumerate(H_ROWS):
        for c, ch in enumerate(line):
            if ch == "X":
                on |= (row == r) & (col == c)

    # Seams: darken a thin line along every cell edge that borders another cell.
    fx = gx - col * cell
    fy = gy - row * cell
    edge = np.minimum(np.minimum(fx, cell - fx), np.minimum(fy, cell - fy))
    seam_t = np.clip(1.0 - edge / seam, 0.0, 1.0)

    # Soft glow behind the glyph so it sits on the plate instead of floating.
    from_center = np.hypot(cx_grid + 0.5 - size / 2, cy_grid + 0.5 - size / 2) / (size * 0.5)
    glow = np.clip(1.0 - from_center / 0.78, 0.0, 1.0) ** 2 * 0.10
    mid = tuple((a + b) / 2 for a, b in zip(GRADIENT_START, GRADIENT_END))
    for c in range(3):
        img[..., c] += glow * mid[c]

    # Diagonal gradient measured across the whole glyph, not per cell.
    t = np.clip(((gx / gw) + (gy / gh)) / 2.0, 0.0, 1.0)
    glyph = np.zeros_like(img)
    for c in range(3):
        glyph[..., c] = lerp(GRADIENT_START[c], GRADIENT_END[c], t)
    # Per-cell top highlight and seam shadow give the blocks a chunky pixel feel.
    glyph *= (1.0 - 0.28 * seam_t)[..., None]
    glyph += (np.clip(1.0 - fy / (seam * 2.0), 0.0, 1.0) * 14.0)[..., None]

    mask = (on & inside)[..., None]
    img = np.where(mask, glyph, img)

    rgb = np.clip(img, 0, 255)
    alpha = plate_alpha * 255.0
    out = np.dstack([rgb, alpha]).round().astype(np.uint8)
    return out


def encode_png(pixels):
    height, width = pixels.shape[:2]
    raw = b"".join(b"\x00" + pixels[row].tobytes() for row in range(height))

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


if __name__ == "__main__":
    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("packaging/android/res/drawable-nodpi/launcher_icon.png")
    data = encode_png(render())
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_bytes(data)
    print(f"Generated {out_path} ({len(data)} bytes)")
