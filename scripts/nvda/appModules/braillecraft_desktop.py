# BrailleCraft NVDA add-on: the app module for braillecraft-desktop.exe.
#
# It gives NVDA what the BrailleCraft JAWS scripts (scripts/jaws) give JAWS, key for key and message for message:
#   Alt+7 the translated line; Alt+8 the current style (twice: table row and column); Alt+9 the location (page, line,
#   column); Alt+0 the document name and the status line; Alt+comma the position on the line (twice: characters to
#   the end); Alt+period the characters on the line (twice: the left spaces); Alt+End the last word on the line;
#   Alt+semicolon the review cursor to the translated line and back; Alt+U the speaking level for up and down in a
#   braille document (silent, braille characters, the translated line); Alt+I the style change and skipped line
#   announcements; Alt+P the echo for entry in a braille document; Alt+W the reading of a completed braille word at
#   a space; Alt+C whether a braille keyboard is connected; Alt+arrows the table entry moves with the entry spoken;
#   NVDA+Alt+UpArrow the table entry; F2, Alt+F3, Control+F8 and Control+H passed to the program with the new state
#   announced (six-key entry, the coded view, the translated line, highlighting); Shift+F3 with the changed text
#   spoken; NVDA+H the key help and NVDA+Shift+H the troubleshooting notes.
# In a braille document, typed characters and the characters, words and lines the caret moves over are spoken as
# braille cell names (the North American ASCII, Russian and Unicode braille pattern names of the JAWS scripts), a
# page change says the page, and style changes and skipped lines are announced when turned on; in the coded view of
# a print document a code is read as "code" and its letters, and a style start or end as "begin style" or "end
# style" with its name.
#
# How it finds the program's windows (spec 17): the editing area is the window of class BrailleCraftEditor with the
# accessible name "Document Editing Window"; its document window holds, after it, the status line (a static text:
# "Current Style: x  Page:n  Line:n  Column:n  c  Keys:1", or the table text in a table, with Page, Line and Column
# left out in the coded view, Keys:6 for six-key entry, H after it for highlighting and + for a connected braille
# keyboard) and the translated line (a read-only edit field, shown while View, Translated Line is on, "Error message
# 0" when the document has no translation); the document window's title holds "Document", and "Braille Document"
# for a braille document.

import ctypes
import re

import api
import appModuleHandler
import braille
import config
import speech
import textInfos
import ui
import wx
from NVDAObjects import NVDAObject
from NVDAObjects.IAccessible import getNVDAObjectFromEvent
from gui import guiHelper
from gui.settingsDialogs import SettingsPanel as _SettingsPanel
from scriptHandler import getLastScriptRepeatCount, script
import winUser

CATEGORY = "BrailleCraft"
EDITOR_CLASS = "BrailleCraftEditor"

# --- settings (the JAWS scripts' Quick Settings choices) ---

CONFIG_SECTION = "braillecraft"
config.conf.spec[CONFIG_SECTION] = {
	# For up and down in a braille document: 0 silent, 1 the line as braille characters, 2 the translated line.
	"brailleLineSpeakingLevel": "integer(default=2,min=0,max=2)",
	# Echo for entry in a braille document: 0 as NVDA's typing echo, 1 silent, 2 characters, 3 words, 4 both.
	"brailleDocInputEcho": "integer(default=4,min=0,max=4)",
	"speakBrailleCharsAsLocalBrailleSigns": "boolean(default=True)",
	"announceStyleChanges": "boolean(default=False)",
	"announceForcedLineSkips": "boolean(default=False)",
	"showRequestedStatusInfoOnBrailleDisplay": "boolean(default=True)",
	"speakPrintLinesByWord": "boolean(default=False)",
	"useStatusLineForUnicodeValues": "boolean(default=False)",
}

LINE_LEVELS = ["silent for up and down in braille", "read line as braille characters for up and down in braille",
	"use translated line for up and down in braille"]
ECHO_NAMES = ["same as NVDA typing echo", "silent", "characters", "words", "characters and words"]


def setting(name):
	return config.conf[CONFIG_SECTION][name]


def say(text):
	"""Requested information: spoken, and shown on the braille display while that choice is on."""
	if setting("showRequestedStatusInfoOnBrailleDisplay"):
		ui.message(text)
	else:
		speech.speakMessage(text)


def setSetting(name, value):
	config.conf[CONFIG_SECTION][name] = value


