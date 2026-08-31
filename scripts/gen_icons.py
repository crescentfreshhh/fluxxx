#!/usr/bin/env python3
"""Generate the fluxxx application icon set (PNGs + Windows .ico).

Design: a rounded-square badge with a deep indigo->violet vertical gradient and a
centered white "play" triangle — a lean, recognizable IPTV/streaming motif.
Run from the repo root: python3 scripts/gen_icons.py
"""
from PIL import Image, ImageDraw
import os

OUT = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
os.makedirs(OUT, exist_ok=True)

BG_TOP = (99, 62, 240)      # indigo
BG_BOT = (168, 62, 240)     # violet
GLYPH = (255, 255, 255, 255)


def rounded_mask(size, radius):
    m = Image.new("L", (size, size), 0)
    d = ImageDraw.Draw(m)
    d.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=255)
    return m


def gradient(size):
    base = Image.new("RGB", (size, size), BG_TOP)
    top, bot = BG_TOP, BG_BOT
    px = base.load()
    for y in range(size):
        t = y / max(1, size - 1)
        r = int(top[0] + (bot[0] - top[0]) * t)
        g = int(top[1] + (bot[1] - top[1]) * t)
        b = int(top[2] + (bot[2] - top[2]) * t)
        for x in range(size):
            px[x, y] = (r, g, b)
    return base


def render(size):
    scale = 4
    S = size * scale
    grad = gradient(S).convert("RGBA")
    mask = rounded_mask(S, radius=int(S * 0.22))
    icon = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    icon.paste(grad, (0, 0), mask)

    # centered play triangle
    d = ImageDraw.Draw(icon)
    w = S * 0.30
    h = S * 0.34
    cx, cy = S * 0.53, S * 0.5
    tri = [
        (cx - w / 2, cy - h / 2),
        (cx - w / 2, cy + h / 2),
        (cx + w / 2, cy),
    ]
    d.polygon(tri, fill=GLYPH)

    return icon.resize((size, size), Image.LANCZOS)


def main():
    png_sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    imgs = {}
    for name, sz in png_sizes.items():
        img = render(sz)
        img.save(os.path.join(OUT, name))
        imgs[sz] = img
        print("wrote", name)

    # Windows .ico with multiple embedded resolutions
    ico_src = render(256)
    ico_src.save(
        os.path.join(OUT, "icon.ico"),
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("wrote icon.ico")


if __name__ == "__main__":
    main()
