// BrailleMaker main window. The menu bar (crates/bm-shell/src/menu.rs) follows the menu table; each command arrives
// here as a "menu" event and is carried out by COMMANDS below. Commands that need parts not built yet (the
// translator, the BrailleMaker file format, printing and embossing) say so in a message box that names the build
// step that brings them. In a plain browser, with no native menu, the shortcuts are bound to the keyboard.
"use strict";

const tauri = window.__TAURI__ || null;
const invoke = (cmd, args) => (tauri && tauri.core ? tauri.core.invoke(cmd, args).catch(() => {}) : Promise.resolve());
// A shell call whose result or error matters (translation). Without the desktop shell, window.BM_TRANSLATOR may
// stand in (used by the page tests); otherwise translation is not available.
async function shellCall(cmd, args) {
  if (tauri && tauri.core) return tauri.core.invoke(cmd, args);
  if (window.BM_TRANSLATOR && window.BM_TRANSLATOR[cmd]) return window.BM_TRANSLATOR[cmd](args);
  throw new Error("Translation runs in the BrailleMaker desktop program.");
}

const editor = document.getElementById("doc");
const editorLabel = document.getElementById("doc-label");
const translatedPane = document.getElementById("translated-line");
const translatedText = document.getElementById("translated-text");
const statusStyle = document.getElementById("status-style");
const statusPage = document.getElementById("status-page");
const statusLineNumber = document.getElementById("status-line-number");
const statusColumn = document.getElementById("status-column");
const statusMode = document.getElementById("status-mode");
const announcer = document.getElementById("announce");

// ---------------------------------------------------------------- braille encodings
// A braille document holds its text in the characters of the encoding chosen in Global, Internationalization
// (North American ASCII braille by default), as braille is shown on screen: screen readers read those characters and the
// braille fonts draw them as cells. The translator works in cells (Unicode braille, U+2800 + dot pattern).

const ENCODING_LIST = window.BRAILLE_ENCODINGS || [];

function encodingCells(id) {
  const e = ENCODING_LIST.find((x) => x.id === id) || ENCODING_LIST.find((x) => x.id === "north-american");
  if (e && e.cells) return e.cells;
  // Fallback when encodings.js is missing: North American ASCII braille by dot pattern.
  return " a1b'k2l@cif/msp\"e3h9o6r^djg>ntq,*5<-u8v.%[$+x!&;:4\\0z7(_?w]#y)=".split("");
}

function cellChar(pattern) {
  const chars = encodingCells(settingsGlobal.encoding)[pattern];
  return chars ? chars[0] : String.fromCharCode(0x2800 + pattern);
}

// Unicode braille cells to the current encoding's characters; anything else (line breaks) passes through.
function cellsToText(cells) {
  let out = "";
  for (const ch of cells) {
    const cp = ch.codePointAt(0);
    out += cp >= 0x2800 && cp <= 0x283f ? cellChar(cp - 0x2800) : ch;
  }
  return out;
}

// The current encoding's characters back to cells, for back-translation.
function textToCells(text, encodingId = settingsGlobal.encoding) {
  const map = new Map();
  encodingCells(encodingId).forEach((chars, p) => { for (const c of chars) if (!map.has(c)) map.set(c, p); });
  let out = "";
  for (const ch of text) {
    let p = map.get(ch);
    if (p === undefined) p = map.get(ch.toLowerCase());
    if (p === undefined) p = map.get(ch.toUpperCase());
    out += p === undefined ? (ch === "\n" ? ch : "⠀") : String.fromCharCode(0x2800 + p);
  }
  return out;
}

// Codes such as [pg] or [es~para] are layout instructions, not text; the formatter (plan step 8) will carry them
// out. Until then they are left out of what is sent to the translator.
function stripCodes(text) { return text.replace(/\[[^\]\n]*\]/g, ""); }

const PLANNED = {
  save: "Saving needs the BrailleMaker document format, which is built in step 7 of the plan. Until then, copy the text out with Select All and Copy.",
  print: "Printing is built in step 9 of the plan.",
  emboss: "Embossing (Index and Enabling embossers, and emboss to file) is built in step 10 of the plan.",
  spell: "The spell checker (Hunspell dictionaries) is built after the core features of version 1.",
  help: "The BrailleMaker help is written with the features it describes; it is not in this build yet.",
  templates: "Templates are built with the BrailleMaker document format in step 7 of the plan.",
  pictures: "Pictures are built after the core features of version 1.",
  toc: "Table of contents generation needs the page formatter, built in step 8 of the plan.",
  tables: "The table list by region comes next; for now Document, Learning Tables chooses between uncontracted and contracted English (UEB).",
};

const settingsGlobal = {
  shortcuts: { grade0: "[cz]", cbc: "[cb]", flushRightLeader: "[fr;p~\"]", newPage: "[pg]", newRefPage: "[lea]" },
  sixKeySet: "fdsjkl",
  encoding: "north-american",
  defaultViews: { translatedFont: "braille", brailleFont: "braille", printFullPage: false, printCodes: false, printTranslated: false, brailleFullPage: false, brailleCodes: false, brailleTranslated: false, sixKey: false },
  dialogs: {},
};

const documents = [];
let active = -1;
const untitledCount = { print: 0, braille: 0 };
let lastFind = null;
let highlightAnchor = null;

// ---------------------------------------------------------------- documents

function kindName(kind) { return kind === "braille" ? "Braille Document" : "Print Document"; }
// Window names: "Untitled Print Document 1"; "name -- New Braille Document" for a translated or opened
// document not yet saved in the program's own format; "name -- Braille Document" once saved.
function fullName(d) {
  if (d.untitled) return d.name;
  return `${d.name} -- ${d.saved ? "" : "New "}${kindName(d.kind)}`;
}

function newDocument(kind, name, text = "") {
  const untitled = !name;
  if (untitled) name = `Untitled ${kind === "braille" ? "Braille" : "Print"} Document ${++untitledCount[kind]}`;
  const v = settingsGlobal.defaultViews;
  const braille = kind === "braille";
  const d = {
    name, kind, text, untitled, saved: false, selStart: 0, selEnd: 0, scrollTop: 0, dirty: false,
    // liblouis table used for File, Translate and the translated line (Document, Learning Tables changes it).
    table: "en-ueb-g2.ctb",
    settings: braille ? { cells: 40, lines: 25 } : { cells: 80, lines: 50 },
    view: {
      font: braille ? v.brailleFont : "print",
      fullPage: braille ? v.brailleFullPage : v.printFullPage,
      codes: braille ? v.brailleCodes : v.printCodes,
      translated: braille ? v.brailleTranslated : v.printTranslated,
      sixKey: braille && v.sixKey,
    },
    hyphen: "dashes",
    styles: [],
    lastStyle: null,
  };
  documents.push(d);
  activate(documents.length - 1);
  return d;
}