# --- braille cell names (the JAWS scripts' tables, one name per character value) ---

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


def cellName(ch):
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


def cellsOf(text):
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
			out.append(cellName(text[i]))
			i += 1
	return ", ".join(out)


# --- the program's windows ---

user32 = ctypes.windll.user32
GW_HWNDNEXT = 2
GW_CHILD = 5
WM_GETTEXT = 0x000D
WM_GETTEXTLENGTH = 0x000E
SMTO_ABORTIFHUNG = 0x0002


def _className(hwnd):
	buf = ctypes.create_unicode_buffer(256)
	user32.GetClassNameW(hwnd, buf, 256)
	return buf.value


def _windowText(hwnd):
	"""The text of a window in the program, read through WM_GETTEXT (the title of a frame, the text of a static text
	or an edit field), with a time limit so a busy program never holds NVDA."""
	res = ctypes.c_ulong()
	if not user32.SendMessageTimeoutW(hwnd, WM_GETTEXTLENGTH, 0, 0, SMTO_ABORTIFHUNG, 500, ctypes.byref(res)):
		return ""
	n = int(res.value)
	buf = ctypes.create_unicode_buffer(n + 1)
	user32.SendMessageTimeoutW(hwnd, WM_GETTEXT, n + 1, buf, SMTO_ABORTIFHUNG, 500, ctypes.byref(res))
	return buf.value


class DocWindows:
	"""The windows of the document whose editing area is `editor`."""

	def __init__(self, editor):
		self.editor = editor
		self.frame = user32.GetParent(editor)
		self.status = 0
		self.trans = 0
		h = user32.GetWindow(self.frame, GW_CHILD)
		while h:
			cls = _className(h)
			if cls == "Static" and not self.status and _windowText(h).startswith("Current Style"):
				self.status = h
			elif cls == "Static" and not self.status and "Row" in _windowText(h):
				self.status = h
			elif cls in ("Edit", "RICHEDIT50W") and not self.trans:
				self.trans = h
			h = user32.GetWindow(h, GW_HWNDNEXT)

	@property
	def title(self):
		return _windowText(self.frame) if self.frame else ""

	@property
	def isBraille(self):
		return "Braille Document" in self.title

	@property
	def statusText(self):
		return _windowText(self.status) if self.status else ""

	@property
	def transShowing(self):
		"""1 when the translated line is shown, -1 when it shows that there is no translation, 0 when hidden."""
		if not self.trans or not user32.IsWindowVisible(self.trans):
			return 0
		return -1 if _windowText(self.trans) == "Error message 0" else 1

	@property
	def transText(self):
		return _windowText(self.trans) if self.trans else ""


class Status:
	"""The status line taken apart: page, line and column (None in the coded view), the style, the table place, the
	keys field."""

	def __init__(self, text):
		self.text = text
		m = re.search(r"Page:\s*(\S+)", text)
		self.page = m.group(1) if m else None
		m = re.search(r"Line:\s*(\d+)", text)
		self.line = int(m.group(1)) if m else None
		m = re.search(r"Column:\s*(\d+)", text)
		self.column = int(m.group(1)) if m else None
		m = re.search(r"Current Style:\s*(.*?)(?:\s\s|$)", text)
		self.style = m.group(1).strip() if m else ""
		m = re.search(r"Row\s*(\d+)/(\d+)", text)
		self.row = (int(m.group(1)), int(m.group(2))) if m else None
		m = re.search(r"Col\s*(\d+)/(\d+)", text)
		self.col = (int(m.group(1)), int(m.group(2))) if m else None
		self.inTable = bool(self.row and self.col) or "Matrix" in text
		# In a table the status line carries the table text in place of the style.
		if self.inTable and not self.style:
			self.style = text.split("  ")[0].strip()
		m = re.search(r"Keys:(\d)([Hh+]?)", text)
		self.sixKey = bool(m and m.group(1) == "6")
		self.highlight = bool(m and m.group(2).upper() == "H")
		self.keyboard = bool(m and m.group(2) == "+")
		m = re.search(r"U\+([0-9A-Fa-f]{4})", text)
		self.unicode = m.group(1).upper() if m else None
		m = re.search(r"\s(\[[^\]]*\])(?:\s|$)", text)
		self.bracketed = m.group(1) if m else None

	@property
	def coded(self):
		# The coded view leaves Page, Line and Column off the status line; a status line that was not found is not
		# taken for the coded view.
		return self.page is None and bool(self.text)


