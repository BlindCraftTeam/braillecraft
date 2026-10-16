#!/usr/bin/env python3
"""Writes the WiX fragment that installs the braille fonts for every program (the SystemFonts component group): one
component per font file in the Fonts folder, registered as a TrueType font, as the reference installer's Font table
registers its braille fonts.

Usage: python3 packaging/windows/fonts_fragment.py <program folder>/fonts > fonts.wxs
"""

import os
import sys
from xml.sax.saxutils import quoteattr

folder = sys.argv[1]
fonts = sorted(f for f in os.listdir(folder) if f.lower().endswith(".ttf"))
print('<?xml version="1.0" encoding="utf-8"?>')
print('<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">')
print("  <Fragment>")
print('    <ComponentGroup Id="SystemFonts" Directory="FontsFolder">')
for i, name in enumerate(fonts):
    source = quoteattr("!(bindpath.dist)\\fonts\\" + name)
    print(f'      <Component Id="SystemFont{i}" Permanent="no"><File Id="SystemFont{i}" Source={source} TrueType="yes" KeyPath="yes" /></Component>')
print("    </ComponentGroup>")
print('    <StandardDirectory Id="FontsFolder" />')
print("  </Fragment>")
print("</Wix>")