function current() { return documents[active]; }

function keepEditorState() {
  const d = current();
  if (!d) return;
  d.text = editor.value;
  d.selStart = editor.selectionStart;
  d.selEnd = editor.selectionEnd;
  d.scrollTop = editor.scrollTop;
}

function activate(index) {
  keepEditorState();
  active = index;
  const d = current();
  if (!d) {
    editor.value = "";
    editor.disabled = true;
    editorLabel.textContent = "No document open";
    setTitle("BrailleMaker");
    refreshMenus();
    updateStatus();
    return;
  }
  editor.disabled = false;
  editor.value = d.text;
  editor.setSelectionRange(d.selStart, d.selEnd);
  editor.scrollTop = d.scrollTop;
  editorLabel.textContent = fullName(d);
  setTitle(`BrailleMaker - [${fullName(d)}]`);
  lastTranslatedLine = null;
  applyView();
  refreshMenus();
  updateStatus();
  editor.focus();
}

function setTitle(text) {
  document.title = text;
  if (tauri && tauri.window) tauri.window.getCurrentWindow().setTitle(text).catch(() => {});
}

function refreshMenus() {
  invoke("set_documents", { names: documents.map(fullName), active: Math.max(active, 0) });
  const d = current();
  if (!d) return;
  invoke("set_check", { id: "view.fullPage", checked: d.view.fullPage });
  invoke("set_check", { id: { print: "view.printFont", braille: "view.brailleFont", simbraille: "view.simBrailleFont" }[d.view.font], checked: true });
  invoke("set_check", { id: "view.sixKey", checked: d.view.sixKey });
  invoke("set_check", { id: "view.codes", checked: d.view.codes });
  invoke("set_check", { id: "view.translatedLine", checked: d.view.translated });
  invoke("set_check", { id: `document.hyphen.${d.hyphen}`, checked: true });
}

function applyView() {
  const d = current();
  if (!d) return;
  editor.classList.remove("print-font", "braille-font", "simbraille-font");
  editor.classList.add(`${d.view.font}-font`);
  editor.style.fontSize = "";
  if (d.view.fullPage) {
    // Full-Page: the font is sized so one whole page (lines by cells) fits the window.
    const h = editor.clientHeight - 8;
    const w = editor.clientWidth - 12;
    const size = Math.max(6, Math.floor(Math.min(h / (d.settings.lines * 1.3), w / (d.settings.cells * 0.62))));
    editor.style.fontSize = `${size}px`;
  }
  translatedPane.hidden = !d.view.translated;
  statusMode.textContent = [d.view.sixKey ? "Six-Key" : "", d.view.codes ? "Codes" : "", highlightAnchor !== null ? "Highlight" : ""].filter(Boolean).join(" ");
  statusMode.hidden = statusMode.textContent === "";
  updateTranslatedLine();
}

// ---------------------------------------------------------------- status line

function caretPosition() {
  const before = editor.value.slice(0, editor.selectionStart);
  const lines = before.split("\n");
  return { line: lines.length - 1, column: lines[lines.length - 1].length };
}

function currentStyle() {
  // The innermost style open at the caret: the last [es~name] before it without its [ee~name].
  const before = editor.value.slice(0, editor.selectionStart);
  const open = [];
  for (const m of before.matchAll(/\[(es|ee)~([^\]]+)\]/g)) {
    if (m[1] === "es") open.push(m[2]);
    else { const i = open.lastIndexOf(m[2]); if (i >= 0) open.splice(i, 1); }
  }
  return open;
}

function updateStatus() {
  const d = current();
  if (!d) { statusStyle.textContent = "Current Style:"; statusPage.textContent = "Page:"; statusLineNumber.textContent = "Line:"; statusColumn.textContent = "Column:"; return; }
  const { line, column } = caretPosition();
  const lines = d.settings.lines;
  statusPage.textContent = `Page:${Math.floor(line / lines) + 1}`;
  statusLineNumber.textContent = `Line:${(line % lines) + 1}`;
  statusColumn.textContent = `Column:${column + 1}`;
  const styles = currentStyle();
  statusStyle.textContent = styles.length === 0 ? "Current Style: None"
    : styles.length === 1 ? `Current Style: ${styles[0]}` : `Current Style: ${styles[styles.length - 1]} (level ${styles.length})`;
  updateTranslatedLine();
}

// View, Translated Line: the line the caret is on, translated. A print document shows its braille (drawn in the
// font chosen in Global, Default Views); a braille document shows its print. Updated when the caret moves to
// another line or the line changes, and announced politely so screen reader users hear it.
let lastTranslatedLine = null;
let translatedTimer = null;

function currentLineText() {
  const v = editor.value;
  const start = v.lastIndexOf("\n", editor.selectionStart - 1) + 1;
  const end = v.indexOf("\n", editor.selectionStart);
  return v.slice(start, end < 0 ? v.length : end);
}

function updateTranslatedLine() {
  const d = current();
  if (!d || !d.view.translated) return;
  const line = currentLineText();
  const key = `${active}|${d.kind}|${d.table}|${settingsGlobal.encoding}|${line}`;
  if (key === lastTranslatedLine) return;
  lastTranslatedLine = key;
  clearTimeout(translatedTimer);
  translatedTimer = setTimeout(async () => {
    const font = settingsGlobal.defaultViews.translatedFont;
    translatedPane.classList.remove("braille", "simbraille");
    try {
      if (d.kind === "print") {
        const cells = await shellCall("translate", { table: d.table, text: stripCodes(line) });
        if (font !== "print") translatedPane.classList.add(font);
        translatedText.textContent = cellsToText(cells);
      } else {
        translatedText.textContent = await shellCall("back_translate", { table: d.table, braille: textToCells(line) });
      }
    } catch (e) {
      translatedText.textContent = String(e.message || e);
    }
  }, 120);
}

function announce(text) {
  announcer.textContent = "";
  setTimeout(() => { announcer.textContent = text; }, 30);
}

// ---------------------------------------------------------------- editing helpers

function insertText(text) {
  editor.focus();
  // execCommand keeps the edit on the undo stack; setRangeText is the fallback.
  if (!document.execCommand("insertText", false, text)) {
    editor.setRangeText(text, editor.selectionStart, editor.selectionEnd, "end");
  }
  markDirty();
}

function wrapSelection(before, after) {
  const start = editor.selectionStart;
  const end = editor.selectionEnd;
  const inner = editor.value.slice(start, end);
  insertText(before + inner + after);
  const caret = start + before.length + inner.length;
  editor.setSelectionRange(caret, caret);
}

