# BrailleCraft Orca script: what the BrailleCraft JAWS scripts give JAWS, for Orca on Linux.
#
# The keys, each with Alt: 7 the translated line; 8 the current style (twice: the table row and column); 9 the
# location (page, line, column); 0 the document name and the status line; comma the position on the line (twice: the
# characters to the end); period the characters on the line (twice: the left spaces); End the last word on the line;
# semicolon the review cursor to the translated line and back; U the speaking level for up and down in a braille
# document (silent, braille characters, the translated line); I the style change and skipped line announcements; P
# the echo for entry in a braille document; W the reading of a completed braille word at a space; C whether a braille
# keyboard is connected; Up, Down, Left and Right the table entry moves with the entry spoken; H the key list. In a
# braille document the characters, words and lines the caret moves over are spoken as braille cell names (the
# tables of the JAWS scripts), and contracted braille is turned off while the editing area has focus so a braille
# display shows the document's cells themselves (the JAWS scripts turn Grade 2 translation off there).
#
# The document window as AT-SPI gives it (checked in the development container): the frame "BrailleCraft - [name --
# Print Document]" (Braille Document for a braille document); in it a page tab list, one page tab per document, each
# holding a panel with the scroll pane of the editing area (a text named Document Editing Window: each formatted
# line or coded row followed by a line feed, the caret as its offset), the status line (a label: "Current Style: x
# Page:n  Line:n  Column:n  c  Keys:1", or the table text with Row r/n Col c/n in a table; Page, Line and Column
# left out in the coded view; Keys:6 for six-key entry, H after it for highlighting, + for a connected braille
# keyboard) and the translated line (a text named Translated line, shown while View, Translated Line is on; "Error
# message 0" when the document has no translation).
#
# Orca 48 and 49 give an application script handlers (setup_input_event_handlers, get_app_key_bindings) and the
# messages presentMessage/speakMessage (48) or present_message/speak_message (49); Orca 50 registers commands
# through command_manager in set_up_commands. This script works with each of them.

import re

from orca import braille, debug, focus_manager, keybindings, settings
from orca.ax_object import AXObject
from orca.ax_text import AXText
from orca.ax_utilities import AXUtilities
from orca.scripts import default
from orca.scripts.toolkits import gtk

try:
    from orca import input_event
except ImportError:  # pragma: no cover
    input_event = None
try:
    from orca import command_manager
except ImportError:  # pragma: no cover
    command_manager = None
try:
    from orca import presentation_manager
except ImportError:  # pragma: no cover
    presentation_manager = None

EDITOR_NAME = "Document Editing Window"
TRANS_NAME = "Translated line"
GROUP = "BrailleCraft"

# Orca 48 and 49 declare key bindings through input event handlers; Orca 50 through the command manager.
OLD_API = hasattr(default.Script, "setup_input_event_handlers")

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


# --- the status line taken apart ---

class Status:
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
        self.highlight = bool(m and m.group(2).upper() == "H")
        self.keyboard = bool(m and m.group(2) == "+")

    @property
    def coded(self):
        # The coded view leaves Page, Line and Column off the status line; a status line that was not found is not
        # taken for the coded view.
        return self.page is None and bool(self.text)


# --- the document window ---

class DocParts:
    """The parts of the document whose editing area is `editor`: the frame, the status label, the translated line."""

    def __init__(self, editor):
        self.editor = editor
        self.frame = None
        self.status = None
        self.trans = None
        o = editor
        panel = None
        while o is not None:
            role = AXObject.get_role_name(o) if hasattr(AXObject, "get_role_name") else ""
            if panel is None and role == "panel":
                panel = o
            if role == "frame":
                self.frame = o
                break
            o = AXObject.get_parent(o)
        if panel is not None:
            for child in AXObject.iter_children(panel):
                name = AXObject.get_name(child) or ""
                role = AXObject.get_role_name(child) if hasattr(AXObject, "get_role_name") else ""
                if role == "label" and self.status is None and (name.startswith("Current Style") or "Row " in name):
                    self.status = child
                elif name == TRANS_NAME and self.trans is None:
                    self.trans = child

    @property
    def title(self):
        return AXObject.get_name(self.frame) or "" if self.frame is not None else ""

    @property
    def is_braille(self):
        return "Braille Document" in self.title

    @property
    def status_text(self):
        return (AXObject.get_name(self.status) or "") if self.status is not None else ""

    @property
    def trans_showing(self):
        """1 when the translated line is shown, -1 when it shows that there is no translation, 0 when hidden."""
        if self.trans is None:
            return 0
        try:
            if not AXUtilities.is_showing(self.trans):
                return 0
        except Exception:
            pass
        return -1 if self.trans_text == "Error message 0" else 1

    @property
    def trans_text(self):
        if self.trans is None:
            return ""
        try:
            return AXText.get_all_text(self.trans)
        except Exception:
            return ""