# --- the editing area as NVDA sees it ---

class EditingWindow(NVDAObject):
	"""The program's drawn editing window (its UI Automation text), with the braille document and coded view
	behaviour of the JAWS scripts over NVDA's own editable text handling."""

	def _docs(self):
		return DocWindows(self.windowHandle)

	def _lineText(self, info=None):
		try:
			if info is None:
				info = self.makeTextInfo(textInfos.POSITION_CARET)
			info = info.copy()
			info.expand(textInfos.UNIT_LINE)
			return info.text.rstrip("\r\n")
		except Exception:
			return ""

	def _charAt(self, info=None):
		try:
			if info is None:
				info = self.makeTextInfo(textInfos.POSITION_CARET)
			info = info.copy()
			info.expand(textInfos.UNIT_CHARACTER)
			return info.text
		except Exception:
			return ""

	def _toEndOfLine(self, info=None):
		try:
			if info is None:
				info = self.makeTextInfo(textInfos.POSITION_CARET)
			end = info.copy()
			end.expand(textInfos.UNIT_LINE)
			end.setEndPoint(info, "startToStart")
			return end.text.rstrip("\r\n")
		except Exception:
			return ""

	def _decipher(self, info):
		"""In the coded view: a code at the caret is spoken as "code" and its letters, a style start or end as
		"begin style" or "end style" and its name. True when something was spoken."""
		c = self._charAt(info)
		rest = self._toEndOfLine(info)
		if c == "[" and "]" in rest:
			name = rest[1:rest.index("]")]
			speech.speakMessage("code")
			speech.speakSpelling(name)
			return True
		if c == "<" and ">" in rest:
			name = rest[1:rest.index(">")]
			if name.startswith("/"):
				speech.speakMessage("end style " + name[1:])
			else:
				speech.speakMessage("begin style " + name)
			return True
		return False

	def _caretScriptPostMovedHelper(self, speakUnit, gesture, info=None):
		app = self.appModule
		docs = self._docs()
		status = Status(docs.statusText)
		app.announceMoves(status, speakUnit)
		if info is None:
			try:
				info = self.makeTextInfo(textInfos.POSITION_CARET)
			except Exception:
				info = None
		braille_doc = docs.isBraille
		if speakUnit == textInfos.UNIT_LINE and braille_doc:
			level = setting("brailleLineSpeakingLevel")
			if info is not None:
				api.setReviewPosition(info.copy(), isCaret=True)
			line = self._lineText(info)
			if level == 1:
				speech.speakMessage("blank" if not line.strip() else cellsOf(line.rstrip()))
			elif level == 2:
				text = docs.transText if docs.transShowing == 1 else ""
				speech.speakMessage(text if text.strip() else ("blank" if not line.strip() else cellsOf(line.rstrip())))
			braille.handler.handleCaretMove(self)
			return
		if speakUnit == textInfos.UNIT_CHARACTER and status.coded and not braille_doc and info is not None:
			if self._decipher(info):
				api.setReviewPosition(info.copy(), isCaret=True)
				braille.handler.handleCaretMove(self)
				return
		if speakUnit == textInfos.UNIT_CHARACTER and braille_doc and setting("speakBrailleCharsAsLocalBrailleSigns"):
			c = self._charAt(info)
			if c and c not in "\r\n":
				if info is not None:
					api.setReviewPosition(info.copy(), isCaret=True)
				speech.speakMessage(cellName(c))
				braille.handler.handleCaretMove(self)
				return
		if speakUnit == textInfos.UNIT_CHARACTER and setting("useStatusLineForUnicodeValues"):
			# The status line's character field: U+XXXX (View Preferences verbose labels), [-] a hard hyphen, ['] a
			# hard space, spoken as the JAWS scripts speak them.
			if status.bracketed == "[']":
				speech.speakMessage("hard space")
				return
			if status.bracketed == "[-]":
				speech.speakMessage("assisted hyphen")
				return
			if status.unicode:
				super()._caretScriptPostMovedHelper(speakUnit, gesture, info)
				speech.speakMessage("U+" + status.unicode + " Hex")
				return
		if speakUnit == textInfos.UNIT_WORD and braille_doc and setting("speakBrailleCharsAsLocalBrailleSigns"):
			try:
				w = info.copy()
				w.expand(textInfos.UNIT_WORD)
				word = w.text.strip("\r\n")
			except Exception:
				word = ""
			if word:
				api.setReviewPosition(info.copy(), isCaret=True)
				speech.speakMessage(cellsOf(word.rstrip()))
				braille.handler.handleCaretMove(self)
				return
		if speakUnit == textInfos.UNIT_LINE and status.coded and not braille_doc and setting("speakPrintLinesByWord"):
			line = self._lineText(info)
			if info is not None:
				api.setReviewPosition(info.copy(), isCaret=True)
			speech.speakMessage(re.sub(r"\[([^\]]*)\]", lambda m: " code " + " ".join(m.group(1)) + " ", line) or "blank")
			braille.handler.handleCaretMove(self)
			return
		super()._caretScriptPostMovedHelper(speakUnit, gesture, info)

	def event_typedCharacter(self, ch):
		docs = self._docs()
		if not docs.isBraille:
			super().event_typedCharacter(ch)
			return
		echo = setting("brailleDocInputEcho")
		if echo == 0:
			super().event_typedCharacter(ch)
			return
		if echo in (2, 4) and ch not in "\r\n":
			speech.speakMessage(cellName(ch))
		if ch == "\r" or ch == "\n":
			speech.speakMessage("new line")
		if echo in (3, 4) and ch in " \t":
			self.appModule.speakCompletedBrailleWord(docs)


