#!/usr/bin/env python3
"""Builds the BrailleCraft VoiceOver scripts and the preferences file that binds them to keys.

For each key script in scripts/voiceover/keys, the shared source scripts/voiceover/common.applescript is put in
front of it (each compiled script stands alone: VoiceOver runs one file per key) and the result is written to
<out>/src/<name>.applescript; where osacompile exists (macOS) it is compiled to <out>/<name>.scpt. Then
<out>/BrailleCraft.voprefs is written: a VoiceOver Command Assignments export (ArchiveType 4096, the kind VoiceOver
Utility writes for File, Export Preferences, Command Assignments) whose Keyboard Commander entries run the scripts
from <install path> (the folder inside the installed application, /Applications/BrailleCraft.app/Contents/Resources/
VoiceOver). The format is the one VoiceOver Utility writes (research row 964): the key
custom.keyboard-commander.SCRKeyboardCommander_<four hex digits of the key character> and the value
SCRWorkspace.customOpenScript:<path>.

Usage: tools/voiceover/build.py <out folder> [--install-path <folder the scripts are run from>] [--no-compile]
"""
import os
import plistlib
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SRC = os.path.join(ROOT, "scripts", "voiceover")
INSTALL = "/Applications/BrailleCraft.app/Contents/Resources/VoiceOver"

# The Keyboard Commander key (Option plus the character) of each script, as the JAWS scripts' Alt keys.
KEYS = [
    ("translated-line", "7"),
    ("style-name", "8"),
    ("location", "9"),
    ("document-name", "0"),
    ("line-position", ","),
    ("line-length", "."),
    ("last-word", "l"),
    ("translated-line-cursor", ";"),
    ("table-entry", "t"),
    ("braille-keyboard", "c"),
    ("key-help", "h"),
]


def main(argv):
    if len(argv) < 2 or argv[1].startswith("--"):
        sys.exit(__doc__)
    out = argv[1]
    install = INSTALL
    compile_scripts = True
    i = 2
    while i < len(argv):
        if argv[i] == "--install-path":
            install = argv[i + 1]
            i += 2
        elif argv[i] == "--no-compile":
            compile_scripts = False
            i += 1
        else:
            sys.exit("unknown argument " + argv[i])
    src_out = os.path.join(out, "src")
    os.makedirs(src_out, exist_ok=True)
    with open(os.path.join(SRC, "common.applescript"), encoding="utf-8") as f:
        common = f.read()
    osacompile = shutil.which("osacompile") if compile_scripts else None
    entries = {}
    for name, key in KEYS:
        with open(os.path.join(SRC, "keys", name + ".applescript"), encoding="utf-8") as f:
            body = f.read()
        text = common.rstrip("\n") + "\n\n" + body
        src_path = os.path.join(src_out, name + ".applescript")
        with open(src_path, "w", encoding="utf-8") as f:
            f.write(text)
        if osacompile:
            subprocess.run([osacompile, "-o", os.path.join(out, name + ".scpt"), src_path], check=True)
        entries["custom.keyboard-commander.SCRKeyboardCommander_%04x" % ord(key)] = (
            "SCRWorkspace.customOpenScript:" + install + "/" + name + ".scpt")
    prefs = {
        "ArchiveType": 4096,
        "ArchiveVersion": "993",
        "ArchiveVersionOS": "270000",
        "ArchiveVersionVO": 4,
        "export": {"default": entries},
    }
    with open(os.path.join(out, "BrailleCraft.voprefs"), "wb") as f:
        plistlib.dump(prefs, f, sort_keys=True)
    with open(os.path.join(out, "README.txt"), "w", encoding="utf-8") as f:
        f.write(README)
    print("%d scripts%s, BrailleCraft.voprefs" % (len(KEYS), " compiled" if osacompile else " (source only)"))


README = """BrailleCraft keys for VoiceOver

1. Put BrailleCraft in Applications (the scripts are run from
   /Applications/BrailleCraft.app/Contents/Resources/VoiceOver).
2. In VoiceOver Utility, General, turn on Allow VoiceOver to be controlled with AppleScript
   (it asks for an administrator name and password).
3. In VoiceOver Utility choose File, Import Preferences, and open BrailleCraft.voprefs.
   It is a Command Assignments file: VoiceOver replaces its Keyboard Commander key assignments with
   these, so export yours first if you have any (File, Export Preferences, Command Assignments).
4. Make sure the Keyboard Commander is on (Commanders, Keyboard, Enable Keyboard Commander on macOS 14
   and earlier; the Option Key inputs of the Commands category on later versions).

The keys, each with the Option key, in a BrailleCraft document window:
  7  the translated line
  8  the current style; twice quickly in a table, the row and column
  9  the location: page, line and column
  0  the document name, its place among the window's tabs ("tab 2 of 3") when there are several,
     and the status line
  ,  the position on the line; twice quickly, the characters to the end of the line
  .  the number of characters on the line; twice quickly, the left spaces
  L  the last word on the line
  ;  the cursor to the translated line, and back to the editing area
  T  the current table entry
  C  whether a braille keyboard is connected
  H  this list

The open documents are tabs of the window, as in the system's own programs: Command+Shift+Right
Bracket and Left Bracket, or Control+Tab and Control+Shift+Tab, move between them (Control+Tab works
with every keyboard layout); Command+W closes one, as does the close button on its tab; the plus
button at the end of the tab bar is File, New. The Window menu has Show Next Tab, Show Previous Tab,
Move Tab to New Window and Merge All Windows. These are the program's keys, not VoiceOver's, so the
Command Assignments file has nothing for them; VoiceOver reads the tab bar itself as a tab group.
"""


if __name__ == "__main__":
    main(sys.argv)
