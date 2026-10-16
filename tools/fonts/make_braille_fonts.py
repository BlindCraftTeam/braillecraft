"""Builds the BrailleCraft braille screen fonts.

For every encoding table in tools/fonts/encodings/*.json two TrueType fonts are written to ui/fonts/:
  BrailleCraft Braille <Name>     the six-dot cells, raised dots only
  BrailleCraft SimBraille <Name>  the same cells with small marks at the unraised positions
Each table lists, for the 64 cells in dot-pattern order (dot 1 = 1, dot 2 = 2, dot 3 = 4, dot 4 = 8, dot 5 = 16,
dot 6 = 32), the characters that show that cell in that encoding. Every font also shows the Unicode braille
patterns U+2800 to U+283F and draws U+00A0 (no-break space) as the blank cell.

The glyphs are drawn here, by this program; no outside font is used.
Run: /srv/dev/tools/fontvenv/bin/python tools/fonts/make_braille_fonts.py   (from the repository root)
"""

import json
import math
import pathlib
import sys

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

ROOT = pathlib.Path(__file__).resolve().parents[2]
TABLES = ROOT / "tools" / "fonts" / "encodings"
OUT = ROOT / "ui" / "fonts"

UPM = 1000
ADVANCE = 600          # one cell, monospaced
ASCENT, DESCENT = 800, 200
COLUMNS = (180, 420)   # x of the left and right dot columns
ROWS = (640, 400, 160)  # y of dot rows 1-4, 2-5, 3-6
RAISED = 78            # radius of a raised dot
UNRAISED = 26          # radius of the SimBraille mark at an unraised position

DOT_PLACE = {0: (0, 0), 1: (0, 1), 2: (0, 2), 3: (1, 0), 4: (1, 1), 5: (1, 2)}  # bit -> (column, row)


def circle(pen, cx, cy, r):
    """A closed circle of eight quadratic segments, drawn clockwise (TrueType outer contour)."""
    n = 8
    off = r / math.cos(math.pi / n)
    on = [(cx + r * math.cos(-2 * math.pi * i / n), cy + r * math.sin(-2 * math.pi * i / n)) for i in range(n)]
    ctl = [(cx + off * math.cos(-2 * math.pi * (i + 0.5) / n), cy + off * math.sin(-2 * math.pi * (i + 0.5) / n)) for i in range(n)]
    pen.moveTo(tuple(round(v) for v in on[0]))
    for i in range(n):
        pen.qCurveTo(tuple(round(v) for v in ctl[i]), tuple(round(v) for v in on[(i + 1) % n]))
    pen.closePath()


def cell_glyph(pattern, shadow):
    pen = TTGlyphPen(None)
    for bit, (col, row) in DOT_PLACE.items():
        raised = pattern & (1 << bit)
        if raised or shadow:
            circle(pen, COLUMNS[col], ROWS[row], RAISED if raised else UNRAISED)
    return pen.glyph()


def notdef_glyph():
    pen = TTGlyphPen(None)
    pen.moveTo((60, 0)); pen.lineTo((60, 700)); pen.lineTo((540, 700)); pen.lineTo((540, 0)); pen.closePath()
    pen.moveTo((110, 50)); pen.lineTo((490, 50)); pen.lineTo((490, 650)); pen.lineTo((110, 650)); pen.closePath()
    return pen.glyph()


def code_points(entry):
    """A table entry is a string of characters, or a list mixing characters and "U+XXXX" values."""
    items = entry if isinstance(entry, list) else list(entry)
    for item in items:
        yield int(item[2:], 16) if isinstance(item, str) and item.startswith("U+") and len(item) > 2 else ord(item)


def build(table, shadow):
    kind = "SimBraille" if shadow else "Braille"
    family = f"BrailleCraft {kind} {table['name']}"
    names = [".notdef"] + [f"cell{p:02d}" for p in range(64)]
    glyphs = {".notdef": notdef_glyph()}
    for p in range(64):
        glyphs[f"cell{p:02d}"] = cell_glyph(p, shadow)

    cmap = {0x2800 + p: f"cell{p:02d}" for p in range(64)}
    cmap[0x00A0] = "cell00"
    for p, entry in enumerate(table["cells"]):
        for cp in code_points(entry):
            if cp in cmap and cp < 0x2800 and cmap[cp] != f"cell{p:02d}":
                raise SystemExit(f"{table['name']}: U+{cp:04X} is listed for cell {p} and for {cmap[cp]}")
            cmap[cp] = f"cell{p:02d}"

    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(names)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics({n: (ADVANCE, glyphs[n].xMin if hasattr(glyphs[n], "xMin") else 0) for n in names})
    fb.setupHorizontalHeader(ascent=ASCENT, descent=-DESCENT)
    fb.setupNameTable({
        "familyName": family,
        "styleName": "Regular",
        "uniqueFontIdentifier": f"{family.replace(' ', '')}-0.1",
        "fullName": family,
        "psName": family.replace(" ", ""),
        "version": "Version 0.1",
        "copyright": "Copyright 2026 Jeremiah Beasley",
        "description": f"Braille cells for the {table['name']} braille encoding. Source of the encoding: {table['source']}",
    })
    fb.setupOS2(sTypoAscender=ASCENT, sTypoDescender=-DESCENT, sTypoLineGap=0,
                usWinAscent=ASCENT, usWinDescent=DESCENT, achVendID="BMKR", fsType=0)
    fb.setupPost(isFixedPitch=1)
    fb.setupHead(unitsPerEm=UPM)
    out = OUT / f"BrailleCraft-{kind}-{table['id']}.ttf"
    fb.save(str(out))
    return out


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    tables = sorted(TABLES.glob("*.json"))
    if not tables:
        raise SystemExit(f"no tables in {TABLES}")
    index = []
    for path in tables:
        table = json.loads(path.read_text(encoding="utf-8"))
        if len(table["cells"]) != 64:
            raise SystemExit(f"{path.name}: {len(table['cells'])} cells, expected 64")
        for shadow in (False, True):
            print(build(table, shadow).relative_to(ROOT))
        # For the page: every cell's characters in this encoding, the first being the one BrailleCraft writes.
        chars = ["".join(chr(cp) for cp in code_points(entry)) for entry in table["cells"]]
        index.append({"id": table["id"], "name": table["name"], "order": table.get("order", 99),
                      "source": table["source"], "cells": chars})
    index.sort(key=lambda t: (t["order"], t["name"]))
    (OUT / "encodings.json").write_text(json.dumps(index, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    # The same list as a script, so the page can read it without a fetch (a page opened from a file cannot fetch).
    (OUT / "encodings.js").write_text(
        "// Written by tools/fonts/make_braille_fonts.py; do not edit.\n"
        f"window.BRAILLE_ENCODINGS = {json.dumps(index, indent=2, ensure_ascii=False)};\n", encoding="utf-8")
    print(f"{len(tables)} encodings, {2 * len(tables)} fonts")


if __name__ == "__main__":
    sys.exit(main())
