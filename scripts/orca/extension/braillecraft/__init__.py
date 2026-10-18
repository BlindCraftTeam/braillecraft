"""BrailleCraft user extension for Orca 51 and later.

Orca 51 no longer loads application scripts from its orca-scripts folder; it loads user extensions from
~/.local/share/orca/extensions after the user approves them (Preferences, User Extensions, or
orca --approve-extension braillecraft). This extension gives Orca, in BrailleCraft on Linux, what the BrailleCraft
JAWS scripts give JAWS, through the extension interface Orca documents (docs/user-extensions.md): commands with
Alt key bindings, the controller for speech, and the speech output and region hooks for braille documents.

The keys, each with Alt: 7 the translated line; 8 the current style (twice quickly: the table row and column); 9
the location; 0 the document name and the status line; comma the position on the line (twice: the characters to
the end); period the characters on the line (twice: the left spaces); End the last word on the line; semicolon
the translated line read; U the speaking level for up and down in a braille document; I the style change and
skipped line announcements; C whether a braille keyboard is connected; Up, Down, Left and Right the table entry
moves with the entry spoken; T the current table entry; H the key list. In a braille document what Orca speaks of
the editing area is spoken as braille cell names (the speech output hook), and contracted braille is turned off
while its editing area has focus and put back on leaving (the region changed hook and the braille presenter).

The document window as AT-SPI gives it: the frame "BrailleCraft - [name -- Print Document]" (Braille Document for a
braille document); a page tab per document holding a panel with the editing area (a text named Document Editing
Window), the status line (a label: "Current Style: x  Page:n  Line:n  Column:n  c  Keys:1", the table text in a
table; Page, Line and Column left out in the coded view) and the translated line (a text named Translated line).
"""

import re
import subprocess
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

from orca import keybindings
from orca.command import Command, KeyboardCommand
from orca.extension import (
    ActivePresentationMode,
    Extension,
    ExtensionPreference,
    RegionChange,
    SpeechOutput,
    SpeechOutputResult,
)

APP_NAME = "braillecraft-desktop"
EDITOR_NAME = "Document Editing Window"
TRANS_NAME = "Translated line"

# --- braille cell names (the tables of the BrailleCraft JAWS scripts, one name per character value) ---

_NA32 = ("space;t h e sign;dot 5;number sign;e d sign;s h sign;a n d sign;dot 3;o f sign;w i t h sign;c h sign;"
    "i n g sign;dot 6;dots 3 6;dots 4 6;s t sign").split(";")
_NA48 = ("low j;low a;low b;low c;low d;low e;low f;low g;low h;low i;w h sign;dots 5 6;g h sign;f o r sign;"
    "a r sign;t h sign;dot 4").split(";")
_NA91 = "o w sign;o u sign;e r sign;dots 4 5;dots 4 5 6;dot 4;a;b;c;d;e;f;g;h;i;j".split(";")
_NA107 = "k;l;m;n;o;p;q;r;s;t;u;v;w;x;y;z".split(";")
_NA123 = "dots 4 6;dots 4 5 6;dot 6;dots 1 2 4 5 6;hard space".split(";")
_RU1072 = ("dot 1;dots 1 2;dots 2 4 5 6;dots 1 2 4 5;dots 1 4 5;dots 1 5;dots 2 4 5;dots 1 3 5 6;dots 2 4;"
    "dots 1 2 3 4 6;dots 1 3;dots 1 2 3;dots 1 3 4;dots 1 3 4 5;dots 1 3 5;dots 1 2 3 4").split(";")
_RU1088 = ("dots 1 2 3 5;dots 2 3 4;dots 2 3 4 5;dots 1 3 6;dots 1 2 4;dots 1 2 5;dots 1 4;dots 1 2 3 4 5;"
    "dots 1 5 6;dots 1 3 4 6;dots 1 2 3 5 6;dots 2 3 4 6;dots 2 3 4 5 6;dots 2 4 6;dots 1 2 5 6;dots 1 2 4 6;"
    "dots 1 6").split(";")
