"""Packs the Windows code page tables (written on Windows by crates/bc-core/examples/windows_code_pages.rs, the
windows-code-pages CI artifact) into crates/bc-core/data/windows-code-pages.bin for bc_core::codepages.

Usage: python3 -I make_windows_tables.py <folder with the .encode and .decode files> <out.bin>

The file, little-endian: "BCWCP1", the number of pages (u16); per page: its number (u32), the number of characters
it writes (u32), for each the UTF-16 unit (u16), the number of bytes (u8) and the bytes; the number of byte
sequences it reads (u32), for each the number of bytes (u8), the bytes, the number of UTF-16 units (u8) and the
units (u16 each). UTF-7 (65000) is left out: it is written by its rule, as the standard gives it.
"""
import os
import struct
import sys

LEFT_OUT = {65000}


def main(folder, out):
    pages = sorted(int(n.split(".")[0]) for n in os.listdir(folder) if n.endswith(".encode"))
    pages = [p for p in pages if p not in LEFT_OUT]
    blob = bytearray(b"BCWCP1")
    blob += struct.pack("<H", len(pages))
    for p in pages:
        enc = []
        with open(os.path.join(folder, "%d.encode" % p)) as f:
            for line in f:
                unit, data = line.split()
                enc.append((int(unit, 16), bytes.fromhex(data)))
        dec = []
        path = os.path.join(folder, "%d.decode" % p)
        if os.path.exists(path):
            with open(path) as f:
                for line in f:
                    parts = line.split()
                    dec.append((bytes.fromhex(parts[0]), [int(u, 16) for u in parts[1:]]))
        blob += struct.pack("<II", p, len(enc))
        for unit, data in enc:
            blob += struct.pack("<HB", unit, len(data)) + data
        blob += struct.pack("<I", len(dec))
        for data, units in dec:
            blob += struct.pack("<B", len(data)) + data + struct.pack("<B", len(units))
            blob += b"".join(struct.pack("<H", u) for u in units)
    with open(out, "wb") as f:
        f.write(blob)
    print("%d pages, %d bytes" % (len(pages), len(blob)))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