class Script(gtk.Script):
    """The BrailleCraft script."""

    def __init__(self, app):
        super().__init__(app)
        # The JAWS scripts' Quick Settings choices, kept for the session.
        self.line_level = 2          # 0 silent, 1 braille characters, 2 the translated line (up and down in braille)
        self.input_echo = 4          # 0 as Orca, 1 silent, 2 characters, 3 words, 4 both (entry in braille)
        self.speak_cells = True      # braille characters spoken as cell names
        self.announce_styles = False
        self.announce_skips = False
        self.last_page = None
        self.last_line = None
        self.last_style = None
        self.in_table = False
        self.last_trans = ""
        self.review_on_trans = False
        self.contracted_saved = None
        self._last_key = None
        self._last_key_time = 0.0

    # --- speech ---

    def _say(self, text):
        """Speaks `text` and flashes it in braille, through whichever API this Orca has."""
        if not text:
            return
        debug.print_message(debug.LEVEL_INFO, "BRAILLECRAFT SAY: %r" % text, True)
        if hasattr(self, "present_message"):
            self.present_message(text)
        elif hasattr(self, "presentMessage"):
            self.presentMessage(text)
        elif presentation_manager is not None:
            presentation_manager.get_manager().present_message(text)

    def _speak(self, text):
        """Speaks `text` only."""
        if not text:
            return
        debug.print_message(debug.LEVEL_INFO, "BRAILLECRAFT SPEAK: %r" % text, True)
        if hasattr(self, "speak_message"):
            self.speak_message(text)
        elif hasattr(self, "speakMessage"):
            self.speakMessage(text)
        elif presentation_manager is not None:
            presentation_manager.get_manager().speak_message(text)

    # --- the document ---

    def _editor(self, say=True):
        """The editing area with focus, or None (then "not in document window" is said)."""
        obj = focus_manager.get_manager().get_locus_of_focus()
        if obj is not None and (AXObject.get_name(obj) or "") == EDITOR_NAME:
            return obj
        if say:
            self._say("not in document window")
        return None

    def _line(self, editor):
        """The caret line's text and the caret's column from 1."""
        offset = AXText.get_caret_offset(editor)
        line, start, _end = AXText.get_line_at_offset(editor, offset)
        return line.rstrip("\n"), offset - start + 1

    def _twice(self, key):
        """True when `key` was pressed again within 0.7 seconds (the second form of a key)."""
        import time
        now = time.monotonic()
        again = self._last_key == key and now - self._last_key_time < 0.7
        self._last_key = key
        self._last_key_time = now
        return again

    # --- the reading keys ---

    def speak_trans_line(self, script=None, event=None):
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

    def say_style_name(self, script=None, event=None):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        if self._twice("style") and status.in_table and status.row and status.col:
            self._say("Row %d Column %d" % (status.row[0], status.col[0]))
            return True
        self._say(status.style or "not in document window")
        return True

    def say_location(self, script=None, event=None):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        if status.coded:
            self._say("location not available in coded view, press Alt F3 to turn codes off")
            return True
        self._say("P%s L%s C%s" % (status.page, status.line, status.column))
        return True

    def say_doc_name_and_status(self, script=None, event=None):
        ed = self._editor()
        if ed is None:
            return True
        docs = DocParts(ed)
        self._say(docs.title + ": " + docs.status_text)
        return True

    def say_line_position(self, script=None, event=None):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        line, col = self._line(ed)
        pos = status.column if status.column is not None else col
        to_end = len(line) - pos + 1
        if self._twice("position"):
            self._say("%d to last nonspace character" % (len(line.rstrip()) - pos))
        elif to_end == 0:
            self._say("position %d end of line" % pos)
        else:
            self._say("position %d" % pos)
        return True

    def say_total_and_left(self, script=None, event=None):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        if status.coded:
            self._say("coded view")
            return True
        line, _col = self._line(ed)
        if self._twice("length"):
            self._say("%d left spaces" % (len(line) - len(line.lstrip())))
        else:
            self._say("%d total characters." % len(line))
        return True

    def speak_last_word(self, script=None, event=None):
        ed = self._editor()
        if ed is None:
            return True
        docs = DocParts(ed)
        status = Status(docs.status_text)
        if status.coded:
            self._say("location not available in coded view, press Alt F3 to turn codes off")
            return True
        line, _col = self._line(ed)
        word = re.split(r"[ -]", line.rstrip())[-1]
        self._say(cells_of(word) if docs.is_braille else (word or "blank"))
        return True

    def trans_line_cursor_toggle(self, script=None, event=None):
        """The review (flat review) location to the translated line, and back to the caret."""
        if self.review_on_trans:
            self.review_on_trans = False
            self._say("review cursor at caret")
            return True
        ed = self._editor()
        if ed is None:
            return True
        docs = DocParts(ed)
        if docs.trans_showing == 0 or docs.trans is None:
            self._say("translated line not shown; press Control F8 to show it")
            return True
        self.review_on_trans = True
        self._say("translated line: " + (docs.trans_text if docs.is_braille else cells_of(docs.trans_text)))
        return True

    def read_line_in_braille_toggle(self, script=None, event=None):
        self.line_level = {0: 2, 2: 1, 1: 0}[self.line_level]
        self._say(["silent for up and down in braille", "read line as braille characters for up and down in braille",
            "use translated line for up and down in braille"][self.line_level])
        return True

    def read_style_change_toggle(self, script=None, event=None):
        if self._editor(say=False) is None:
            return False
        style, skips = self.announce_styles, self.announce_skips
        if not style and not skips:
            style = True
        elif style and not skips:
            skips = True
        elif style and skips:
            style = False
        else:
            skips = False
        self.announce_styles, self.announce_skips = style, skips
        self._say(("Announcements: Change of style on." if style else "Announcements: change of style off.")
            + " " + ("Skipped lines on." if skips else "Skipped lines off."))
        return True

    def braille_doc_input_echo_toggle(self, script=None, event=None):
        if self._editor(say=False) is None:
            return False
        self.input_echo = (self.input_echo + 1) % 5
        self._say("Echo for Braille entry is " + ["same as Orca key echo", "silent", "characters", "words",
            "characters and words"][self.input_echo])
        return True

    def speak_completed_words_toggle(self, script=None, event=None):
        if self.input_echo in (3, 4):
            self.input_echo = 1 if self.input_echo == 3 else 2
            self._say("echo completed braille words off")
        else:
            self.input_echo = 4 if self.input_echo == 2 else 3
            self._say("echo completed braille words on")
        return True

    def check_braille_keyboard(self, script=None, event=None):
        ed = self._editor()
        if ed is None:
            return True
        status = Status(DocParts(ed).status_text)
        self._say("braille keyboard connected" if status.keyboard else "braille keyboard not connected")
        return True

    def _table_entry(self, ed, docs, status):
        line, _col = self._line(ed)
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

    def say_table_entry(self, script=None, event=None):
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
        """Passes the arrow to the program and speaks the entry the caret lands on."""
        ed = self._editor(say=False)
        if ed is None:
            return False
        from gi.repository import GLib
        try:
            import subprocess
            subprocess.Popen(["xdotool", "key", "--clearmodifiers", "alt+" + keysym])
        except Exception:
            return False

        def after():
            docs = DocParts(ed)
            status = Status(docs.status_text)
            if status.in_table:
                self._table_entry(ed, docs, status)
            elif speak_line:
                line, _col = self._line(ed)
                self._say(cells_of(line.rstrip()) if docs.is_braille and self.line_level == 1 else (line or "blank"))
            return False
        GLib.timeout_add(150, after)
        return True

    def next_row_entry(self, script=None, event=None):
        return self._table_move("Down", True)

    def prior_row_entry(self, script=None, event=None):
        return self._table_move("Up", True)

    def next_column_entry(self, script=None, event=None):
        return self._table_move("Right", False)

    def prior_column_entry(self, script=None, event=None):
        return self._table_move("Left", False)

    def hot_key_help(self, script=None, event=None):
        self._say(HELP)
        return True

    # --- the keys ---

    COMMANDS = [
        ("braillecraft_translated_line", "speak_trans_line", "Speaks the translated line", "7"),
        ("braillecraft_style", "say_style_name", "Speaks the current style; twice: the table row and column", "8"),
        ("braillecraft_location", "say_location", "Speaks the location: page, line and column", "9"),
        ("braillecraft_document_name", "say_doc_name_and_status", "Speaks the document name and the status line", "0"),
        ("braillecraft_line_position", "say_line_position", "Speaks the position on the line; twice: characters to the end", "comma"),
        ("braillecraft_line_length", "say_total_and_left", "Speaks the characters on the line; twice: the left spaces", "period"),
        ("braillecraft_last_word", "speak_last_word", "Speaks the last word on the line", "End"),
        ("braillecraft_translated_line_cursor", "trans_line_cursor_toggle", "Reads the translated line as the review place, and back", "semicolon"),
        ("braillecraft_line_level", "read_line_in_braille_toggle", "Cycles the speaking level for up and down in a braille document", "u"),
        ("braillecraft_announcements", "read_style_change_toggle", "Cycles the style change and skipped line announcements", "i"),
        ("braillecraft_input_echo", "braille_doc_input_echo_toggle", "Cycles the echo for entry in a braille document", "p"),
        ("braillecraft_completed_words", "speak_completed_words_toggle", "Toggles reading the completed braille word at a space", "w"),
        ("braillecraft_braille_keyboard", "check_braille_keyboard", "Says whether a braille keyboard is connected", "c"),
        ("braillecraft_next_row", "next_row_entry", "Moves down one table row and speaks the entry", "Down"),
        ("braillecraft_prior_row", "prior_row_entry", "Moves up one table row and speaks the entry", "Up"),
        ("braillecraft_next_column", "next_column_entry", "Moves right one table column and speaks the entry", "Right"),
        ("braillecraft_prior_column", "prior_column_entry", "Moves left one table column and speaks the entry", "Left"),
        ("braillecraft_table_entry", "say_table_entry", "Speaks the current table entry", "t"),
        ("braillecraft_help", "hot_key_help", "Lists the BrailleCraft keys", "h"),
    ]

    # Orca 48 and 49.
    def setup_input_event_handlers(self):
        super().setup_input_event_handlers()
        for name, method, description, _key in self.COMMANDS:
            self.input_event_handlers[name] = input_event.InputEventHandler(getattr(Script, method), description)

    def get_app_key_bindings(self):
        bindings = keybindings.KeyBindings()
        for name, _method, _description, key in self.COMMANDS:
            bindings.add(keybindings.KeyBinding(key, keybindings.DEFAULT_MODIFIER_MASK,
                keybindings.ALT_MODIFIER_MASK, self.input_event_handlers[name]))
        return bindings

    # Orca 50.
    def set_up_commands(self):
        super().set_up_commands()
        if OLD_API or command_manager is None:
            return
        manager = command_manager.get_manager()
        for name, method, description, key in self.COMMANDS:
            try:
                manager.register_command(name, getattr(self, method), description, key,
                    keybindings.ALT_MODIFIER_MASK, 1, GROUP)
            except Exception as e:
                debug.print_message(debug.LEVEL_WARNING, "BRAILLECRAFT: command %s: %s" % (name, e), True)

    # --- braille documents: cells spoken, contraction off, announcements ---

    def _in_braille_doc(self, obj):
        return obj is not None and (AXObject.get_name(obj) or "") == EDITOR_NAME and DocParts(obj).is_braille

    def _set_contracted(self, on):
        try:
            if hasattr(braille, "set_enable_contracted_braille"):
                braille.set_enable_contracted_braille(on)
            else:
                settings.enableContractedBraille = on
                braille.refresh()
        except Exception as e:
            debug.print_message(debug.LEVEL_WARNING, "BRAILLECRAFT: contracted braille: %s" % e, True)

    def locus_of_focus_changed(self, event, old_focus, new_focus):
        # Contracted braille off while a braille document's editing area has focus, as the JAWS scripts turn Grade 2
        # translation off there, so a display shows the document's cells; the user's choice comes back on leaving.
        if self._in_braille_doc(new_focus):
            if self.contracted_saved is None:
                self.contracted_saved = bool(getattr(settings, "enableContractedBraille", False))
                if self.contracted_saved:
                    self._set_contracted(False)
        elif self.contracted_saved is not None:
            if self.contracted_saved:
                self._set_contracted(True)
            self.contracted_saved = None
        super().locus_of_focus_changed(event, old_focus, new_focus)

    def deactivate(self):
        if self.contracted_saved is not None:
            if self.contracted_saved:
                self._set_contracted(True)
            self.contracted_saved = None
        super().deactivate()

    def _announce_moves(self, status):
        if not status.coded and status.page != self.last_page and self.last_page is not None and status.page is not None:
            self._speak("Page " + status.page)
        if (self.announce_skips and status.line is not None and self.last_line is not None
                and status.page == self.last_page and abs(status.line - self.last_line) > 1):
            self._speak("line skip")
        if self.announce_styles and status.style and self.last_style is not None and status.style != self.last_style:
            self._speak(status.style)
        if status.in_table and not self.in_table:
            self._speak("entering table")
        elif self.in_table and not status.in_table:
            self._speak("exiting table")
        self.last_page = status.page
        self.last_line = status.line
        self.last_style = status.style
        self.in_table = status.in_table

    def _say_character_cells(self, obj):
        """True when the character at the caret was spoken as a cell name (a braille document)."""
        if not self.speak_cells or not self._in_braille_doc(obj):
            return False
        offset = AXText.get_caret_offset(obj)
        ch = AXText.get_character_at_offset(obj, offset)[0]
        if not ch or ch in "\r\n":
            return False
        self._speak(cell_name(ch))
        return True

    def _say_word_cells(self, obj):
        if not self.speak_cells or not self._in_braille_doc(obj):
            return False
        word = AXText.get_word_at_offset(obj)[0].strip("\r\n")
        if not word:
            return False
        self._speak(cells_of(word.rstrip()))
        return True

    def _say_line_cells(self, obj):
        """True when the caret line was spoken per the speaking level (a braille document)."""
        if not self._in_braille_doc(obj):
            return False
        docs = DocParts(obj)
        self._announce_moves(Status(docs.status_text))
        line, _col = self._line(obj)
        if self.line_level == 1:
            self._speak("blank" if not line.strip() else cells_of(line.rstrip()))
            return True
        if self.line_level == 2:
            text = docs.trans_text if docs.trans_showing == 1 else ""
            self._speak(text if text.strip() else ("blank" if not line.strip() else cells_of(line.rstrip())))
            return True
        return True  # silent

    # Orca 50 names.
    def say_character(self, obj):
        if not self._say_character_cells(obj):
            super().say_character(obj)

    def say_word(self, obj):
        if not self._say_word_cells(obj):
            super().say_word(obj)

    def say_line(self, obj, offset=None):
        if offset is not None or not self._say_line_cells(obj):
            super().say_line(obj, offset)

    # Orca 48 and 49 names.
    def sayCharacter(self, obj):
        if not self._say_character_cells(obj):
            super().sayCharacter(obj)

    def sayWord(self, obj):
        if not self._say_word_cells(obj):
            super().sayWord(obj)

    def sayLine(self, obj, offset=None):
        if offset is not None or not self._say_line_cells(obj):
            super().sayLine(obj, offset)


HELP = ("BrailleCraft keys, each with Alt. 7, the translated line. 8, the current style; twice quickly in a table, the "
    "row and column. 9, the location: page, line and column. 0, the document name and status line. Comma, the "
    "position on the line; twice quickly, the characters to the end of the line. Period, the number of characters on "
    "the line; twice quickly, the left spaces. End, the last word on the line. Semicolon, the translated line as the "
    "review place and back. U, the speaking level for up and down in a braille document. I, the style change and "
    "skipped line announcements. P, the echo for entry in a braille document. W, the reading of the completed braille "
    "word at a space. C, whether a braille keyboard is connected. Up, Down, Left and Right, the table entry moves. T, "
    "the current table entry. H, this list.")
