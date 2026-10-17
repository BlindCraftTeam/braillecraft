#!/usr/bin/env python3
"""Packs the BrailleCraft NVDA add-on (scripts/nvda) into an .nvda-addon file.

An NVDA add-on is a zip archive with manifest.ini at its root. This writes <out dir>/braillecraft-<version>.nvda-addon
from scripts/nvda, the version read from the manifest, after compiling every Python file in it so a broken script
fails the build rather than the install.

Usage: python3 tools/nvda/make_addon.py <out dir>
"""

import os
import re
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.normpath(os.path.join(HERE, "..", "..", "scripts", "nvda"))


def version():
    with open(os.path.join(SOURCE, "manifest.ini"), encoding="utf-8") as f:
        for line in f:
            m = re.match(r'\s*version\s*=\s*"([^"]+)"', line)
            if m:
                return m.group(1)
    raise SystemExit("manifest.ini has no version")


def main(out_dir):
    files = []
    for root, _dirs, names in os.walk(SOURCE):
        for name in sorted(names):
            if name.endswith(".pyc") or name == "__pycache__":
                continue
            path = os.path.join(root, name)
            if name.endswith(".py"):
                with open(path, encoding="utf-8") as f:
                    compile(f.read(), path, "exec")
            files.append(path)
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, "braillecraft-%s.nvda-addon" % version())
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for path in sorted(files):
            z.write(path, os.path.relpath(path, SOURCE).replace(os.sep, "/"))
    print(out)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    main(sys.argv[1])