function markDirty() {
  const d = current();
  if (d) d.dirty = true;
  updateStatus();
}

// Layout menu codes: what each item inserts. A pair wraps the selection.
const CODE_INSERTS = {
  "code.grade1": "[tx][g1]", "code.grade2": "[tx][g2]",
  "code.spanishText": "[g1][fl-span]", "code.lifgText": "[g1][fl-lifg]",
  "code.letterSign": "&+",
  "code.newLine": "[l]", "code.skipLine": "[sk1]", "code.suppressSkippedLine": "[skn]",
  "code.centering": ["[hds]", "[hde]"], "code.flushRight": "[fr]",
  "code.tabRunover": "[l][run]", "code.singleSpacing": "[svsbl0]", "code.doubleSpacing": "[svsbl1]",
  "code.group": ["[:]", "[;]"],
  "code.math.technicalContext": ["[tcs]", "[tce]"], "code.math.technicalNotation": ["[ts]", "[te]"],
  "code.math.base": ["[bs]", "[be]"], "code.math.power": ["[ps]", "[pe]"], "code.math.directExpression": "[e]",
  "code.math.subscriptUnder": ["[us]", "[ue]"], "code.math.superscriptOver": ["[os]", "[oe]"],
  "code.math.fraction": ["[fs]", "[fl][fe]"], "code.math.squareRoot": ["[sqrts]", "[sqrte]"],
  "code.math.indexedRoot": ["[ixrts]", "[ixrtd][ixrte]"],
  "code.math.grade1": "[i]", "code.math.grade1LetterSign": "[ii]",
  "code.math.boldPassage": ["[fts~b]", "[fte~b]"], "code.math.italicPassage": ["[fts~i]", "[fte~i]"],
  "code.math.numericEquivalent": "[eqvn]", "code.math.omitNumericIndicator": ["[ons]", "[one]"],
  "code.math.romanNumeral": "[rmn]", "code.math.cancellation": ["[ecans]", "[ecane]"],
  "code.math.horizontalBar": "[bar]", "code.math.extendedVerticalBar": "[evb]", "code.math.baseline": "[bline]",
  "code.math.enclosedList": "[enclis]", "code.math.newLineIndicated": "[nr]",
  "code.blockProtect": ["[kps]", "[kpe]"], "code.verticalCentering": ["[vcs]", "[vce]"],
};
for (const n of [1, 2, 3, 4, 5, 7, 9, 11, 13]) CODE_INSERTS[`code.leftMargin.${n}`] = `[ind${n}]`;
for (const n of [0, 1, 2, 4, 6]) CODE_INSERTS[`code.rightMargin.${n}`] = `[rm${n}]`;
CODE_INSERTS["code.runover.0"] = "[ptye]";
for (const n of [1, 2, 3, 4, 5, 6, 8, 10]) CODE_INSERTS[`code.runover.${n}`] = `[ptys${n}]`;

function insertCode(id) {
  const d = current();
  if (!d) return;
  const s = settingsGlobal.shortcuts;
  const dynamic = {
    "code.grade0": s.grade0, "code.cbc": s.cbc, "code.flushRightLeader": s.flushRightLeader,
    "code.newPage": s.newPage, "code.newReferencePage": s.newRefPage,
    "code.terminationSign": d.kind === "braille" ? ",'" : "&@",
    "code.hardSpace": " ",
  };
  const code = dynamic[id] !== undefined ? dynamic[id] : CODE_INSERTS[id];
  if (code === undefined) return;
  if (Array.isArray(code)) wrapSelection(code[0], code[1]);
  else insertText(code);
}

// The code vocabulary for Layout, Code List (code, description).
const CODE_LIST = `atx|ASCII is all text;be|Base (subscript) end;bline|Baseline/multipurpose indicator;bs|Base (subscript) start;caplv|Set capital letter indication level;cap-invert|Inverted caps sense;cap-normal|Normal caps sense;cb|Switch to CBC;cbi|Switch to CBC with indicator;cbn|Switch to CBC without indicator;cp|Conditional page;cs|Use countable spaces;cs-off|Don't use countable spaces;ctb|Clear tab stops;cz|Switch to computer 'grade 0' braille;d~|Direct braille followed by a space;e|Expression modified by directly over/under super/subscript;ecane|Cancellation end;ecans|Cancellation start;ee~|Style (element) end;enclis|Enclosed list prefix;eqvn|Equivalent to number prefix;es~|Style (element) start;evb|Extended vertical bar;fe|Fraction end;fl|Fraction line;fl-lifg|Foreign language is Latin, Italian, French, or German;fl-none|Disable foreign language treatment;fl-span|Foreign language is Spanish;fr|Flush right;fs|Fraction start;fte~|Verbal passage end;fts~|Verbal passage start;g1|Switch to grade 1;g2|Switch to grade 2;hde|Center text, end;hds|Center text, start;i|Next word is grade 1;ii|Next word is grade 1 (with indicator);ind|Set left margin (on new line);inm|Set left margin (without new line);kpe|Line block-protect, end;kps|Line block-protect, start;l|Break line if text on line;lea|Mark print page break;lec|Set reference page number;led|Discontinue reference page number;<|Force new line;lnb~|Language switch - basic;lng~|Language switch;nr|New line indicated only, e.g. in binomial;oe|Directly over superscript end;one|Omit numeric-indicator end;ons|Omit numeric-indicator start;os|Directly over superscript start;p|New paragraph;pe|Power (superscript) end;pg|Force new page;pnta|Start Arabic page numbering;pntr|Start Roman page numbering;ps|Power (superscript) start;ptye|Poetry mode (indented runover) end;ptys|Poetry mode (indented runover) start;pv|Set page numbering without page change;q~|Direct braille;rfd|Discontinue running footer;rfe|End definition of running footer;rfs|Start definition of running footer;rm|Set right margin;rmn|Roman numeral indicator;rpe|End text for table of contents;rps|Start text for table of contents;run|Tab to runover point;sk|Skip line;skn|Skipped line nullify;sqrte|Square root end;sqrts|Square root start;st|Skip to specified line;stb|Set tab stop;svsbl|Set interline spacing;tab|Tab to position on same or next line;tce|Technical context end;tcs|Technical context start;te|Technical notation end;tld|Discontinue running header;tle|End definition of running header;tls|Start definition of running header;tne|Transcriber's note end;tns|Transcriber's note start;top|Assure top of page; conditional page eject;ts|Technical notation start;tx|Resume normal text translation;ue|Directly under subscript end;us|Directly under subscript start;vce|Vertical centering end;vcs|Vertical centering start;wb|Resume normal word breaking;wb-no|Suspend all word breaking`
  .split(";").map((e) => e.split("|"));