_UNI = ("space;a;low a;b;dot 3;k;low b;l;dot 4;c;i;f;s t sign;m;s;p;"
    "dot 5;e;low c;h;low i;o;low f;r;dots 4 5;d;j;g;a r sign;n;t;q;"
    "dot 6;c h sign;low e;g h sign;hyphen;u;low h;v;dots 4 6;s h sign;o w sign;e d sign;i n g sign;x;t h e sign;"
    "a n d sign;dots 5 6;w h sign;low d;o u sign;low j;z;low g;o f sign;dots 4 5 6;t h sign;w;e r sign;"
    "number sign;y;w i t h sign;f o r sign").split(";")


def cell_name(ch):
    """The spoken name of one braille cell written as `ch`, or the character itself when it has none."""
    if not ch:
        return ""
    v = ord(ch[0])
    if v == 30 or v == 160:
        return "hard space"
    if 32 <= v < 48:
        return _NA32[v - 32]
    if 48 <= v <= 64:
        return _NA48[v - 48]
    if 65 <= v <= 90:
        v += 32
    if 91 <= v < 107:
        return _NA91[v - 91]
    if 107 <= v < 123:
        return _NA107[v - 107]
    if 123 <= v < 128:
        return _NA123[v - 123]
    if 1072 <= v < 1088:
        return _RU1072[v - 1072]
    if 1088 <= v < 1105:
        return _RU1088[v - 1088]
    if 0x2800 <= v < 0x2840:
        return _UNI[v - 0x2800]
    return ch


