#!/usr/bin/env python3
"""Writes BrailleCraft's own icons into packaging/icons: the program (two braille cells, b and c, on a blue square)
and the two document types (a page with the letter P for print documents, a page with braille dots for braille
documents), as PNG at each size, Windows .ico files, and a macOS iconset folder for iconutil.

Run from the repository root with Pillow: python3 tools/icons/make_icons.py
"""

import os

from PIL import Image, ImageDraw, ImageFont

OUT = "packaging/icons"
SIZES = [16, 24, 32, 48, 64, 128, 256, 512, 1024]
BLUE = (21, 84, 160, 255)
WHITE = (255, 255, 255, 255)
PAGE = (250, 250, 250, 255)
EDGE = (90, 90, 90, 255)
INK = (30, 30, 30, 255)

# Braille cells as raised dots (1 to 6): b is dots 1 2, c is dots 1 4.
B, C, P, DOTS = {1, 2}, {1, 4}, {1, 2, 3, 4}, {1, 2, 4, 5}


def cell(draw, x, y, dots, r, gap, fill, empty):
    """A braille cell with its top-left dot centre at (x, y): raised dots filled, the others as small rings."""
    for d in range(1, 7):
        cx = x + (gap if d > 3 else 0)
        cy = y + ((d - 1) % 3) * gap
        box = [cx - r, cy - r, cx + r, cy + r]
        if d in dots:
            draw.ellipse(box, fill=fill)
        elif empty:
            draw.ellipse(box, outline=empty, width=max(1, int(r // 3)))


def program(size):
    s = size * 4
    im = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([s * 0.04, s * 0.04, s * 0.96, s * 0.96], radius=s * 0.18, fill=BLUE)
    # The space between the two cells is wider than between the dots of a cell, as on paper.
    gap = s * 0.17
    r = s * 0.065
    cell(d, s * 0.19, s * 0.33, B, r, gap, WHITE, (255, 255, 255, 110))
    cell(d, s * 0.64, s * 0.33, C, r, gap, WHITE, (255, 255, 255, 110))
    return im.resize((size, size), Image.LANCZOS)


def document(size, braille):
    s = size * 4
    im = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    fold = s * 0.24
    left, top, right, bottom = s * 0.16, s * 0.06, s * 0.84, s * 0.94
    d.polygon([(left, top), (right - fold, top), (right, top + fold), (right, bottom), (left, bottom)], fill=PAGE, outline=EDGE, width=max(1, s // 64))
    d.polygon([(right - fold, top), (right - fold, top + fold), (right, top + fold)], fill=(215, 215, 215, 255), outline=EDGE, width=max(1, s // 64))
    if braille:
        gap = s * 0.12
        r = s * 0.045
        cell(d, s * 0.27, s * 0.4, DOTS, r, gap, BLUE, None)
        cell(d, s * 0.6, s * 0.4, B, r, gap, BLUE, None)
    else:
        try:
            font = ImageFont.truetype("DejaVuSans-Bold.ttf", int(s * 0.5))
        except OSError:
            font = ImageFont.load_default()
        d.text((s * 0.5, s * 0.58), "P", font=font, fill=BLUE, anchor="mm")
    return im.resize((size, size), Image.LANCZOS)


def write(name, make):
    images = {n: make(n) for n in SIZES}
    os.makedirs(f"{OUT}/png", exist_ok=True)
    for n, im in images.items():
        im.save(f"{OUT}/png/{name}-{n}.png")
    images[256].save(f"{OUT}/{name}.ico", sizes=[(n, n) for n in [16, 24, 32, 48, 64, 128, 256]])
    iconset = f"{OUT}/{name}.iconset"
    os.makedirs(iconset, exist_ok=True)
    for n in [16, 32, 128, 256, 512]:
        images[n].save(f"{iconset}/icon_{n}x{n}.png")
        images[n * 2].save(f"{iconset}/icon_{n}x{n}@2x.png")


if __name__ == "__main__":
    write("braillecraft", program)
    write("print-document", lambda n: document(n, False))
    write("braille-document", lambda n: document(n, True))
    print("icons written to", OUT)