// ---------------------------------------------------------------- find and replace

function findNext(spec, from, backward) {
  const text = editor.value;
  let needle = spec.find;
  let hay = text;
  if (!spec.matchCase) { needle = needle.toLowerCase(); hay = hay.toLowerCase(); }
  const isWord = (i) => i >= 0 && i < hay.length && /\w/.test(hay[i]);
  let i = backward ? hay.lastIndexOf(needle, from - 1) : hay.indexOf(needle, from);
  while (i >= 0 && spec.wholeWords && (isWord(i - 1) || isWord(i + needle.length))) {
    i = backward ? hay.lastIndexOf(needle, i - 1) : hay.indexOf(needle, i + 1);
  }
  return i;
}

function selectRange(start, end) {
  editor.focus();
  editor.setSelectionRange(start, end);
  // Scroll the found text into view by moving the caret through the textarea.
  const lineHeight = parseFloat(getComputedStyle(editor).lineHeight) || 20;
  const line = editor.value.slice(0, start).split("\n").length - 1;
  editor.scrollTop = Math.max(0, line * lineHeight - editor.clientHeight / 2);
  updateStatus();
}

async function doFind(again) {
  if (!current()) return;
  if (!again || !lastFind) {
    const r = await showDialog("find", (p) => {
      if (lastFind) { p.find.value = lastFind.find; p.matchCase.checked = lastFind.matchCase; p.backward.checked = lastFind.backward; p.wholeWords.checked = lastFind.wholeWords; }
      else if (editor.selectionEnd > editor.selectionStart) p.find.value = editor.value.slice(editor.selectionStart, editor.selectionEnd);
    });
    if (r.button !== "ok") return;
    if (!r.values.find) { await messageBox("The search string is empty."); return; }
    lastFind = r.values;
  }
  const backward = lastFind.backward;
  const from = backward ? editor.selectionStart : editor.selectionEnd;
  let i = findNext(lastFind, from, backward);
  const atEdge = backward ? from === editor.value.length : from === 0;
  if (i < 0 && !atEdge) {
    // Offer to continue from the other end of the document.
    const question = backward
      ? "BrailleMaker has finished searching to the beginning of the document.  Would you like to continue searching at the end?"
      : "BrailleMaker has finished searching to the end of the document.  Would you like to continue searching at the beginning?";
    const r = await showDialog("yesNo", (p) => { p.text.textContent = question; });
    if (r.button !== "yes") return;
    i = findNext(lastFind, backward ? editor.value.length : 0, backward);
  }
  if (i < 0) { await messageBox(again ? "The text does not appear again in the document." : "Text was not found in the document."); return; }
  selectRange(i, i + lastFind.find.length);
}

async function doReplace() {
  if (!current()) return;
  const r = await showDialog("replace", (p) => {
    if (lastFind) { p.find.value = lastFind.find; p.matchCase.checked = lastFind.matchCase; p.wholeWords.checked = lastFind.wholeWords; }
  });
  if (r.button !== "ok") return;
  if (!r.values.find) { await messageBox("The search string is empty."); return; }
  lastFind = r.values;
  let count = 0;
  let all = false;
  let pos = r.values.backward ? editor.selectionStart : editor.selectionEnd;
  for (;;) {
    const i = findNext(r.values, pos, r.values.backward);
    if (i < 0) break;
    selectRange(i, i + r.values.find.length);
    let answer = "yes";
    if (!all) answer = (await showDialog("confirmReplace")).button;
    if (answer === "cancel") break;
    if (answer === "all") all = true;
    if (answer === "yes" || answer === "all") {
      editor.setSelectionRange(i, i + r.values.find.length);
      insertText(r.values.replaceWith);
      count++;
      pos = r.values.backward ? i : i + r.values.replaceWith.length;
    } else {
      pos = r.values.backward ? i : i + r.values.find.length;
    }
  }
  await messageBox(`${count} replacements were made.`);
}

// ---------------------------------------------------------------- six-key entry

const sixKeyDown = new Set();
let sixKeyChord = 0;

function sixKeyIndex(key) { return settingsGlobal.sixKeySet.indexOf(key.toLowerCase()); }

editor.addEventListener("keydown", (event) => {
  const d = current();
  if (!d || !d.view.sixKey || d.kind !== "braille" || event.ctrlKey || event.altKey || event.metaKey) return;
  const dot = sixKeyIndex(event.key);
  if (dot < 0) return;
  event.preventDefault();
  sixKeyDown.add(dot);
  sixKeyChord |= 1 << dot;
});

editor.addEventListener("keyup", (event) => {
  const d = current();
  if (!d || !d.view.sixKey || d.kind !== "braille") return;
  const dot = sixKeyIndex(event.key);
  if (dot < 0) return;
  event.preventDefault();
  sixKeyDown.delete(dot);
  if (sixKeyDown.size === 0 && sixKeyChord) {
    // The cell is entered when the last key of the chord is released.
    insertText(cellChar(sixKeyChord));
    sixKeyChord = 0;
  }
});

// ---------------------------------------------------------------- files (read only until the format exists)

function pickFile(input) {
  return new Promise((resolve) => {
    input.value = "";
    input.onchange = () => resolve(input.files[0] || null);
    input.click();
  });
}

async function openFile() {
  const file = await pickFile(document.getElementById("open-file"));
  if (!file) return;
  const text = (await file.text()).replace(/\r\n?/g, "\n");
  const braille = /\.(brf|brl|bru)$/i.test(file.name);
  newDocument(braille ? "braille" : "print", file.name, braille ? text.replace(/\f/g, "\n") : text);
  current().dirty = false;
  announce(`Opened ${file.name}`);
}

async function insertFile() {
  if (!current()) return;
  const file = await pickFile(document.getElementById("insert-file"));
  if (!file) return;
  insertText((await file.text()).replace(/\r\n?/g, "\n"));
}

// Asks about an unsaved document. Returns false if the user cancels.
async function confirmDiscard(d) {
  if (!d.dirty) return true;
  const r = await showDialog("saveChanges", (p) => { p.text.textContent = `Document "${d.name}" not saved!  Save changes?`; });
  if (r.button === "cancel") return false;
  if (r.button === "yes") { await messageBox(PLANNED.save); return false; }
  return true;
}

async function closeDocument() {
  const d = current();
  if (!d) return;
  keepEditorState();
  if (!(await confirmDiscard(d))) return;
  documents.splice(active, 1);
  active = -1;
  activate(Math.min(documents.length - 1, Math.max(0, documents.length - 1)));
}