def cells_of(text):
    """`text` spoken cell by cell, runs of two or more spaces as a count."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        if text[i] == " " and i + 1 < n and text[i + 1] == " ":
            j = i
            while j < n and text[j] == " ":
                j += 1
            out.append("%d spaces" % (j - i))
            i = j
        else:
            out.append(cell_name(text[i]))
            i += 1
    return ", ".join(out)


# --- AT-SPI reading ---

def _name(obj):
    try:
        return obj.get_name() or ""
    except Exception:
        return ""


def _role(obj):
    try:
        return obj.get_role()
    except Exception:
        return None


def _text_of(obj):
    try:
        t = obj.get_text_iface()
        return t.get_text(0, t.get_character_count())
    except Exception:
        return ""


def _is_editor(obj):
    if obj is None or _name(obj) != EDITOR_NAME:
        return False
    try:
        app = obj.get_application()
        return _name(app) == APP_NAME
    except Exception:
        return False


class Status:
    """The status line taken apart."""

    def __init__(self, text):
        self.text = text or ""
        t = self.text
        m = re.search(r"Page:\s*(\S+)", t)
        self.page = m.group(1) if m else None
        m = re.search(r"Line:\s*(\d+)", t)
        self.line = int(m.group(1)) if m else None
        m = re.search(r"Column:\s*(\d+)", t)
        self.column = int(m.group(1)) if m else None
        m = re.search(r"Current Style:\s*(.*?)(?:\s\s|$)", t)
        self.style = m.group(1).strip() if m else ""
        m = re.search(r"Row\s*(\d+)/(\d+)", t)
        self.row = (int(m.group(1)), int(m.group(2))) if m else None
        m = re.search(r"Col\s*(\d+)/(\d+)", t)
        self.col = (int(m.group(1)), int(m.group(2))) if m else None
        self.in_table = bool(self.row and self.col) or "Matrix" in t
        if self.in_table and not self.style:
            self.style = t.split("  ")[0].strip()
        m = re.search(r"Keys:(\d)([Hh+]?)", t)
        self.six_key = bool(m and m.group(1) == "6")
        self.keyboard = bool(m and m.group(2) == "+")

    @property
    def coded(self):
        return self.page is None and bool(self.text)


class DocParts:
    """The parts of the document whose editing area is `editor`: the frame, the status label, the translated line."""

    def __init__(self, editor):
        self.editor = editor
        self.frame = None
        self.status = None
        self.trans = None
        o = editor
        panel = None
        for _ in range(30):
            if o is None:
                break
            r = _role(o)
            if panel is None and r == Atspi.Role.PANEL:
                panel = o
            if r == Atspi.Role.FRAME:
                self.frame = o
                break
            try:
                o = o.get_parent()
            except Exception:
                break
        if panel is not None:
            try:
                count = panel.get_child_count()
            except Exception:
                count = 0
            for i in range(count):
                try:
                    child = panel.get_child_at_index(i)
                except Exception:
                    continue
                name = _name(child)
                if _role(child) == Atspi.Role.LABEL and self.status is None and (name.startswith("Current Style") or "Row " in name):
                    self.status = child
                elif name == TRANS_NAME and self.trans is None:
                    self.trans = child

    @property
    def title(self):
        return _name(self.frame) if self.frame is not None else ""

    @property
    def is_braille(self):
        return "Braille Document" in self.title

    @property
    def status_text(self):
        return _name(self.status) if self.status is not None else ""

    @property
    def trans_showing(self):
        """1 when the translated line is shown, -1 when it shows that there is no translation, 0 when hidden."""
        if self.trans is None:
            return 0
        try:
            if not self.trans.get_state_set().contains(Atspi.StateType.SHOWING):
                return 0
        except Exception:
            pass
        return -1 if self.trans_text == "Error message 0" else 1

    @property
    def trans_text(self):
        return _text_of(self.trans) if self.trans is not None else ""


def caret_line(editor):
    """The caret line's text and the caret's column from 1."""
    try:
        t = editor.get_text_iface()
        offset = t.get_caret_offset()
        r = t.get_string_at_offset(offset, Atspi.TextGranularity.LINE)
        line = (r.content or "").rstrip("\n")
        return line, offset - r.start_offset + 1
    except Exception:
        return "", 1


LINE_LEVELS = (("silent", "Silent"), ("cells", "Braille characters"), ("translated", "Translated line"))

HELP = ("BrailleCraft keys, each with Alt. 7, the translated line. 8, the current style; twice quickly in a table, the "
    "row and column. 9, the location: page, line and column. 0, the document name and status line. Comma, the "
    "position on the line; twice quickly, the characters to the end of the line. Period, the number of characters on "
    "the line; twice quickly, the left spaces. End, the last word on the line. Semicolon, the translated line. U, the "
    "speaking level for up and down in a braille document. I, the style change and skipped line announcements. C, "
    "whether a braille keyboard is connected. Up, Down, Left and Right, the table entry moves. T, the current table "
    "entry. H, this list.")


class BrailleCraft(Extension):
    """The BrailleCraft reading keys and braille document behaviour for Orca."""

    GROUP_LABEL = "BrailleCraft"
    DESCRIPTION = "The BrailleCraft reading keys, braille cell names and exact braille on a display in BrailleCraft."
    VERSION = "1.0"
    AUTHOR = "BlindCraft Team"
    ORGANIZATION = "BlindCraft Team"
    COPYRIGHT = "2026 BlindCraft Team and the BrailleCraft contributors"
    WEBSITE = "https://github.com/BlindCraftTeam/braillecraft"

    def __init__(self):
        self._last_key = None
        self._last_key_time = 0.0
        self._last_page = None
        self._last_line = None
        self._last_style = None
        self._in_table = False
        self._contracted_saved = None
        self._braille_editor_seen = False
        super().__init__()

    # --- settings ---

    def get_preferences(self):
        return [
            ExtensionPreference.enum("line-level", "Up and down in a braille document speak", LINE_LEVELS, "translated"),
            ExtensionPreference.boolean("speak-cells", "Speak braille characters as braille cell names", True),
            ExtensionPreference.boolean("announce-styles", "Announce style changes", False),
            ExtensionPreference.boolean("announce-skips", "Announce skipped lines", False),
        ]

    def _line_level(self):
        return self.settings.get("line-level", default="translated")

    # --- speech ---

    def _say(self, text):
        if text:
            self.controller.present_message_internal(text)

    def _speak(self, text):
        if text:
            self.controller.speak_message_internal(text)

    # --- the document ---

    def _editor(self, say=True):
        obj = self.controller.get_current_object_internal()
        if _is_editor(obj):
            return obj
        if say:
            self._say("not in document window")
        return None

    def _twice(self, key):
        now = time.monotonic()
        again = self._last_key == key and now - self._last_key_time < 0.7
        self._last_key = key
        self._last_key_time = now
        return again

    # --- commands ---

    def _get_commands(self) -> list[Command]:
        keys = [
            ("braillecraft_translated_line", self.speak_trans_line, "Speaks the translated line", "7"),
            ("braillecraft_style", self.say_style_name, "Speaks the current style; twice: the table row and column", "8"),
            ("braillecraft_location", self.say_location, "Speaks the location: page, line and column", "9"),
            ("braillecraft_document_name", self.say_doc_name_and_status, "Speaks the document name and the status line", "0"),
            ("braillecraft_line_position", self.say_line_position, "Speaks the position on the line; twice: characters to the end", "comma"),
            ("braillecraft_line_length", self.say_total_and_left, "Speaks the characters on the line; twice: the left spaces", "period"),
            ("braillecraft_last_word", self.speak_last_word, "Speaks the last word on the line", "End"),
            ("braillecraft_translated_line_read", self.speak_trans_line, "Reads the translated line", "semicolon"),
            ("braillecraft_line_level", self.line_level_toggle, "Cycles the speaking level for up and down in a braille document", "u"),
            ("braillecraft_announcements", self.announcements_toggle, "Cycles the style change and skipped line announcements", "i"),
            ("braillecraft_braille_keyboard", self.check_braille_keyboard, "Says whether a braille keyboard is connected", "c"),
            ("braillecraft_next_row", self.next_row_entry, "Moves down one table row and speaks the entry", "Down"),
            ("braillecraft_prior_row", self.prior_row_entry, "Moves up one table row and speaks the entry", "Up"),
            ("braillecraft_next_column", self.next_column_entry, "Moves right one table column and speaks the entry", "Right"),
            ("braillecraft_prior_column", self.prior_column_entry, "Moves left one table column and speaks the entry", "Left"),
            ("braillecraft_table_entry", self.say_table_entry, "Speaks the current table entry", "t"),
            ("braillecraft_help", self.hot_key_help, "Lists the BrailleCraft keys", "h"),
        ]
        commands = []
        for name, function, description, key in keys:
            commands.append(KeyboardCommand(
                name, function, self.GROUP_LABEL, description,
                desktop_keybinding=keybindings.KeyBinding(key, keybindings.ALT_MODIFIER_MASK),
                laptop_keybinding=keybindings.KeyBinding(key, keybindings.ALT_MODIFIER_MASK),
            ))
        return commands

    def speak_trans_line(self):
        ed = self._editor()
        if ed is None:
            return True
        docs = DocParts(ed)
        if docs.trans_showing == 0:
            self._say("translated line not shown; press Control F8 to show it")
            return True
        if docs.trans_showing == -1:
            self._say("translation not available")
            return True
        text = docs.trans_text
        if not text.strip():
            self._say("blank")
        elif docs.is_braille:
            self._say(text)
        else:
            self._say(cells_of(text))
        return True

    def say_style_name(self):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        if self._twice("style") and status.in_table and status.row and status.col:
            self._say("Row %d Column %d" % (status.row[0], status.col[0]))
            return True
        self._say(status.style or "not in document window")
        return True

    def say_location(self):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        if status.coded:
            self._say("location not available in coded view, press Alt F3 to turn codes off")
            return True
        self._say("P%s L%s C%s" % (status.page, status.line, status.column))
        return True

    def say_doc_name_and_status(self):
        ed = self._editor()
        if ed is None:
            return True
        docs = DocParts(ed)
        self._say(docs.title + ": " + docs.status_text)
        return True

    def say_line_position(self):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        line, col = caret_line(ed)
        pos = status.column if status.column is not None else col
        to_end = len(line) - pos + 1
        if self._twice("position"):
            self._say("%d to last nonspace character" % (len(line.rstrip()) - pos))
        elif to_end == 0:
            self._say("position %d end of line" % pos)
        else:
            self._say("position %d" % pos)
        return True

    def say_total_and_left(self):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        if status.coded:
            self._say("coded view")
            return True
        line, _col = caret_line(ed)
        if self._twice("length"):
            self._say("%d left spaces" % (len(line) - len(line.lstrip())))
        else:
            self._say("%d total characters." % len(line))
        return True

    def speak_last_word(self):
        ed = self._editor()
        if ed is None:
            return True
        docs = DocParts(ed)
        status = Status(docs.status_text)
        if status.coded:
            self._say("location not available in coded view, press Alt F3 to turn codes off")
            return True
        line, _col = caret_line(ed)
        word = re.split(r"[ -]", line.rstrip())[-1]
        self._say(cells_of(word) if docs.is_braille else (word or "blank"))
        return True

    def line_level_toggle(self):
        level = self._line_level()
        level = {"silent": "translated", "translated": "cells", "cells": "silent"}.get(level, "translated")
        self.settings.set("line-level", level)
        self._say({"silent": "silent for up and down in braille", "cells": "read line as braille characters for up and down in braille",
            "translated": "use translated line for up and down in braille"}[level])
        return True

    def announcements_toggle(self):
        style = self.settings.get("announce-styles", default=False)
        skips = self.settings.get("announce-skips", default=False)
        if not style and not skips:
            style = True
        elif style and not skips:
            skips = True
        elif style and skips:
            style = False
        else:
            skips = False
        self.settings.set("announce-styles", style)
        self.settings.set("announce-skips", skips)
        self._say(("Announcements: Change of style on." if style else "Announcements: change of style off.")
            + " " + ("Skipped lines on." if skips else "Skipped lines off."))
        return True

    def check_braille_keyboard(self):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        self._say("braille keyboard connected" if status.keyboard else "braille keyboard not connected")
        return True

    def _table_entry(self, ed, docs, status):
        line, _col = caret_line(ed)
        col = status.col[0] if status.col else 1
        if status.coded:
            parts = [p for p in re.split(r"\[<\]|\[>\]", line) if p.strip()]
        elif "Linear" in status.style:
            parts = [p for p in re.split(r"; |: ", line) if p.strip()]
        elif "Listed" in status.style:
            self._say(line)
            return
        else:
            parts = [p for p in re.split(r" \.\.|\.\.|  ", line) if p.strip()]
        if "Two-Page" in status.style and status.page and status.page[-1:] in "13579" and status.col:
            col = col - status.col[1] - 1
        entry = parts[col - 1] if 0 < col <= len(parts) else ""
        self._say(cells_of(entry.strip()) if docs.is_braille else (entry.strip() or "blank"))

    def say_table_entry(self):
        ed = self._editor()
        if ed is None:
            return True
        docs = DocParts(ed)
        status = Status(docs.status_text)
        if status.in_table:
            self._table_entry(ed, docs, status)
        else:
            self._say("not in a table")
        return True

    def _table_move(self, keysym, speak_line):
        """Passes the arrow to the program (xdotool, X11) and speaks the entry the caret lands on."""
        ed = self._editor(say=False)
        if ed is None:
            return False
        try:
            subprocess.Popen(["xdotool", "key", "--clearmodifiers", "alt+" + keysym])
        except Exception:
            return False
        from gi.repository import GLib

        def after():
            docs = DocParts(ed)
            status = Status(docs.status_text)
            if status.in_table:
                self._table_entry(ed, docs, status)
            elif speak_line:
                line, _col = caret_line(ed)
                self._say(cells_of(line.rstrip()) if docs.is_braille and self._line_level() == "cells" else (line or "blank"))
            return False
        GLib.timeout_add(150, after)
        return True

    def next_row_entry(self):
        return self._table_move("Down", True)

    def prior_row_entry(self):
        return self._table_move("Up", True)

    def next_column_entry(self):
        return self._table_move("Right", False)

    def prior_column_entry(self):
        return self._table_move("Left", False)

    def hot_key_help(self):
        self._say(HELP)
        return True

    # --- braille documents: cell names, the translated line for up and down, contraction off ---

    def _set_contracted(self, on):
        try:
            self.controller.set_value_internal("BraillePresenter", "ContractedBrailleIsEnabled", bool(on))
        except Exception:
            pass

    def _announce_moves(self, status):
        if not status.coded and status.page != self._last_page and self._last_page is not None and status.page is not None:
            self._speak("Page " + status.page)
        if (self.settings.get("announce-skips", default=False) and status.line is not None and self._last_line is not None
                and status.page == self._last_page and abs(status.line - self._last_line) > 1):
            self._speak("line skip")
        if self.settings.get("announce-styles", default=False) and status.style and self._last_style is not None and status.style != self._last_style:
            self._speak(status.style)
        if status.in_table and not self._in_table:
            self._speak("entering table")
        elif self._in_table and not status.in_table:
            self._speak("exiting table")
        self._last_page = status.page
        self._last_line = status.line
        self._last_style = status.style
        self._in_table = status.in_table

    def on_region_changed(self, change: RegionChange) -> None:
        obj = change.obj
        in_braille = _is_editor(obj) and DocParts(obj).is_braille
        # Contracted braille off while a braille document's editing area is the place, as the JAWS scripts turn Grade
        # 2 translation off there; the user's choice back on leaving.
        if in_braille and self._contracted_saved is None:
            try:
                self._contracted_saved = bool(self.controller.get_value_internal("BraillePresenter", "ContractedBrailleIsEnabled"))
            except Exception:
                self._contracted_saved = False
            if self._contracted_saved:
                self._set_contracted(False)
        elif not in_braille and self._contracted_saved is not None:
            if self._contracted_saved:
                self._set_contracted(True)
            self._contracted_saved = None
        if in_braille and change.mode in (ActivePresentationMode.CARET_TRACKING, ActivePresentationMode.CARET_NAVIGATOR):
            self._announce_moves(Status(DocParts(obj).status_text))

    def on_speech_output(self, output: SpeechOutput) -> SpeechOutputResult | None:
        """In a braille document, what Orca is about to speak of the editing area becomes braille cell names, or the
        translated line for a line (per the speaking level)."""
        obj = output.obj
        if obj is None or not _is_editor(obj):
            return None
        docs = DocParts(obj)
        if not docs.is_braille:
            return None
        text = output.text or ""
        if not text.strip():
            return None
        line, _col = caret_line(obj)
        if line and text.rstrip("\n") == line:
            level = self._line_level()
            if level == "silent":
                return SpeechOutputResult.consume_output()
            if level == "translated":
                trans = docs.trans_text if docs.trans_showing == 1 else ""
                if trans.strip():
                    return SpeechOutputResult.replace(trans)
            return SpeechOutputResult.replace(cells_of(line.rstrip()))
        if self.settings.get("speak-cells", default=True) and len(text) <= 200:
            return SpeechOutputResult.replace(cells_of(text.rstrip("\n")))
        return None

    def on_disabled(self) -> None:
        if self._contracted_saved:
            self._set_contracted(True)
        self._contracted_saved = None

    def on_shutdown(self) -> None:
        self._contracted_saved = None
