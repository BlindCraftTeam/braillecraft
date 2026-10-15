"""Writes BrailleMaker encoding tables (tools/fonts/encodings/*.json) from a braille display code page file: for each
section, the characters that show each of the 64 cells.

Run: python3 tools/fonts/import_display_encodings.py <path to the code page file>
The two North American tables already in the folder are kept (written from the published standard); every other
section becomes a table named after it.
"""

import json
import pathlib
import re
import sys

OUT = pathlib.Path(__file__).resolve().parent / "encodings"


def sections(path):
    lines = pathlib.Path(path).read_text(encoding="utf-8-sig").splitlines()
    body = [l for l in lines if l.strip() and not l.startswith("| ") and l.strip() != "|"]
    for i in range(0, len(body) - 64, 65):
        name = body[i].split("|")[0].strip()
        cells = []
        for row in body[i + 1:i + 65]:
            values = row.split("|")[0].split()
            cells.append([int(v[2:], 16) if v.upper().startswith("U+") else int(v) for v in values])
        yield name, cells


def slug(name):
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def entry(codes):
    # Characters as text where printable, "U+XXXX" otherwise; the no-break space is added by the font builder.
    out = []
    for cp in codes:
        if cp == 0xA0:
            continue
        out.append(chr(cp) if 0x20 <= cp != 0x7F and not (0x80 <= cp < 0xA0) else f"U+{cp:04X}")
    return out if len(out) != 1 else out[0]


def main():
    order = 0
    for name, cells in sections(sys.argv[1]):
        order += 1
        table_id = slug(name)
        path = OUT / f"{table_id}.json"
        if table_id in ("north-american-english", "north-american-english-variant"):
            continue  # north-american.json and north-american-variant.json cover these
        seen = {}
        clean = []
        for p, codes in enumerate(cells):
            kept = []
            for cp in codes:
                # A character listed for two cells keeps its first cell (the font maps one cell per character).
                if cp in seen:
                    print(f"{name}: U+{cp:04X} listed for cells {seen[cp]} and {p}; kept cell {seen[cp]}")
                    continue
                seen[cp] = p
                kept.append(cp)
            clean.append(entry(kept) if kept else "")
        table = {"id": table_id, "name": name, "order": order,
                 "source": f"Cell assignments for {name} braille.", "cells": clean}
        path.write_text(json.dumps(table, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print(path.name)


if __name__ == "__main__":
    main()