async function exitApp() {
  keepEditorState();
  for (let i = 0; i < documents.length; i++) {
    if (documents[i].dirty) { activate(i); if (!(await confirmDiscard(documents[i]))) return; }
  }
  if (tauri) invoke("exit_app");
  else announce("BrailleMaker would exit now.");
}

// ---------------------------------------------------------------- dialogs wired to settings

function remember(name, values) { settingsGlobal.dialogs[name] = values; }

function restore(name, parts) {
  const saved = settingsGlobal.dialogs[name];
  if (!saved) return;
  for (const [id, v] of Object.entries(saved)) {
    const p = parts[id];
    if (!p) continue;
    if (p.radios) { const r = p.radios.querySelector(`input[value="${CSS.escape(String(v))}"]`); if (r) r.checked = true; }
    else if (p.type === "checkbox") p.checked = v;
    else if ("value" in p) p.value = v;
  }
}

async function settingsDialog(name, after) {
  const r = await showDialog(name, (p) => restore(name, p));
  if (r.button === "ok") { remember(name, r.values); if (after) await after(r.values); }
  return r;
}

async function fileNew() {
  const r = await showDialog("fileNew", (p) => {
    for (const t of ["English (UEB) - Standard", "English (UEB) - Textbook Format"]) {
      const o = document.createElement("option"); o.textContent = t; p.template.append(o);
    }
    p.template.selectedIndex = 0;
    for (const b of ["markDefault", "hide", "showAlways", "region"]) {
      p[b].addEventListener("click", () => messageBox(PLANNED.templates));
    }
  });
  if (r.button === "ok") newDocument(r.values.kind);
}

async function embosserSetup(globalSetup) {
  const d = current();
  await showDialog("embosserSetup", (p, dialog) => {
    if (globalSetup) dialog.querySelector(".caption").textContent = "Global: Embosser Setup";
    restore("embosserSetup", p);
    if (d && !globalSetup) { p.cells.value = d.kind === "braille" ? d.settings.cells : 40; p.lines.value = d.kind === "braille" ? d.settings.lines : 25; }
    for (const b of ["new", "modify", "remove"]) p[b].addEventListener("click", () => messageBox(PLANNED.emboss));
  }).then(async (r) => {
    if (r.button !== "ok") return;
    remember("embosserSetup", r.values);
    if (d && !globalSetup && d.kind === "braille") { d.settings.cells = r.values.cells; d.settings.lines = r.values.lines; applyView(); updateStatus(); }
  });
}

async function printerSetup(globalSetup) {
  const d = current();
  await showDialog("printerSetup", (p, dialog) => {
    if (globalSetup) dialog.querySelector(".caption").textContent = "Global: Printer Setup...";
    restore("printerSetup", p);
    if (d && !globalSetup && d.kind === "print") { p.lines.value = d.settings.lines; p.chars.value = d.settings.cells; }
    p.printer.addEventListener("click", () => messageBox(PLANNED.print));
  }).then((r) => {
    if (r.button !== "ok") return;
    remember("printerSetup", r.values);
    if (d && !globalSetup && d.kind === "print") { d.settings.lines = r.values.lines; d.settings.cells = r.values.chars; applyView(); updateStatus(); }
  });
}

function fillStyles(select, d) {
  select.replaceChildren();
  for (const s of d.styles) { const o = document.createElement("option"); o.textContent = s.name; select.append(o); }
  if (d.styles.length) select.selectedIndex = 0;
}

async function addStyle() {
  const d = current();
  if (!d) return;
  const r = await showDialog("addStyle", (p) => {
    const none = document.createElement("option"); none.textContent = "(No Style)"; p.basis.append(none);
    for (const s of d.styles) { const o = document.createElement("option"); o.textContent = s.name; p.basis.append(o); }
    p.basis.selectedIndex = 0;
  });
  if (r.button !== "ok") return;
  const name = r.values.name.trim();
  if (!name) { await messageBox("The style name is empty."); return; }
  if (name === "New Style Name") { await messageBox("You must give the style a name other than \"New Style Name\"."); return; }
  if (!/^[A-Za-z0-9._-]+$/.test(name)) { await messageBox("The style name contains invalid characters.  Style names may only contain letters (uppercase and lowercase), numbers, the period, the hyphen, and the underscore."); return; }
  if (d.styles.some((s) => s.name === name)) { await messageBox(`A style named ${name} already exists.`); return; }
  const basis = d.styles.find((s) => s.name === r.values.basis);
  d.styles.push({ name, nested: r.values.nested, begin: r.values.begin || (basis ? basis.begin : ""), end: r.values.end || (basis ? basis.end : "") });
  announce(`Style ${name} added.`);
}

async function chooseStyle(dialogName) {
  const d = current();
  if (!d) return null;
  if (!d.styles.length) { await messageBox("This document has no styles yet. Use Document, Add Style to create one."); return null; }
  const r = await showDialog(dialogName, (p) => fillStyles(p.style, d));
  return r.button === "ok" ? d.styles.find((s) => s.name === r.values.style) : null;
}

function applyStyleNamed(style) {
  wrapSelection(`[es~${style.name}]`, `[ee~${style.name}]`);
  current().lastStyle = style;
}

async function modifyStyle() {
  const d = current();
  if (!d) return;
  if (!d.styles.length) { await messageBox("This document has no styles yet. Use Document, Add Style to create one."); return; }
  const r = await showDialog("modifyStyle", (p) => {
    fillStyles(p.style, d);
    const load = () => { const s = d.styles.find((x) => x.name === p.style.value); if (s) { p.begin.value = s.begin; p.end.value = s.end; } };
    p.style.addEventListener("change", load);
    load();
    p.rename.addEventListener("click", async () => {
      const s = d.styles.find((x) => x.name === p.style.value);
      if (!s) return;
      const rr = await showDialog("renameStyle", (q) => { q.name.value = s.name; });
      if (rr.button !== "ok" || !rr.values.name.trim()) return;
      const old = s.name;
      s.name = rr.values.name.trim();
      if (rr.values.replaceInstances) {
        editor.value = editor.value.split(`[es~${old}]`).join(`[es~${s.name}]`).split(`[ee~${old}]`).join(`[ee~${s.name}]`);
        markDirty();
      }
      fillStyles(p.style, d);
      p.style.value = s.name;
    });
  });
  if (r.button !== "ok") return;
  const s = d.styles.find((x) => x.name === r.values.style);
  if (s) { s.begin = r.values.begin; s.end = r.values.end; announce(`Style ${s.name} changed.`); }
}

async function deleteStyle() {
  const d = current();
  const s = await chooseStyle("deleteStyle");
  if (!s) return;
  d.styles = d.styles.filter((x) => x !== s);
  announce(`Style ${s.name} deleted.`);
}