# --- the app module ---

class AppModule(appModuleHandler.AppModule):
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)
		self.lastPage = None
		self.lastLine = None
		self.lastStyle = None
		self.inTable = False
		self.lastTrans = ""
		self.reviewOnTransLine = False

	def chooseNVDAObjectOverlayClasses(self, obj, clsList):
		if obj.windowClassName == EDITOR_CLASS:
			clsList.insert(0, EditingWindow)

	# The editing area with focus, or None (then "not in document window" is said).
	def _editor(self, speak=True):
		obj = api.getFocusObject()
		if obj and obj.windowClassName == EDITOR_CLASS and obj.appModule is self:
			return obj
		if speak:
			say("not in document window")
		return None

	def announceMoves(self, status, speakUnit):
		"""After a caret move: the page when it changed, a style change and a skipped line when those are on."""
		if not status.coded and status.page != self.lastPage and self.lastPage is not None and status.page is not None:
			speech.speakMessage("Page " + status.page)
		if (setting("announceForcedLineSkips") and speakUnit == textInfos.UNIT_LINE and status.line is not None
				and self.lastLine is not None and status.page == self.lastPage and abs(status.line - self.lastLine) > 1):
			speech.speakMessage("line skip")
		if setting("announceStyleChanges") and status.style and self.lastStyle is not None and status.style != self.lastStyle:
			speech.speakMessage(status.style)
		if status.inTable and not self.inTable:
			speech.speakMessage("entering table")
		elif self.inTable and not status.inTable:
			speech.speakMessage("exiting table")
		self.lastPage = status.page
		self.lastLine = status.line
		self.lastStyle = status.style
		self.inTable = status.inTable

	def speakCompletedBrailleWord(self, docs):
		"""At a space in a braille document: the word the translated line gained (its print form)."""
		new = docs.transText if docs.transShowing == 1 else ""
		old = self.lastTrans
		self.lastTrans = new
		if not new.strip():
			return
		i = 0
		while i < len(old) and i < len(new) and old[i] == new[i]:
			i += 1
		added = new[i:].strip()
		if not added:
			added = new.rstrip().split(" ")[-1]
		if added:
			speech.speakMessage(added)

	# --- the reading keys ---

	@script(description="Speaks the translated line (print for a braille document, braille cells for a print one)",
		category=CATEGORY, gesture="kb:alt+7")
	def script_speakTransLine(self, gesture):
		ed = self._editor()
		if not ed:
			return
		docs = DocWindows(ed.windowHandle)
		if docs.transShowing == 0:
			# Turned on for the reading, as the JAWS script does with Control+F8.
			from keyboardHandler import KeyboardInputGesture
			KeyboardInputGesture.fromName("control+f8").send()
			say("translated line on")
			wx.CallLater(300, self._speakTransLineNow, ed.windowHandle)
			return
		self._speakTransLineNow(ed.windowHandle)

	def _speakTransLineNow(self, hwnd):
		docs = DocWindows(hwnd)
		if docs.transShowing == -1:
			say("translation not available")
			return
		text = docs.transText
		if not text.strip():
			say("blank")
			return
		if docs.isBraille:
			# Right-to-left scripts come reversed in the window; they are reversed back and spelled, as in JAWS.
			if any(0x0590 <= ord(c) <= 0x06FF or 0xFB1D <= ord(c) <= 0xFB4F or 0xFE00 <= ord(c) <= 0xFEFF for c in text):
				speech.speakSpelling(text[::-1])
			else:
				say(text)
		else:
			say(cellsOf(text))

	@script(description="Speaks the current style; twice: the row and column in a table", category=CATEGORY,
		gesture="kb:alt+8")
	def script_sayStyleName(self, gesture):
		ed = self._editor()
		if not ed:
			return
		status = Status(DocWindows(ed.windowHandle).statusText)
		if getLastScriptRepeatCount() > 0 and status.inTable and status.row and status.col:
			say("Row %d Column %d" % (status.row[0], status.col[0]))
			return
		say(status.style or "not in document window")

	@script(description="Speaks the location: page, line and column", category=CATEGORY, gesture="kb:alt+9")
	def script_sayLocation(self, gesture):
		ed = self._editor()
		if not ed:
			return
		status = Status(DocWindows(ed.windowHandle).statusText)
		if status.coded:
			say("location not available in coded view, press Alt+F3 to turn codes off")
			return
		say("P%s L%s C%s" % (status.page, status.line, status.column))

	@script(description="Speaks the document name and the status line", category=CATEGORY, gesture="kb:alt+0")
	def script_sayDocNameAndStatusLine(self, gesture):
		ed = self._editor()
		if not ed:
			return
		docs = DocWindows(ed.windowHandle)
		say(docs.title + ": " + docs.statusText)

	@script(description="Speaks the position on the line; twice: the characters to the last non-space character",
		category=CATEGORY, gesture="kb:alt+,")
	def script_sayLinePosition(self, gesture):
		ed = self._editor()
		if not ed:
			return
		status = Status(DocWindows(ed.windowHandle).statusText)
		line = ed._lineText()
		pos = status.column if status.column is not None else len(line) - len(ed._toEndOfLine()) + 1
		toEnd = len(line) - pos + 1
		if getLastScriptRepeatCount() == 0:
			say("position %d end of line" % pos if toEnd == 0 else "position %d" % pos)
		else:
			say("%d to last nonspace character" % (len(line.rstrip()) - pos))

	@script(description="Speaks the number of characters on the line; twice: the left spaces", category=CATEGORY,
		gesture="kb:alt+.")
	def script_sayTotalAndLeftOffset(self, gesture):
		ed = self._editor()
		if not ed:
			return
		status = Status(DocWindows(ed.windowHandle).statusText)
		if status.coded:
			say("coded view")
			return
		line = ed._lineText()
		if getLastScriptRepeatCount() == 0:
			say("%d total characters." % len(line))
		else:
			say("%d left spaces" % (len(line) - len(line.lstrip())))

	@script(description="Speaks the last word on the line", category=CATEGORY, gesture="kb:alt+end")
	def script_speakLastWord(self, gesture):
		ed = self._editor()
		if not ed:
			return
		docs = DocWindows(ed.windowHandle)
		status = Status(docs.statusText)
		if status.coded:
			say("location not available in coded view, press Alt+F3 to turn codes off")
			return
		word = re.split(r"[ -]", ed._lineText().rstrip())[-1]
		say(cellsOf(word) if docs.isBraille else word)

	@script(description="Moves the review cursor to the start of the translated line, and back to the caret",
		category=CATEGORY, gesture="kb:alt+;")
	def script_transLineCursorToggle(self, gesture):
		focus = api.getFocusObject()
		if self.reviewOnTransLine:
			self.reviewOnTransLine = False
			api.setNavigatorObject(focus)
			try:
				api.setReviewPosition(focus.makeTextInfo(textInfos.POSITION_CARET))
			except Exception:
				pass
			say("review cursor at caret")
			return
		ed = self._editor()
		if not ed:
			return
		docs = DocWindows(ed.windowHandle)
		if docs.transShowing == 0:
			from keyboardHandler import KeyboardInputGesture
			KeyboardInputGesture.fromName("control+f8").send()
			say("turning on translated line")
			wx.CallLater(300, self._reviewToTransLine, ed.windowHandle)
			return
		self._reviewToTransLine(ed.windowHandle)

	def _reviewToTransLine(self, hwnd):
		docs = DocWindows(hwnd)
		if docs.transShowing == 0 or not docs.trans:
			say("translated line not available")
			return
		obj = getNVDAObjectFromEvent(docs.trans, winUser.OBJID_CLIENT, 0)
		if not obj:
			say("translated line not available")
			return
		api.setNavigatorObject(obj)
		try:
			api.setReviewPosition(obj.makeTextInfo(textInfos.POSITION_FIRST))
		except Exception:
			pass
		self.reviewOnTransLine = True
		say("review cursor at start of translated line")

	@script(description="Cycles the speaking level for up and down in a braille document", category=CATEGORY,
		gesture="kb:alt+u")
	def script_readLineInBrailleDocToggle(self, gesture):
		level = setting("brailleLineSpeakingLevel")
		level = {0: 2, 2: 1, 1: 0}[level]
		setSetting("brailleLineSpeakingLevel", level)
		say(LINE_LEVELS[level])

	@script(description="Cycles the announcements of style changes and skipped lines", category=CATEGORY,
		gesture="kb:alt+i")
	def script_readStyleChangeToggle(self, gesture):
		if not self._editor(speak=False):
			gesture.send()
			return
		style = setting("announceStyleChanges")
		skips = setting("announceForcedLineSkips")
		if not style and not skips:
			style = True
		elif style and not skips:
			skips = True
		elif style and skips:
			style = False
		else:
			skips = False
		setSetting("announceStyleChanges", style)
		setSetting("announceForcedLineSkips", skips)
		say(("Announcements: Change of style on." if style else "Announcements: change of style off.")
			+ " " + ("Skipped lines on." if skips else "Skipped lines off."))

	@script(description="Cycles the echo for entry in a braille document", category=CATEGORY, gesture="kb:alt+p")
	def script_brailleDocInputEchoToggle(self, gesture):
		if not self._editor(speak=False):
			gesture.send()
			return
		echo = (setting("brailleDocInputEcho") + 1) % 5
		setSetting("brailleDocInputEcho", echo)
		say("Echo for Braille entry is " + ECHO_NAMES[echo])

	@script(description="Toggles the reading of a completed braille word when you press space", category=CATEGORY,
		gesture="kb:alt+w")
	def script_speakCompletedBrailleWordsToggle(self, gesture):
		echo = setting("brailleDocInputEcho")
		if echo in (3, 4):
			setSetting("brailleDocInputEcho", 1 if echo == 3 else 2)
			say("echo completed braille words off")
		else:
			setSetting("brailleDocInputEcho", 4 if echo == 2 else 3)
			say("echo completed braille words on")

	@script(description="Says whether a braille keyboard (Cosmo) is connected", category=CATEGORY, gesture="kb:alt+c")
	def script_checkCosmo(self, gesture):
		ed = self._editor()
		if not ed:
			return
		status = Status(DocWindows(ed.windowHandle).statusText)
		say("Cosmo connected" if status.keyboard else "Cosmo not connected")

	@script(description="Six-key entry on or off (F2), with the new state announced", category=CATEGORY, gesture="kb:f2")
	def script_f2Press(self, gesture):
		ed = self._editor(speak=False)
		gesture.send()
		if not ed:
			return
		wx.CallLater(150, self._sayAfterF2, ed.windowHandle)

	def _sayAfterF2(self, hwnd):
		status = Status(DocWindows(hwnd).statusText)
		say("6 key on" if status.sixKey else "6 key off")

	@script(description="Codes on or off (Alt+F3), with the new view announced", category=CATEGORY, gesture="kb:alt+f3")
	def script_altF3Press(self, gesture):
		ed = self._editor()
		if not ed:
			return
		status = Status(DocWindows(ed.windowHandle).statusText)
		say("formatted view" if status.coded else "coded view")
		gesture.send()

	@script(description="Translated line on or off (Control+F8), with the new state announced", category=CATEGORY,
		gesture="kb:control+f8")
	def script_controlF8Press(self, gesture):
		ed = self._editor()
		if not ed:
			return
		gesture.send()
		wx.CallLater(150, self._sayAfterControlF8, ed.windowHandle)

	def _sayAfterControlF8(self, hwnd):
		showing = DocWindows(hwnd).transShowing
		if showing == 1:
			say("translated line on")
		elif showing == 0:
			say("translated line off")
		else:
			say("translation not available")

	@script(description="Highlighting on or off (Control+H), with the new state announced", category=CATEGORY,
		gesture="kb:control+h")
	def script_highlight(self, gesture):
		ed = self._editor(speak=False)
		gesture.send()
		if not ed:
			return
		wx.CallLater(150, self._sayAfterControlH, ed.windowHandle)

	def _sayAfterControlH(self, hwnd):
		status = Status(DocWindows(hwnd).statusText)
		say("highlighting on" if status.highlight else "highlighting off")

	@script(description="Change Case (Shift+F3), with the changed text spoken", category=CATEGORY, gesture="kb:shift+f3")
	def script_changeCase(self, gesture):
		ed = self._editor(speak=False)
		gesture.send()
		if not ed:
			return
		wx.CallLater(200, self._sayAfterChangeCase, ed)

	def _sayAfterChangeCase(self, ed):
		try:
			sel = ed.makeTextInfo(textInfos.POSITION_SELECTION).text
		except Exception:
			sel = ""
		if sel.strip():
			speech.speakSpelling(sel) if len(sel) < 3 else say(sel)
		else:
			try:
				w = ed.makeTextInfo(textInfos.POSITION_CARET)
				w.expand(textInfos.UNIT_WORD)
				speech.speakSpelling(w.text.strip())
			except Exception:
				pass

	def _tableMove(self, gesture, speakLine):
		ed = self._editor(speak=False)
		gesture.send()
		if not ed:
			return
		wx.CallLater(150, self._sayAfterTableMove, ed, speakLine)

	def _sayAfterTableMove(self, ed, speakLine):
		docs = DocWindows(ed.windowHandle)
		status = Status(docs.statusText)
		if status.inTable:
			self._speakTableEntry(ed, docs, status)
		elif speakLine:
			line = ed._lineText()
			say(cellsOf(line.rstrip()) if docs.isBraille and setting("brailleLineSpeakingLevel") == 1 else (line or "blank"))

	def _speakTableEntry(self, ed, docs, status):
		line = ed._lineText()
		col = status.col[0] if status.col else 1
		if status.coded:
			parts = [p for p in re.split(r"\[<\]|\[>\]", line) if p.strip()]
		elif "Linear" in status.style:
			parts = [p for p in re.split(r"; |: ", line) if p.strip()]
		elif "Listed" in status.style:
			say(line)
			return
		else:
			parts = [p for p in re.split(r" \.\.|\.\.|  ", line) if p.strip()]
		if "Two-Page" in status.style and status.page and status.page[-1:] in "13579" and status.col:
			col = col - status.col[1] - 1
		entry = parts[col - 1] if 0 < col <= len(parts) else ""
		say(cellsOf(entry.strip()) if docs.isBraille else (entry.strip() or "blank"))

	@script(description="Moves down one row in a table and speaks the entry", category=CATEGORY, gesture="kb:alt+downArrow")
	def script_nextRowEntry(self, gesture):
		self._tableMove(gesture, True)

	@script(description="Moves up one row in a table and speaks the entry", category=CATEGORY, gesture="kb:alt+upArrow")
	def script_priorRowEntry(self, gesture):
		self._tableMove(gesture, True)

	@script(description="Moves right one column in a table and speaks the entry", category=CATEGORY,
		gesture="kb:alt+rightArrow")
	def script_nextColumnEntry(self, gesture):
		self._tableMove(gesture, False)

	@script(description="Moves left one column in a table and speaks the entry", category=CATEGORY,
		gesture="kb:alt+leftArrow")
	def script_priorColumnEntry(self, gesture):
		self._tableMove(gesture, False)

	@script(description="Speaks the current table entry", category=CATEGORY, gesture="kb:NVDA+alt+upArrow")
	def script_sayTableEntry(self, gesture):
		ed = self._editor()
		if not ed:
			return
		docs = DocWindows(ed.windowHandle)
		status = Status(docs.statusText)
		if status.inTable:
			self._speakTableEntry(ed, docs, status)

	@script(description="Shows the BrailleCraft key help", category=CATEGORY, gesture="kb:NVDA+h")
	def script_hotKeyHelp(self, gesture):
		ui.browseableMessage(HELP, "BrailleCraft keys")

	@script(description="Shows the BrailleCraft troubleshooting notes", category=CATEGORY, gesture="kb:NVDA+shift+h")
	def script_trouble(self, gesture):
		ui.browseableMessage(TROUBLE, "BrailleCraft and NVDA")