async function codeList() {
  if (!current()) return;
  const r = await showDialog("codeList", (p) => {
    for (const [code, desc] of CODE_LIST) { const o = document.createElement("option"); o.value = code; o.textContent = `${code} (${desc})`; p.code.append(o); }
    p.code.selectedIndex = 0;
    const show = () => { p.chosen.textContent = `Chosen Code: [${p.code.value}]`; };
    p.code.addEventListener("change", show);
    show();
  });
  if (r.button === "help") { await messageBox(PLANNED.help); return; }
  if (r.button !== "ok") return;
  const params = (r.values.parameters || "").trim();
  if (/\s/.test(params)) { await messageBox("The parameters to a code must not contain spaces."); return; }
  insertText(`[${r.values.code}${params}]`);
}

async function gotoPosition() {
  const d = current();
  if (!d) return;
  const { line, column } = caretPosition();
  const r = await showDialog("goto", (p) => {
    p.page.value = Math.floor(line / d.settings.lines) + 1;
    p.line.value = (line % d.settings.lines) + 1;
    p.column.value = column + 1;
  });
  if (r.button !== "ok") return;
  const all = editor.value.split("\n");
  const target = (r.values.page - 1) * d.settings.lines + (r.values.line - 1);
  if (r.values.page < 1 || r.values.line < 1 || r.values.column < 1) { await messageBox("You cannot go to pages with numbers less than 1."); return; }
  if (target >= all.length) { await messageBox("You cannot go to that location in the document."); return; }
  let offset = 0;
  for (let i = 0; i < target; i++) offset += all[i].length + 1;
  offset += Math.min(r.values.column - 1, all[target].length);
  selectRange(offset, offset);
}

function changeCase() {
  const start = editor.selectionStart;
  const end = editor.selectionEnd;
  if (start === end) return;
  const text = editor.value.slice(start, end);
  insertText(text === text.toUpperCase() ? text.toLowerCase() : text.toUpperCase());
  editor.setSelectionRange(start, end);
}

function toggleHighlight() {
  // Highlight (Ctrl+H) sets an anchor at the caret; moving the caret then selects from the anchor; again ends it.
  highlightAnchor = highlightAnchor === null ? editor.selectionStart : null;
  announce(highlightAnchor === null ? "Highlight off" : "Highlight on");
  applyView();
}

document.addEventListener("selectionchange", () => {
  if (document.activeElement !== editor) return;
  if (highlightAnchor !== null && editor.selectionStart === editor.selectionEnd && editor.selectionStart !== highlightAnchor) {
    const caret = editor.selectionStart;
    editor.setSelectionRange(Math.min(caret, highlightAnchor), Math.max(caret, highlightAnchor), caret < highlightAnchor ? "backward" : "forward");
  }
  updateStatus();
});

function editCommand(name) {
  editor.focus();
  if (!document.execCommand(name)) {
    if (name === "paste") messageBox("Use Control+V to paste.");
  }
  if (name !== "copy" && name !== "selectAll") markDirty();
}

function joinLines() {
  const value = editor.value;
  const nl = value.indexOf("\n", editor.selectionStart);
  if (nl < 0) return;
  editor.setSelectionRange(nl, nl + 1);
  insertText(" ");
}

function setView(key, value) {
  const d = current();
  if (!d) return;
  if (key === "sixKey" && d.kind !== "braille" && value) {
    messageBox("Six-key entry is for braille documents.");
    invoke("set_check", { id: "view.sixKey", checked: false });
    return;
  }
  d.view[key] = value;
  applyView();
  announce(`${{ fullPage: "Full-Page", sixKey: "Six-Key Entry", codes: "Codes", translated: "Translated Line" }[key]} ${value ? "on" : "off"}`);
}

function setFont(font) {
  const d = current();
  if (!d) return;
  // Any of the three fonts in any document (in a print document the braille fonts show its characters
  // as the cells they stand for in the chosen encoding).
  d.view.font = font;
  applyView();
}

// ---------------------------------------------------------------- commands

const COMMANDS = {
  "file.new": fileNew,
  "file.open": openFile,
  "file.close": closeDocument,
  "file.save": () => messageBox(PLANNED.save),
  "file.saveAs": () => messageBox(PLANNED.save),
  "file.saveEbraille": () => messageBox(PLANNED.save),
  "file.translate": translateDocument,
  "file.print": async () => { if ((await showDialog("print")).button === "ok") await messageBox(PLANNED.print); },
  "file.emboss": async () => { if ((await showDialog("emboss")).button === "ok") await messageBox(PLANNED.emboss); },
  "file.exit": exitApp,

  "edit.undo": () => editCommand("undo"),
  "edit.redo": () => editCommand("redo"),
  "edit.cut": () => editCommand("cut"),
  "edit.copy": () => editCommand("copy"),
  "edit.paste": () => editCommand("paste"),
  "edit.clear": () => editCommand("delete"),
  "edit.selectAll": () => editCommand("selectAll"),
  "edit.highlight": toggleHighlight,
  "edit.changeCase": changeCase,
  "edit.insertFile": insertFile,
  "edit.goto": gotoPosition,
  "edit.find": () => doFind(false),
  "edit.findAgain": () => doFind(true),
  "edit.replace": doReplace,
  "edit.spellCheck": () => messageBox(PLANNED.spell),
  "edit.findMisspelling": () => messageBox(PLANNED.spell),

  "view.fullPage": (checked) => setView("fullPage", checked),
  "view.printFont": () => setFont("print"),
  "view.brailleFont": () => setFont("braille"),
  "view.simBrailleFont": () => setFont("simbraille"),
  "view.sixKey": (checked) => setView("sixKey", checked),
  "view.codes": (checked) => setView("codes", checked),
  "view.translatedLine": (checked) => setView("translated", checked),

  "layout.applyStyle": async () => { const s = await chooseStyle("applyStyle"); if (s) applyStyleNamed(s); },
  "layout.applyLastStyle": () => { const d = current(); if (d && d.lastStyle) applyStyleNamed(d.lastStyle); else messageBox("No style has been applied yet in this document."); },
  "layout.styleLevelUp": () => messageBox("Style levels need the page formatter, built in step 8 of the plan."),
  "layout.styleLevelDown": () => messageBox("Style levels need the page formatter, built in step 8 of the plan."),
  "layout.joinStyles": () => messageBox("Join Styles needs the page formatter, built in step 8 of the plan."),
  "layout.joinLines": joinLines,
  "layout.setPageNumber": async () => {
    const r = await showDialog("setPageNumber");
    if (r.button === "ok") insertText(`[${r.values.style === "roman" ? "pntr" : "pnta"}][pv${r.values.page}]`);
  },
  "layout.header": async () => {
    const r = await showDialog("header");
    if (r.button === "ok") insertText(r.values.placement === "none" ? "[tld]" : `[tls]${r.values.text}[tle]`);
  },
  "layout.footer": async () => {
    const r = await showDialog("footer");
    if (r.button === "ok") insertText(r.values.placement === "none" ? "[rfd]" : `[rfs]${r.values.text}[rfe]`);
  },
  "layout.generateToc": () => messageBox(PLANNED.toc),
  "layout.codeList": codeList,

  "table.create": async () => { if ((await settingsDialog("tableProperties")).button === "ok") wrapSelection("[htbs]", "[htbe]"); },
  "table.properties": () => settingsDialog("tableProperties"),

  "document.embosserSetup": () => embosserSetup(false),
  "document.printerSetup": () => printerSetup(false),
  "document.pageNumbering": () => settingsDialog("pageNumbering"),
  "document.ebrailleMetadata": () => messageBox(PLANNED.save),
  "document.learning.uncontracted": () => setTable("en-ueb-g1.ctb", "Uncontracted (UEB grade 1)"),
  "document.learning.contracted": () => setTable("en-ueb-g2.ctb", "Fully Contracted (UEB grade 2)"),
  "document.addStyle": addStyle,
  "document.modifyStyle": modifyStyle,
  "document.deleteStyle": deleteStyle,
  "document.createTemplate": async () => { if ((await showDialog("createTemplate")).button === "ok") await messageBox(PLANNED.templates); },
  "document.deleteTemplate": () => messageBox(PLANNED.templates),

  "macro.none": () => messageBox("No macros are defined."),

  "global.embosserSetup": () => embosserSetup(true),
  "global.printerSetup": () => printerSetup(true),
  "global.fileLocations": () => settingsDialog("fileLocations"),
  "global.defaultViews": () => settingsDialog("defaultViews", (v) => { Object.assign(settingsGlobal.defaultViews, v); }),
  "global.internationalization": internationalization,
  "global.fonts": () => settingsDialog("fonts"),
  "global.viewPreferences": () => settingsDialog("viewPreferences", async (v) => {
    if (v.sixKeySet.length !== 6) { await messageBox("You must type exactly six keys in as the \"Six-Key Key Set.\"  The keys you type represent, in order, what you will type for Dots 1, 2, 3, 4, 5, and 6 when entering braille by six-key chording."); return; }
    settingsGlobal.sixKeySet = v.sixKeySet.toLowerCase();
  }),
  "global.shortcutPreferences": () => settingsDialog("shortcutPreferences", (v) => {
    Object.assign(settingsGlobal.shortcuts, { grade0: v.grade0, cbc: v.cbc, flushRightLeader: v.flushRightLeader, newPage: v.newPage, newRefPage: v.newRefPage });
  }),
  "global.notificationSettings": () => showDialog("notificationSettings"),
  "global.importOptions": () => settingsDialog("importOptions"),
  "global.wordPerfectImporter": () => settingsDialog("wordPerfectImporter"),
  "global.wordImporter": () => settingsDialog("wordImporter"),
  "global.pdfImporter": () => messageBox("The PDF importer and its options are built in step 9 of the plan."),
  "global.formattedBrailleImporter": () => settingsDialog("formattedBrailleImporter"),

  "help.topics": () => messageBox(PLANNED.help),
  "help.translate": () => messageBox(PLANNED.help),
  "help.favorites": () => messageBox(PLANNED.help),
  "help.whatsNew": () => messageBox("BrailleMaker 0.1: the main window, menus, dialogs, editing, find and replace, six-key entry and the code vocabulary. Translation, the file format, printing and embossing follow in the plan."),
  "help.checkUpdates": () => messageBox("Updates are published on the BrailleMaker GitHub page."),
  "help.about": () => showDialog("about"),
};
for (const id of Object.keys(CODE_INSERTS)) COMMANDS[id] = () => insertCode(id);
for (const id of ["code.grade0", "code.cbc", "code.flushRightLeader", "code.newPage", "code.newReferencePage", "code.terminationSign", "code.hardSpace"]) {
  COMMANDS[id] = () => insertCode(id);
}
for (const h of ["none", "dashes", "dashesHyphens", "french", "french113"]) {
  COMMANDS[`document.hyphen.${h}`] = () => { const d = current(); if (d) d.hyphen = h; };
}
for (const t of ["western", "europe", "asia", "africa", "pacific", "all"]) COMMANDS[`document.tables.${t}`] = () => messageBox(PLANNED.tables);
for (const p of ["add", "alignLeft", "alignCenter", "alignRight", "dots25", "dots20", "dots16", "dots13", "dots10"]) COMMANDS[`layout.picture.${p}`] = () => messageBox(PLANNED.pictures);

let busy = false;
async function runCommand(id, checked) {
  // While a dialog is open, menu commands wait (dialogs are modal).
  if (busy || document.querySelector("dialog[open]")) return;
  if (id.startsWith("view.document.")) { activate(Number(id.slice(14))); return; }
  const command = COMMANDS[id];
  if (!command) return;
  busy = true;
  try { await command(checked); } finally { busy = false; }
  if (current()) { keepEditorState(); updateStatus(); }
}

// ---------------------------------------------------------------- wiring

editor.addEventListener("input", markDirty);
editor.addEventListener("click", updateStatus);
editor.addEventListener("keyup", updateStatus);
window.addEventListener("resize", applyView);