HELP = """To hear the translated line press Alt+7.
To hear the current style press Alt+8; press it twice quickly for the current row and column numbers in a table.
To hear the current location press Alt+9.
To hear the current document name and status line press Alt+0.
To toggle use of the translated line for up and down in braille, press Alt+U or use the BrailleCraft page of NVDA's settings.
To toggle reading of braille characters as English braille signs, use the BrailleCraft page of NVDA's settings.
To toggle style change and line skip announcements, press Alt+I or use the BrailleCraft settings.
To toggle reading lines by word in the print coded view, use the BrailleCraft settings.
To toggle the keyboard echo for entry in a braille document press Alt+P or use the BrailleCraft settings; some choices read the completed braille word when you press space.
To toggle the reading of the just-completed braille word when you press space, press Alt+W.
To move the review cursor to the translated line and back, press Alt+Semicolon.
To hear the current line position press Alt+Comma; twice quickly for the number of characters to the end of the line.
To hear the number of characters on the line press Alt+Period; twice quickly for the number of left spaces.
To hear the last word on the line press Alt+End.
To move in a table and hear the entry press Alt with an arrow key; to hear the current entry press NVDA+Alt+UpArrow.
To hear whether a braille keyboard is connected press Alt+C.
F2 (six-key entry), Alt+F3 (codes), Control+F8 (translated line) and Control+H (highlighting) announce their new state.
"""