if (tauri && tauri.event) {
  tauri.event.listen("menu", (event) => {
    const p = event.payload || {};
    runCommand(p.id, p.checked);
  });
} else {
  // Browser use: the shortcuts bound to the same commands.
  const KEYS = {
    "Ctrl+n": "file.new", "Ctrl+o": "file.open", "Ctrl+F4": "file.close", "Ctrl+s": "file.save", "F3": "file.saveAs",
    "Ctrl+t": "file.translate", "Ctrl+p": "file.print", "Ctrl+e": "file.emboss", "Ctrl+h": "edit.highlight",
    "Shift+F3": "edit.changeCase", "Ctrl+g": "edit.goto", "Ctrl+f": "edit.find", "Ctrl+i": "edit.findAgain", "F6": "edit.replace",
    "Shift+F11": "edit.spellCheck", "F11": "edit.findMisspelling", "Ctrl+F5": "view.printFont", "Ctrl+F6": "view.brailleFont",
    "Ctrl+F7": "view.simBrailleFont", "F2": "view.sixKey", "Alt+F3": "view.codes", "Ctrl+F8": "view.translatedLine",
    "F8": "layout.applyStyle", "Shift+F8": "layout.applyLastStyle", "Alt+ArrowRight": "layout.styleLevelUp", "Alt+ArrowLeft": "layout.styleLevelDown",
    "Alt+0": "code.grade0", "Alt+1": "code.grade1", "Alt+2": "code.grade2", "Alt+3": "code.cbc", "Alt+4": "code.spanishText", "Alt+5": "code.lifgText",
    "Alt+F2": "code.hardSpace", "Ctrl+b": "code.group", "Ctrl+l": "code.newLine", "Ctrl+k": "code.skipLine", "Alt+k": "code.suppressSkippedLine",
    "Shift+F6": "code.centering", "Shift+F7": "code.flushRight", "Alt+F7": "code.flushRightLeader", "Shift+F2": "code.doubleSpacing",
    "Ctrl+m": "code.newParagraph", "F4": "code.leftMargin.3", "Shift+F4": "code.rightMargin.6", "Ctrl+r": "code.runover.2",
    "Ctrl+Enter": "code.newPage", "Alt+Enter": "code.newReferencePage", "F5": "layout.codeList",
    "Ctrl+F11": "document.embosserSetup", "Ctrl+F12": "document.printerSetup",
  };
  const TOGGLES = { "view.fullPage": "fullPage", "view.sixKey": "sixKey", "view.codes": "codes", "view.translatedLine": "translated" };
  document.addEventListener("keydown", (event) => {
    if (document.querySelector("dialog[open]")) return;
    const mods = `${event.ctrlKey || event.metaKey ? "Ctrl+" : ""}${event.altKey ? "Alt+" : ""}${event.shiftKey ? "Shift+" : ""}`;
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
    const id = KEYS[mods + key];
    if (!id) return;
    event.preventDefault();
    const d = current();
    const toggle = TOGGLES[id];
    runCommand(id, toggle && d ? !d.view[toggle] : undefined);
  });
}

// "code.newParagraph" (Ctrl+M) inserts the new paragraph code.
CODE_INSERTS["code.newParagraph"] = "[p]";
COMMANDS["code.newParagraph"] = () => insertCode("code.newParagraph");

// ---------------------------------------------------------------- braille fonts

// View, Braille Font and SimBraille Font draw the document with the font pair of the braille encoding chosen in
// Global, Internationalization (ui/fonts, built by tools/fonts/make_braille_fonts.py). The text is not changed.
const ENCODINGS = window.BRAILLE_ENCODINGS || [{ id: "north-american", name: "North American English" }];
const loadedFonts = [];

async function loadBrailleFonts(id) {
  for (const f of loadedFonts.splice(0)) document.fonts.delete(f);
  for (const kind of ["Braille", "SimBraille"]) {
    const face = new FontFace(`BrailleMaker ${kind}`, `url("fonts/BrailleMaker-${kind}-${id}.ttf")`);
    try {
      await face.load();
      document.fonts.add(face);
      loadedFonts.push(face);
    } catch (e) {
      announce(`The ${kind} font for this encoding could not be loaded.`);
    }
  }
  settingsGlobal.encoding = id;
}

async function internationalization() {
  const r = await showDialog("internationalization", (p) => {
    restore("internationalization", p);
    p.encoding.replaceChildren();
    for (const e of ENCODINGS) {
      const o = document.createElement("option");
      o.value = e.id;
      o.textContent = e.name;
      p.encoding.append(o);
    }
    p.encoding.value = settingsGlobal.encoding;
  });
  if (r.button !== "ok") return;
  remember("internationalization", r.values);
  if (r.values.encoding && r.values.encoding !== settingsGlobal.encoding) {
    // Braille documents keep their cells: their text is rewritten in the new encoding's characters.
    const before = settingsGlobal.encoding;
    keepEditorState();
    for (const d of documents) {
      if (d.kind === "braille") d.text = cellsToTextIn(textToCells(d.text, before), r.values.encoding);
    }
    await loadBrailleFonts(r.values.encoding);
    activate(active);
    const e = ENCODINGS.find((x) => x.id === r.values.encoding);
    announce(`Braille encoding: ${e ? e.name : r.values.encoding}`);
  }
}

function cellsToTextIn(cells, encodingId) {
  const saved = settingsGlobal.encoding;
  settingsGlobal.encoding = encodingId;
  try { return cellsToText(cells); } finally { settingsGlobal.encoding = saved; }
}

// ---------------------------------------------------------------- translation

function setTable(table, label) {
  const d = current();
  if (!d) return;
  d.table = table;
  lastTranslatedLine = null;
  updateTranslatedLine();
  announce(`Translation table: ${label}`);
}

// Words longer than the line are broken; otherwise a line breaks at the last blank cell that fits.
function wrapBraille(line, width) {
  const out = [];
  let rest = line;
  while (rest.length > width) {
    let cut = rest.lastIndexOf(" ", width);
    if (cut <= 0) cut = width;
    out.push(rest.slice(0, cut).replace(/\s+$/, ""));
    rest = rest.slice(cut).replace(/^\s+/, "");
  }
  out.push(rest);
  return out;
}

// File, Translate (1109): a NEW document made from the open one, which is left untouched. A print document
// becomes a braille document (forward translation, lines wrapped to the cells per line); a braille document
// becomes a print document (back-translation). Page layout by codes and styles comes with the formatter.
async function translateDocument() {
  const source = current();
  if (!source) return;
  keepEditorState();
  if (source.text.trim() === "") { await messageBox("The file that you are trying to translate is empty."); return; }
  const toBraille = source.kind === "print";
  announce("Translating");
  let result;
  try {
    const lines = source.text.split("\n");
    const out = [];
    for (const line of lines) {
      if (toBraille) {
        const cells = await shellCall("translate", { table: source.table, text: stripCodes(line) });
        out.push(...wrapBraille(cellsToText(cells).replace(/\s+$/, ""), 40));
      } else {
        out.push(await shellCall("back_translate", { table: source.table, braille: textToCells(line) }));
      }
    }
    result = out.join("\n");
  } catch (e) {
    await messageBox(`Translation failed: ${e.message || e}`, "Error");
    return;
  }
  const kind = toBraille ? "braille" : "print";
  let name = null;
  if (!source.untitled) name = source.name.replace(/\.(txt|brf|brl|bru|prf)$/i, "") + (toBraille ? ".brf" : ".txt");
  const d = newDocument(kind, name, result);
  d.table = source.table;
  if (toBraille) d.settings = { ...d.settings, cells: 40 };
  d.dirty = true;
  activate(documents.indexOf(d));
  announce(`Translated into ${fullName(d)}`);
}

loadBrailleFonts(settingsGlobal.encoding);
newDocument("print");
current().dirty = false;