TROUBLE = """You are using the BrailleCraft add-on for NVDA dated October 17, 2026.
1. NVDA reads the editing area through UI Automation. In BrailleCraft's Global menu, Internationalization, the item Support UI Automation must be on (it is on unless it was turned off).
2. If Global, Internationalization has Prefer Unicode Braille Patterns on, the braille document's text reaches NVDA as Unicode braille patterns; the add-on speaks those as cell names too.
3. The reading keys work in a document window; elsewhere they say "not in document window".
4. If you get wrong characters when entering braille from your braille display keyboard, press F2 to turn off six-key entry.
"""


# --- the settings page ---

class SettingsPanel(_SettingsPanel):
	title = "BrailleCraft"

	def makeSettings(self, sizer):
		helper = guiHelper.BoxSizerHelper(self, sizer=sizer)
		self.level = helper.addLabeledControl("Braille line speaking level (up and down in a braille document):",
			wx.Choice, choices=["Silent", "Braille characters", "Translated line"])
		self.level.SetSelection(setting("brailleLineSpeakingLevel"))
		self.echo = helper.addLabeledControl("Echo for entry in a braille document:", wx.Choice,
			choices=["Same as NVDA typing echo", "Silent", "Characters", "Words", "Characters and words"])
		self.echo.SetSelection(setting("brailleDocInputEcho"))
		self.signs = helper.addItem(wx.CheckBox(self, label="Speak braille characters as braille signs"))
		self.signs.SetValue(setting("speakBrailleCharsAsLocalBrailleSigns"))
		self.styles = helper.addItem(wx.CheckBox(self, label="Announce style changes"))
		self.styles.SetValue(setting("announceStyleChanges"))
		self.skips = helper.addItem(wx.CheckBox(self, label="Announce forced line skips"))
		self.skips.SetValue(setting("announceForcedLineSkips"))
		self.status = helper.addItem(wx.CheckBox(self, label="Show requested status info on the braille display"))
		self.status.SetValue(setting("showRequestedStatusInfoOnBrailleDisplay"))
		self.byWord = helper.addItem(wx.CheckBox(self, label="Speak print lines by word in the coded view"))
		self.byWord.SetValue(setting("speakPrintLinesByWord"))
		self.unicode = helper.addItem(wx.CheckBox(self, label="Use the status line for Unicode values"))
		self.unicode.SetValue(setting("useStatusLineForUnicodeValues"))

	def onSave(self):
		setSetting("brailleLineSpeakingLevel", self.level.GetSelection())
		setSetting("brailleDocInputEcho", self.echo.GetSelection())
		setSetting("speakBrailleCharsAsLocalBrailleSigns", self.signs.GetValue())
		setSetting("announceStyleChanges", self.styles.GetValue())
		setSetting("announceForcedLineSkips", self.skips.GetValue())
		setSetting("showRequestedStatusInfoOnBrailleDisplay", self.status.GetValue())
		setSetting("speakPrintLinesByWord", self.byWord.GetValue())
		setSetting("useStatusLineForUnicodeValues", self.unicode.GetValue())
