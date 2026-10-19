// BrailleCraft web build: the desktop program's menus and dialogs (ui-web/tables.js, generated from the desktop's own
// tables), a document editor with the translated line and status line, and the core (bc-web, WebAssembly) for
// documents, translation, embosser setups and embossing.

import { MENUS, DIALOGS } from "./tables.js";

const $ = (id) => document.getElementById(id);
const editor = $("editor");
const translated = $("translated");
const statusLine = $("status");

// ------------------------------------------------------------------ the core

let core = null;
const engines = {};

// The core's browser glue (pkg/bc_web.js) is made by wasm-bindgen when the site is built; without it the page
// still opens, edits, saves text and shows every menu and dialog.
async function loadCore() {
  try {
    const m = await import("./pkg/bc_web.js");
    await m.default();
    core = m;
  } catch {
    core = null;
  }
}

async function engine(name) {
  if (!core) {
    return null;
  }
  if (!engines[name]) {
    const response = await fetch(`tables/${name}.bmt`);
    if (!response.ok) {
      return null;
    }
    engines[name] = new core.Engine(await response.text());
  }
  return engines[name];
}

// The shipped table each rules table code is recreated as (tables/index.tsv: rules, scan, table, language), the
// first line for a code chosen, as the desktop program's table set finds it.
let tableIndex = null;
// The number of tables the index lists (the About box's count, as the desktop program's table set counts them).
let tableCount = 0;
async function tableFor(code) {
  if (!tableIndex) {
    tableIndex = new Map();
    try {
      const response = await fetch("tables/index.tsv");
      for (const line of response.ok ? (await response.text()).split("\n") : []) {
        const [rules, , table] = line.split("\t");
        if (!rules || rules.startsWith("#") || !table) continue;
        tableCount += 1;
        if (!tableIndex.has(rules)) tableIndex.set(rules, table.trim());
      }
    } catch {
      // No index: no table is found.
    }
  }
  return tableIndex.get(code) ?? null;
}

// The engine for the document's own rules table: print-to-braille for a print document, braille-to-print for a
// braille one (or `braille` given). Answers the engine (null when the table is not in this copy), the document's
// file and the table code.
async function documentEngine(braille = doc.kind === "braille") {
  const file = await documentFile();
  const code = file.tables[braille ? 2 : 0];
  const name = await tableFor(code);
  return { e: name ? await engine(name) : null, file, code };
}

// Translates `text` (editor form, braille in North American braille) with engine `e`, the document's file `file`
// and its initial codes, with the language switches of the table list, as the desktop program does. A switch asks
// for a table the page has not fetched yet: it is fetched and the text translated again, until nothing new is
// asked for; a table not in this copy stays on the current table, as an unknown code does.
async function translateText(e, text, file, braille, running = false) {
  const initial = file.initial_codes;
  const list = await languages();
  if (!list) return running ? e.translate_document_running(text, file, initial) : e.translate_document(text, file, initial);
  for (let pass = 0; ; pass++) {
    const out = e.translate_document_with(text, file, initial, list, braille, running);
    const missing = list.missing();
    if (!missing.length || pass >= 8) return out;
    for (const code of missing) {
      const name = await tableFor(code);
      let source = null;
      try {
        const response = name ? await fetch(`tables/${name}.bmt`) : null;
        source = response?.ok ? await response.text() : null;
      } catch {
        source = null;
      }
      try {
        if (source) list.add_table(code, source);
        else list.add_absent(code);
      } catch {
        list.add_absent(code);
      }
    }
  }
}

// The translation table sets by language (tables/languages.tsv), for Document, Translation and Learning Tables.
let languageList = null;
async function languages() {
  if (!languageList && core) {
    try {
      const response = await fetch("tables/languages.tsv");
      if (response.ok) languageList = new core.LanguageList(await response.text());
    } catch {
      languageList = null;
    }
  }
  return languageList;
}

// The embosser models (embossers/embossers.txt beside the page).
let catalog = null;
async function embossers() {
  if (!core) {
    return null;
  }
  if (!catalog) {
    try {
      const response = await fetch("embossers/embossers.txt");
      if (!response.ok) {
        return null;
      }
      catalog = new core.Embossers(await response.text(), null);
    } catch {
      return null;
    }
  }
  return catalog;
}

// The braille encodings (fonts/encodings.json), in their listed order.
let encodingTable = null;
async function encodings() {
  if (!encodingTable) {
    try {
      encodingTable = (await (await fetch("fonts/encodings.json")).json()).sort((a, b) => a.order - b.order);
    } catch {
      encodingTable = [];
    }
  }
  return encodingTable;
}

// The output encodings: the byte sent for each of the 64 cells in an encoding, empty for North American English,
// which the byte pipeline sends itself; a character above one byte is sent as a space.
async function encodingCodes(name) {
  if (/^north american english$/i.test(name ?? "")) {
    return new Uint8Array(0);
  }
  const enc = (await encodings()).find((e) => e.name.toLowerCase() === (name ?? "").toLowerCase());
  if (!enc) {
    return new Uint8Array(0);
  }
  const codes = new Uint8Array(64);
  for (let p = 0; p < 64; p++) {
    const first = [...(enc.cells[p] ?? "")][0];
    const c = first ? first.codePointAt(0) : 0x2800 + p;
    codes[p] = c <= 0xff ? c : 0x20;
  }
  return codes;
}

// The template File, New gives a document (the built-in default, "English (UEB) - basic"): its text, read once
// from templates/ beside the page.
let defaultTemplate = null;
let defaultTemplateName = null;
async function templateText() {
  // The template File, New marked as the default (kept as DefaultTemplate), else the built-in default.
  const want = store.get("DefaultTemplate") ?? "English (UEB) - basic";
  if (defaultTemplate === null || defaultTemplateName !== want) {
    defaultTemplateName = want;
    try {
      const index = await (await fetch("templates/index.tsv")).text();
      const rows = index.split("\n").map((l) => l.split("\t"));
      const row = rows.find((c) => (c[2] ?? "").trim() === want) ?? rows.find((c) => (c[2] ?? "").trim() === "English (UEB) - basic");
      defaultTemplate = row ? await (await fetch(`templates/${row[1]}.template`)).text() : "";
    } catch {
      defaultTemplate = "";
    }
  }
  return defaultTemplate;
}

async function fetchBytes(path) {
  const response = await fetch(path);
  if (!response.ok) {
    throw new Error(`${path} was not found.`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

// ------------------------------------------------------------------ kept settings

// The browser keeps this viewer's settings (the desktop program keeps them in its settings file). A browser that
// keeps nothing (a private window) still works; the settings then last until the page is closed.
const memory = new Map();
const store = {
  get(key) {
    try {
      return localStorage.getItem(`braillecraft.${key}`) ?? memory.get(key) ?? null;
    } catch {
      return memory.get(key) ?? null;
    }
  },
  set(key, value) {
    memory.set(key, value);
    try {
      if (value === null) {
        localStorage.removeItem(`braillecraft.${key}`);
      } else {
        localStorage.setItem(`braillecraft.${key}`, value);
      }
    } catch {
      // Kept in memory only.
    }
  },
  json(key, fallback) {
    try {
      const v = this.get(key);
      return v ? JSON.parse(v) : fallback;
    } catch {
      return fallback;
    }
  },
  // Every key of `section` (keys named "section.key") taken out.
  clearSection(section) {
    const prefix = `${section}.`;
    for (const k of [...memory.keys()]) if (k.startsWith(prefix)) memory.delete(k);
    try {
      const full = `braillecraft.${prefix}`;
      for (let i = localStorage.length - 1; i >= 0; i--) {
        const k = localStorage.key(i);
        if (k?.startsWith(full)) localStorage.removeItem(k);
      }
    } catch {
      // Kept in memory only.
    }
  },
};

// Global, Notification Settings, OK: every kept "do not show this again" answer is forgotten
// these sections.
async function notificationSettings() {
  const { result } = await showDialog(1023);
  if (result === "ok") for (const s of ["Mutable", "AddIns", "EmbossTV", "EmbossTVTemp"]) store.clearSection(s);
}

// Embosser setups, as the desktop program keeps them: the list of names, one value per setup, the default setup,
// and the layout new documents take with its cells and lines.
const NEW_LAYOUT = { form: "", top_margin: 0, binding: 0, interpoint: false, banner: false, interline: 0 };
const setupNames = () => store.json("embosser.setups", []);
const loadSetup = (name) => store.json(`embosser.setup.${name}`, null);
const defaultSetup = () => store.get("embosser.default") || "";
const setDefaultSetup = (name) => store.set("embosser.default", name);
const defaultLayout = () => store.json("embosser.layout", [NEW_LAYOUT, 40, 25]);
const saveDefaultLayout = (layout, cells, lines) => store.set("embosser.layout", JSON.stringify([layout, cells, lines]));
const shownName = (s) => (s.name.trim() ? s.name : s.model);

function saveSetup(setup, old) {
  const name = shownName(setup);
  let names = setupNames();
  if (old && old !== name) {
    names = names.filter((n) => n !== old);
    store.set(`embosser.setup.${old}`, null);
    if (defaultSetup() === old) {
      setDefaultSetup(name);
    }
  }
  if (!names.includes(name)) {
    names.push(name);
  }
  store.set("embosser.setups", JSON.stringify(names));
  store.set(`embosser.setup.${name}`, JSON.stringify(setup));
}

function removeSetup(name) {
  store.set("embosser.setups", JSON.stringify(setupNames().filter((n) => n !== name)));
  store.set(`embosser.setup.${name}`, null);
  if (defaultSetup() === name) {
    setDefaultSetup("");
  }
}

// New setups take the European order of default forms when the browser's region is one counted as
// European (its country code begins with 3 or 4, spec 25 12.3); a language without a region takes its likely one.
const european = () => {
  try {
    const region = new Intl.Locale(navigator.language || "en-US").maximize().region || "";
    return core.a4Region(region);
  } catch {
    return false;
  }
};

// ------------------------------------------------------------------ the document

// Global settings, as the desktop program keeps them; the defaults are those of spec 18.
const DEFAULT_VIEWS = {
  translated_font: "print",
  print_full_page: false,
  print_codes: false,
  print_translated: false,
  braille_font: "braille", // braille documents open in the braille font (owner direction, queue line T4)
  braille_full_page: false,
  braille_codes: false,
  braille_translated: true,
  six_key: true,
};
const DEFAULT_FONTS = {
  print_face: "Courier New",
  print_size: 12,
  braille_size: 18,
  status_face: "Segoe UI",
  status_size: 12,
  print_auto: true,
  print_output_size: 12,
  braille_output_size: 18,
};
const DEFAULT_SHORTCUTS = {
  grade0: "[cz]",
  flush_right_leader: '[fr;p~"]',
  cbc: "[cb]",
  new_line: "[<]",
  new_page: "[pg]",
  new_ref_page: "[lea]",
  page_nav: 0,
  control_page_nav: 1,
};
const settings = {
  views: { ...DEFAULT_VIEWS, ...store.json("views.default", {}) },
  fonts: { ...DEFAULT_FONTS, ...store.json("fonts", {}) },
  shortcuts: { ...DEFAULT_SHORTCUTS, ...store.json("shortcuts", {}) },
  encoding: store.get("encoding") || "north-american",
  uia: store.get("uia") === "1",
  // Global, Internationalization: braille shown as Unicode braille patterns, and reported to screen readers so.
  preferPatterns: store.get("PreferBraillePatterns") === "1",
  accessPatterns: store.get("AccessUseBraillePatterns") === "1",
  verbose: store.get("verbose") === "1",
  sixKeySet: store.get("sixkey.set") || "fdsjkl",
  // Global, View Preferences: line numbers beside a braille page (printed; spec 24 3d).
  lineNumbers: store.get("Display.ShowLineNumbers") === "yes",
};

// A new document's view, from Global, Default Views.
function newView(braille) {
  const v = settings.views;
  return {
    font: braille ? v.braille_font : "print",
    full_page: braille ? v.braille_full_page : v.print_full_page,
    codes: braille ? v.braille_codes : v.print_codes,
    translated: braille ? v.braille_translated : v.print_translated,
    six_key: braille && v.six_key,
  };
}

const doc = { kind: "print", name: "Untitled Print Document 1", file: null, view: newView(false) };
// Whether the document file last opened was in the recent documents list.
let inRecentList = false;

// A new document takes the default embosser setup and layout, as in the desktop program.
function embosserDefaults() {
  const [layout, cells, lines] = defaultLayout();
  doc.embosser = defaultSetup();
  doc.layout = { ...layout };
  doc.cells = cells;
  doc.lines = lines;
  // A print document's embosser page, kept apart from its print page (spec 05 section A10).
  doc.embPage = [cells, lines];
  // A braille document's own Printer Setup page (spec A16.2): the Global, Printer Setup page size to start with.
  doc.printPage = globalPrintPage();
  doc.doubleSize = 0;
}

// The Global, Printer Setup page size, [lines per page, characters per line] (54 and 65 until it is changed).
function globalPrintPage() {
  if (!core) return [54, 65];
  const view = JSON.parse(core.printer_setup_view(store.get("printer.setup") || ""));
  return [view.lines, view.chars];
}
embosserDefaults();

function kindName() {
  return doc.kind === "braille" ? "Braille Document" : "Print Document";
}

function showTitle() {
  const title = `BrailleCraft - [${doc.name} -- ${kindName()}]`;
  $("doc-title").textContent = title;
  document.title = title;
}

// ------------------------------------------------------------------ the shown encoding, fonts and the view

// The braille encoding documents are shown in (Global, Internationalization), once encodings.json is read.
let currentEncoding = null;
async function loadEncoding() {
  const all = await encodings();
  currentEncoding = all.find((e) => e.id === settings.encoding) ?? all.find((e) => e.id === "north-american") ?? null;
}
const encodingNow = () => currentEncoding ?? { id: "north-american", name: "North American English", cells: [] };
const naCells = () => encodingTable?.find((e) => e.id === "north-american")?.cells ?? [];

// Braille is kept, translated, saved and embossed in North American ASCII braille, whatever encoding it is shown in;
// these convert the shown text to it and back, codes keeping their text.
function toNA(text) {
  const e = encodingNow();
  return core && e.id !== "north-american" && e.cells.length ? core.convert_braille(text, JSON.stringify(e.cells), JSON.stringify(naCells())) : text;
}
function fromNA(text) {
  const e = encodingNow();
  return core && e.id !== "north-american" && e.cells.length ? core.convert_braille(text, JSON.stringify(naCells()), JSON.stringify(e.cells)) : text;
}

// The character the shown encoding gives a cell (dot pattern), Unicode braille when it lists none.
function cellChar(pattern) {
  return [...(encodingNow().cells[pattern] ?? "")][0] ?? String.fromCodePoint(0x2800 + pattern);
}

// Global, Internationalization, "Prefer Unicode Braille Patterns for Braille Display": a braille document shown in a
// braille font shows its cells as Unicode braille patterns read through the shown encoding (a character the
// encoding does not list is a blank cell), as the desktop program draws them; codes keep their text. The document
// keeps its own characters: docText holds the document while the text field shows the patterns, and an edit made
// in the field is taken as the change between the text last shown and the text now, its inserted part read back
// through the encoding, so the unchanged characters stay exactly as they were. On the web the text screen readers
// read is the text shown, so the dialog's screen reader choice follows the display here.
let docText = "";
let shownText = "";
let patternsShown = false;
function patternsOn() {
  return doc.kind === "braille" && settings.preferPatterns && doc.view.font !== "print";
}
function cellIndex() {
  const m = new Map();
  encodingNow().cells.forEach((chars, p) => {
    for (const c of chars ?? "") if (!m.has(c)) m.set(c, p);
  });
  return m;
}
// Every character of `text` as its pattern, but the code spans (UTF-16 ranges, from the core's tracker when
// `codes` is not given) and line breaks.
function toPatterns(text, codes) {
  const cellOf = cellIndex();
  const spans = codes ?? (core ? core.editor_codes() : new Uint32Array(0));
  let out = "";
  let c = 0;
  for (let i = 0; i < text.length; i++) {
    while (c < spans.length && spans[c + 1] <= i) c += 2;
    const ch = text[i];
    if ((c < spans.length && spans[c] <= i && i < spans[c + 1]) || ch === "\n") {
      out += ch;
      continue;
    }
    const p = cellOf.get(ch);
    out += p === undefined ? "⠀" : String.fromCodePoint(0x2800 + p);
  }
  return out;
}
function fromPatterns(text) {
  return text.replace(/[⠀-⠿]/g, (ch) => cellChar(ch.codePointAt(0) - 0x2800));
}
// The document's text (what the core and the files work on) and the text field's content.
function editorText() {
  return patternsShown ? docText : editor.value;
}
function setEditorText(t) {
  docText = t;
  patternsShown = patternsOn();
  editor.value = patternsShown ? toPatterns(t) : t;
  shownText = editor.value;
}
// After an edit in the field: the document takes the change, and the field shows the patterns again with the
// caret where it was.
function takeEdit() {
  if (!patternsShown) return;
  const now = editor.value;
  if (now === shownText) return;
  const n = Math.min(now.length, shownText.length);
  let a = 0;
  while (a < n && now[a] === shownText[a]) a++;
  let b = 0;
  while (b < n - a && now[now.length - 1 - b] === shownText[shownText.length - 1 - b]) b++;
  const inserted = fromPatterns(now.slice(a, now.length - b));
  docText = docText.slice(0, a) + inserted + docText.slice(shownText.length - b);
  const [s, e, top] = [editor.selectionStart, editor.selectionEnd, editor.scrollTop];
  editor.value = toPatterns(docText);
  shownText = editor.value;
  editor.setSelectionRange(s, e);
  editor.scrollTop = top;
}
// The field shown again as the view and the settings now ask (a view font or encoding change), the caret kept.
function reshowEditor() {
  const [s, e, top] = [editor.selectionStart, editor.selectionEnd, editor.scrollTop];
  setEditorText(editorText());
  editor.setSelectionRange(s, e);
  editor.scrollTop = top;
}
// A line of braille (the translated line of a print document) as patterns, its bracketed codes kept.
function linePatterns(text) {
  const codes = [];
  const re = /\[[^\]\n]*\]/g;
  for (let m = re.exec(text); m; m = re.exec(text)) codes.push(m.index, m.index + m[0].length);
  return toPatterns(text, Uint32Array.from(codes));
}

// The braille fonts of the shown encoding ("BrailleCraft Braille North American English" and its SimBraille),
// loaded from fonts/ beside the page when first used.
const facesAdded = new Set();
function brailleFamily(kind) {
  const e = encodingNow();
  const family = `BrailleCraft ${kind} ${e.name}`;
  if (!facesAdded.has(family) && "FontFace" in window) {
    facesAdded.add(family);
    const face = new FontFace(family, `url("fonts/BrailleCraft-${kind}-${e.id}.ttf")`);
    document.fonts.add(face);
    face.load().catch(() => {});
  }
  return family;
}

// A view font's CSS family and point size (Global, Fonts). A braille view font is shown in "RNIB Moon" when the
// document's print-to-braille table is a Moon table (its name starts with "ENMOCP") and that font is installed
// (spec A16.1).
function faceOf(font) {
  const f = settings.fonts;
  if (font === "print") return [`"${f.print_face}", monospace`, f.print_size];
  const own = `"${brailleFamily(font === "braille" ? "Braille" : "SimBraille")}", monospace`;
  const table = doc.file?.tables?.[0] ?? "";
  if (table.slice(0, 6).toUpperCase() === "ENMOCP" && moonInstalled()) return [`"RNIB Moon", ${own}`, f.braille_size];
  return [own, f.braille_size];
}

// The Full-Page font size (spec 27 section 9, unknown 3), as the desktop program fits it: sizes 5 to 48 are tried
// in turn; the first at which the window's width holds no more than the page's cells, or the editing area's height
// no more than its lines, stops the search and the size before it is taken (never less than 5); 47 when none stops
// it. INFERENCE: the character measured is "x"; the measured string is not known.
function fullPageSize(face) {
  const ctx = (fullPageSize.canvas ??= document.createElement("canvas")).getContext("2d");
  if (!ctx) return 6;
  for (let size = 5; size <= 48; size++) {
    ctx.font = `${size}pt ${face}`;
    const m = ctx.measureText("x");
    const width = Math.max(m.width, 1);
    const height = Math.max((m.fontBoundingBoxAscent ?? size) + (m.fontBoundingBoxDescent ?? size / 3), 1);
    const cols = Math.floor(window.innerWidth / width);
    const rows = Math.floor(editor.clientHeight / height);
    if (cols <= doc.cells || rows <= doc.lines) return Math.max(size - 1, 5);
  }
  return 47;
}

// The document's view applied: its font (sized so one whole page fits the window in Full-Page view), the translated
// line's font (Global, Default Views, for print documents; the print font for braille documents) and whether it
// shows, the status line's font, and the View menu's marks.
function applyView() {
  const v = doc.view;
  let [face, size] = faceOf(v.font);
  // Fitted with Full-Page on: the print font only outside the coded view, the braille fonts in either view.
  if (v.full_page && (v.font !== "print" || !v.codes)) size = fullPageSize(face);
  editor.style.fontFamily = face;
  editor.style.fontSize = `${size}pt`;
  const [lineFace, lineSize] = faceOf(doc.kind === "print" ? settings.views.translated_font : "print");
  translated.style.fontFamily = lineFace;
  translated.style.fontSize = `${lineSize}pt`;
  $("translated-section").hidden = !v.translated;
  statusLine.style.fontFamily = `"${settings.fonts.status_face}", system-ui, sans-serif`;
  statusLine.style.fontSize = `${settings.fonts.status_size}pt`;
  if (patternsShown !== patternsOn()) reshowEditor();
  refreshMenus();
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

// The menus' check marks follow the document: highlighting, its view, and its hyphenation.
function refreshMenus() {
  const v = doc.view;
  const marks = {
    1209: doc.highlight !== null && doc.highlight !== undefined,
    1301: v.full_page,
    1303: v.font === "print",
    1304: v.font === "braille",
    1305: v.font === "simbraille",
    1307: v.six_key,
    1309: v.codes,
    1310: v.translated,
  };
  for (let id = 2901; id <= 2905; id++) marks[id] = (doc.hyphen ?? 2902) === id;
  // Document, Translation and Learning Tables follow the document's tables (filled when the list has loaded).
  refreshTableMenus();
  for (const [id, on] of Object.entries(marks)) {
    menubar.querySelector(`[data-id="${id}"]`)?.setAttribute("aria-checked", String(on));
  }
}

function caretLine() {
  const before = editorText().slice(0, editor.selectionStart);
  const line = before.split("\n").length;
  const column = before.length - before.lastIndexOf("\n");
  const start = before.lastIndexOf("\n") + 1;
  const end = editorText().indexOf("\n", start);
  return { line, column, text: editorText().split("\n")[line - 1] ?? "", start, end: end < 0 ? editorText().length : end };
}

// The status line, as the desktop program shows it (bc_core::status): the current style, Page, Line and Column on the
// laid-out pages (left out in coded view), the character at the caret, and Keys.
// Gives the core the active document's page numbering, which its pages are laid out with. A new document takes
// its template's, once its document file is made.
function syncNumbering() {
  syncHyphenation();
  if (doc.file) {
    core.set_page_numbering(Uint32Array.from(doc.file.page_numbers));
    return;
  }
  core.set_page_numbering(new Uint32Array(5));
  const d = doc;
  documentFile().then(() => {
    if (d === doc) showStatus();
  });
}

function showStatus() {
  if (!core) {
    const { line, column } = caretLine();
    statusLine.textContent = `Current Style: None  Line:${line}  Column:${column}`;
    return;
  }
  syncNumbering();
  // As in the desktop program, the text as it is shown (in the shown encoding) is laid out.
  const status = core.status_line(editorText(), doc.cells, doc.lines, doc.kind === "braille", editor.selectionStart, doc.view.codes, settings.verbose, doc.view.six_key, doc.highlight !== null && doc.highlight !== undefined);
  if (statusLine.textContent !== status) statusLine.textContent = status;
  // Table, Create is enabled outside a table and Table Properties inside one (spec 33 unknown 6).
  const inTable = core.editor_in_table(editor.selectionStart);
  menubar.querySelector('[data-id="1501"]')?.setAttribute("aria-disabled", String(inTable));
  menubar.querySelector('[data-id="1503"]')?.setAttribute("aria-disabled", String(!inTable));
}

// Edit, Highlight: from where it was turned on, the selection follows the caret.
function followHighlight() {
  const anchor = doc.highlight;
  if (anchor === null || anchor === undefined) return;
  const at = editor.selectionStart === editor.selectionEnd ? editor.selectionStart : null;
  if (at !== null && at !== anchor) editor.setSelectionRange(Math.min(at, anchor), Math.max(at, anchor), at < anchor ? "backward" : "forward");
}

// View, Translated Line, as the desktop program's: the caret's laid-out page line (the formatted view) or coded row
// (the coded view), translated with the codes before it in force (bc_core::translated_line): those codes are
// translated ahead of the line and what they write alone is left off. What they write alone is kept while the caret
// moves among lines with the same codes before them. `lastLine` set to null asks for the line again.
let lastLine = null;
let lineLead = { key: null, lead: "" };
async function showTranslatedLine() {
  if (!doc.view.translated || !core) {
    return;
  }
  if (lastLine === null) lineLead = { key: null, lead: "" };
  const braille = doc.kind === "braille";
  const width = braille ? 42 : 65;
  const at = editor.selectionStart;
  const parts = JSON.parse(core.translated_line_parts(at, doc.cells, doc.lines, braille, !!doc.view.codes, width));
  const key = `${parts.carried}\u0000${parts.line}`;
  if (key === lastLine) {
    return;
  }
  lastLine = key;
  // The document's own tables, initial codes and styles, as the desktop program's translated line.
  const { e, file } = await documentEngine();
  if (!e) {
    translated.textContent = "";
    return;
  }
  try {
    // The result keeps its own codes apart from braille brackets.
    const run = async (text) =>
      braille ? translateText(e, toNA(text), file, true, true) : translateText(e, text, file, false, true);
    let out;
    if (!parts.carried) {
      out = await run(parts.line);
    } else {
      const leadKey = `${file.tables.join("+")}|${file.initial_codes}|${parts.carried}`;
      if (lineLead.key !== leadKey) lineLead = { key: leadKey, lead: await run(parts.carried) };
      out = core.leave_off_lead(lineLead.lead, await run(parts.whole));
    }
    // A print document's braille, shown in a braille font, as Unicode braille patterns when they are preferred.
    const patterns = doc.kind === "print" && settings.preferPatterns && settings.views.translated_font !== "print";
    const line = braille ? core.running_text(out) : fromNA(core.running_text(out));
    translated.textContent = patterns ? linePatterns(line) : line;
  } catch (err) {
    translated.textContent = String(err.message ?? err);
  }
}

// Six-key entry (bc_core::sixkey, through the core's SixKey): the six keys of Global, View Preferences are dots 1 to
// 6, named by their place on the keyboard (KeyboardEvent.code). With View, Six-Key Entry on they chord with Shift up;
// in a braille document with it off, Shift and the six keys chord. The cell is typed when the first chord key comes
// up; Space during a chord types the cell at once and a space when the chord key comes up; a held chord key types
// the cell again on each repeat; the other letter keys do nothing while chording. Control, Alt and Meta keys go on.
let editorChord = null;
function sixKeyTyped(r, field) {
  const c = r === 64 ? " " : cellChar(r);
  field.setRangeText(c, field.selectionStart, field.selectionEnd, "end");
  field.dispatchEvent(new Event("input"));
}
function sixKeyDown(e, field, chord) {
  if (!core || !chord || e.ctrlKey || e.altKey || e.metaKey) return;
  const braille = doc.kind === "braille";
  const r = chord.key_down(e.code, settings.sixKeySet, braille, !!doc.view.six_key, e.shiftKey, e.repeat);
  if (r === -1) return;
  e.preventDefault();
  if (r >= 0) sixKeyTyped(r, field);
}
function sixKeyUp(e, field, chord) {
  if (!core || !chord) return;
  const r = chord.key_up(e.code, settings.sixKeySet);
  if (r === -1) return;
  e.preventDefault();
  if (r >= 0) sixKeyTyped(r, field);
}
editor.addEventListener("keydown", (e) => {
  if (core && !editorChord) editorChord = new core.SixKey();
  sixKeyDown(e, editor, editorChord);
});
// Six-key entry in a dialog field: Edit, Find and the Header and Footer dialogs take the keys as the editing area
// does, with a chord of their own; the other dialogs type as usual.
function sixKeyField(field) {
  if (!core || !field) return;
  const chord = new core.SixKey();
  field.addEventListener("keydown", (e) => sixKeyDown(e, field, chord));
  field.addEventListener("keyup", (e) => sixKeyUp(e, field, chord));
}
// The one-line prompts (dialog 30070), as in the desktop program: F9 or Ctrl+[ a code, Ctrl+F9 or Ctrl+< a style
// (Ctrl+Shift+Comma on US keyboards), Alt+F9 or Ctrl+] a character by its Unicode number.
editor.addEventListener("keydown", (e) => {
  if (!core || e.metaKey) return;
  const ctrl = e.ctrlKey && !e.altKey;
  let which = null;
  if (e.key === "F9") {
    if (e.altKey && !e.ctrlKey) which = "char";
    else if (ctrl) which = "style";
    else if (!e.ctrlKey && !e.altKey && !e.shiftKey) which = "code";
  } else if (ctrl && e.key === "[") which = "code";
  else if (ctrl && (e.key === "<" || (e.shiftKey && e.code === "Comma"))) which = "style";
  else if (ctrl && e.key === "]") which = "char";
  if (!which) return;
  e.preventDefault();
  promptKey(which);
});

// A prompt key: the prompt asks for one line, its default the code (or style) at the caret, which the answer then
// replaces; else the answer goes in at the caret. An empty answer or Cancel puts nothing in; an answer refused gives
// its message.
async function promptKey(which) {
  const [question, max] = JSON.parse(core.prompt_texts())[which];
  const at = editor.selectionStart;
  const def =
    which === "char"
      ? { text: "", from: null, to: null }
      : JSON.parse(core.editor_prompt_default(which === "style", at, doc.view.codes));
  const answer = await askText(question, def.text, max);
  editor.focus();
  if (answer === null) return;
  const out = JSON.parse(core.prompt_answer(which, answer));
  if (out.message) {
    await message(out.message);
    editor.focus();
    return;
  }
  if (out.put === null) return;
  if (def.from !== null) editor.setSelectionRange(def.from, def.to);
  if (out.code) insertCodes(out.put);
  else insertText(out.put);
}

// Dialog 30070: `prompt` in its label and `def` in its field, all of it selected, with focus there. Answers the text
// typed, cut to `max` characters (the field itself takes any length), or null for Cancel. INFERENCE: the title is
// the resource's (empty); whether a title is set as it opens is not known.
async function askText(prompt, def, max) {
  const { result, d } = await showDialog("30070", {
    focus: 5,
    setup(d) {
      d.setText(4, prompt);
      d.setText(5, def);
      // With no title, the dialog is named by its prompt for screen readers.
      const dialog = d.get(5)?.closest("dialog");
      const heading = dialog?.querySelector("h2");
      if (dialog && heading && !heading.textContent.trim()) {
        heading.hidden = true;
        dialog.removeAttribute("aria-labelledby");
        dialog.setAttribute("aria-label", prompt);
      }
    },
  });
  return result === "ok" ? [...d.text(5)].slice(0, max).join("") : null;
}

editor.addEventListener("keyup", (e) => sixKeyUp(e, editor, editorChord));

editor.addEventListener("input", () => {
  // The document takes the edit (the field may show braille patterns), then the codes follow it
  // (bc_core::doctext, as in the desktop program); a code that lost a bracket goes as a whole, last first, and
  // that change comes back here.
  takeEdit();
  if (core) {
    const remains = core.editor_edit(editorText());
    if (remains.length) {
      for (let k = 0; k < remains.length; k += 2) editor.setRangeText("", remains[k], remains[k + 1], "end");
      editor.dispatchEvent(new Event("input"));
      return;
    }
  }
  doc.dirty = true;
  showStatus();
  showTranslatedLine();
});
// Copy and cut keep the selection with its codes, so pasting the same text brings them back.
for (const kind of ["copy", "cut"]) {
  editor.addEventListener(kind, () => core?.editor_remember(editor.selectionStart, editor.selectionEnd));
}
// Leaving the page with changes not saved: the browser asks, as the desktop program does at Exit.
window.addEventListener("beforeunload", (e) => {
  if (doc.dirty) e.preventDefault();
});
editor.addEventListener("keyup", () => {
  followHighlight();
  showStatus();
  showTranslatedLine();
});
editor.addEventListener("click", () => {
  followHighlight();
  showStatus();
});

function download(name, bytes, type = "application/octet-stream") {
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
}

function stem() {
  return doc.name.replace(/\.[^.]*$/, "");
}

// ------------------------------------------------------------------ dialogs

// A label's text without its mnemonic marker, and the mnemonic letter.
function plain(label) {
  return (label ?? "").replace(/&(.)/g, "$1");
}
function mnemonic(label) {
  const m = /(?<!&)&([^&])/.exec(label ?? "");
  return m ? m[1].toUpperCase() : null;
}

// The controls of dialog or sheet page `rid` built into `form`, in resource order: each group box a
// fieldset around the controls inside it, each label tied to the control after it, radio buttons in their groups.
// OK and Cancel (ids 1 and 2) are left to the caller. Returns the controls by id (labels and texts included) and
// the Alt+letter keys.
function buildControls(rid, form) {
  const spec = DIALOGS[rid];
  const boxes = spec.controls.filter((c) => c.kind === "box");
  const inside = (c, b) => c.x >= b.x && c.y >= b.y && c.x < b.x + b.w && c.y < b.y + b.h;
  const fieldsets = new Map();
  const containerOf = (c) => {
    const box = boxes.filter((b) => b !== c && inside(c, b)).sort((a, b) => a.w * a.h - b.w * b.h)[0];
    if (!box) {
      return form;
    }
    if (!fieldsets.has(box.id)) {
      const fs = document.createElement("fieldset");
      const legend = document.createElement("legend");
      legend.textContent = plain(box.label);
      fs.append(legend);
      fs.dataset.mnemonic = mnemonic(box.label) ?? "";
      containerOf(box).append(fs);
      fieldsets.set(box.id, fs);
    }
    return fieldsets.get(box.id);
  };
  const keys = [];
  const controls = new Map();
  let group = null;
  let pendingLabel = null;
  const order = spec.controls.filter((c) => c.id !== 1 && c.id !== 2);
  for (const c of order) {
    if (c.kind !== "radio") {
      group = null;
    }
    if (c.kind === "box") {
      containerOf(c);
      continue;
    }
    const where = containerOf(c);
    const id = `dlg${rid}-c${c.id}`;
    let el;
    if (c.kind === "static") {
      const next = order[order.indexOf(c) + 1];
      if (next && ["text", "list", "combo"].includes(next.kind)) {
        pendingLabel = c;
        continue;
      }
      el = document.createElement("p");
      el.className = "text";
      el.textContent = plain(c.label);
      controls.set(c.id, el);
    } else if (c.kind === "text" || c.kind === "list" || c.kind === "combo") {
      const row = document.createElement("div");
      row.className = "row";
      const field = c.kind === "text" ? document.createElement("input") : document.createElement("select");
      field.id = id;
      if (c.kind === "text") {
        field.type = "text";
        field.value = c.value ?? "";
      }
      if (c.kind === "list") {
        field.size = 6;
      }
      if (pendingLabel && !plain(pendingLabel.label).trim()) {
        // A blank label (some are filled at run time) names nothing: the field is named by the text
        // before it, else its group.
        controls.set(pendingLabel.id, document.createElement("span"));
        pendingLabel = null;
        const before = [...where.querySelectorAll(":scope > p.text")].at(-1)?.textContent;
        field.setAttribute("aria-label", before ?? where.querySelector("legend")?.textContent ?? plain(spec.title));
      } else if (pendingLabel) {
        const label = document.createElement("label");
        label.htmlFor = id;
        label.textContent = plain(pendingLabel.label);
        row.append(label);
        controls.set(pendingLabel.id, label);
        const k = mnemonic(pendingLabel.label);
        if (k) {
          keys.push([k, field]);
        }
        pendingLabel = null;
      } else {
        // A field with no label before it that stands on the row of a radio button or check box to its left (as Send
        // to Printer: and its printer list do) is named by that control and placed beside it;
        // any other is named by its group, as a screen reader would read it there.
        const beside = spec.controls.find((r) => ["radio", "check"].includes(r.kind) && r.x < c.x && Math.abs(r.y - c.y) <= 3 && controls.has(r.id));
        if (beside) {
          field.setAttribute("aria-label", plain(beside.label));
          const label = controls.get(beside.id).closest("label");
          let pair = label.parentElement;
          if (!pair.classList.contains("pair")) {
            pair = document.createElement("div");
            pair.className = "pair";
            label.replaceWith(pair);
            pair.append(label);
          }
          row.append(field);
          pair.append(row);
          controls.set(c.id, field);
          if (c.hidden) row.hidden = true;
          if (!c.enabled) field.setAttribute("disabled", "");
          continue;
        }
        field.setAttribute("aria-label", where.querySelector("legend")?.textContent ?? plain(spec.title));
      }
      row.append(field);
      el = row;
      controls.set(c.id, field);
    } else if (c.kind === "check" || c.kind === "radio") {
      const label = document.createElement("label");
      label.className = "row";
      const input = document.createElement("input");
      input.type = c.kind === "check" ? "checkbox" : "radio";
      input.id = id;
      if (c.kind === "radio") {
        if (!group || (c.style ?? "").includes("wxRB_GROUP")) {
          group = `dlg${rid}-g${c.id}`;
        }
        input.name = group;
      }
      label.append(input, document.createTextNode(plain(c.label)));
      const k = mnemonic(c.label);
      if (k) {
        input.dataset.mnemonic = k;
        keys.push([k, input]);
      }
      el = label;
      controls.set(c.id, input);
    } else {
      const button = document.createElement("button");
      button.type = "button";
      button.id = id;
      button.textContent = plain(c.label);
      const k = mnemonic(c.label);
      if (k) {
        button.dataset.mnemonic = k;
        keys.push([k, button]);
      }
      el = button;
      controls.set(c.id, button);
    }
    if (c.hidden) {
      el.hidden = true;
    }
    if (!c.enabled) {
      controls.get(c.id)?.setAttribute("disabled", "");
    }
    where.append(el);
  }
  // Alt with a group's letter moves to the first control in the group.
  for (const fs of fieldsets.values()) {
    const first = fs.querySelector("input, select, button");
    if (fs.dataset.mnemonic && first) {
      keys.push([fs.dataset.mnemonic, first]);
    }
  }
  return { controls, keys };
}

// Helpers on a built dialog's controls (the desktop program's Dlg calls).
function dialogHelpers(controls) {
  const get = (id) => controls.get(id);
  return {
    get,
    text: (id) => get(id)?.value ?? "",
    setText(id, v) {
      const el = get(id);
      if (!el) return;
      if ("value" in el && el.tagName !== "BUTTON") el.value = v;
      else el.textContent = v;
    },
    number(id) {
      const v = (get(id)?.value ?? "").trim();
      return /^-?\d+$/.test(v) ? Number(v) : null;
    },
    checked: (id) => !!get(id)?.checked,
    setChecked(id, on) {
      if (get(id)) get(id).checked = !!on;
    },
    setItems(id, items, index) {
      const select = get(id);
      if (!select) return;
      select.replaceChildren(...items.map((t) => new Option(t, t)));
      select.selectedIndex = index ?? -1;
    },
    selected(id) {
      const s = get(id);
      return s && s.selectedIndex >= 0 ? s.options[s.selectedIndex].text : null;
    },
    chosenIn(id, items) {
      const t = this.selected(id);
      const i = items.indexOf(t);
      return i >= 0 ? i : null;
    },
    radio(ids) {
      const i = ids.findIndex((id) => get(id)?.checked);
      return i >= 0 ? i : null;
    },
    setRadio(ids, i) {
      ids.forEach((id, k) => this.setChecked(id, k === i));
    },
    enable(id, on) {
      const el = get(id);
      if (!el) return;
      if (["INPUT", "SELECT", "BUTTON", "TEXTAREA"].includes(el.tagName)) el.disabled = !on;
      else el.classList.toggle("disabled", !on);
    },
    hide(id) {
      const el = get(id);
      if (el) (el.closest(".row") ?? el).hidden = true;
    },
    show(id, on) {
      const el = get(id);
      if (el) (el.closest(".row") ?? el).hidden = !on;
    },
    // Every field's value, for the dialogs that remember what was chosen last (the desktop program's dialog memory).
    values() {
      const out = [];
      for (const [id, el] of controls) {
        if (el.tagName === "INPUT" && (el.type === "checkbox" || el.type === "radio")) out.push([id, el.checked]);
        else if (el.tagName === "INPUT") out.push([id, el.value]);
        else if (el.tagName === "SELECT") out.push([id, this.selected(id)]);
      }
      return out;
    },
    restore(values) {
      for (const [id, v] of values ?? []) {
        const el = get(id);
        if (!el) continue;
        if (typeof v === "boolean") el.checked = v;
        else if (el.tagName === "SELECT") {
          const i = [...el.options].findIndex((o) => o.text === v);
          if (i >= 0) el.selectedIndex = i;
        } else el.value = v;
      }
    },
  };
}

// Alt with a letter, in a dialog or on its shown page, as in the desktop program's dialogs: a button's letter presses
// it, a check box's or radio button's letter chooses it, and a label's or group's letter moves to its control.
function wireKeys(dialog, keysNow) {
  dialog.addEventListener("keydown", (e) => {
    if (e.altKey && !e.ctrlKey && e.key.length === 1) {
      const hit = keysNow().find(([k, el]) => k === e.key.toUpperCase() && !el.closest("[hidden]") && !el.disabled);
      if (hit) {
        e.preventDefault();
        const [letter, el] = hit;
        el.focus();
        // Only the control's own letter acts on it (a group's letter only moves to its first control).
        if (el.dataset.mnemonic === letter && !(el.type === "radio" && el.checked)) {
          el.click();
        }
      }
    }
  });
}

// OK and Cancel of dialog `rid` into `form`. OK (also Return in a field) runs `onOk`, which may keep the dialog open
// by answering false, as the desktop program's dialogs do after a message.
function addButtons(rid, dialog, form, controls, onOk) {
  const buttons = document.createElement("div");
  buttons.className = "buttons";
  for (const c of DIALOGS[rid].controls.filter((c) => c.id === 1 || c.id === 2)) {
    const button = document.createElement("button");
    button.textContent = plain(c.label);
    if (c.id === 1) {
      button.type = "submit";
      button.className = "default";
    } else {
      button.type = "button";
      button.addEventListener("click", () => dialog.close("cancel"));
    }
    buttons.append(button);
    controls.set(c.id, button);
  }
  form.append(buttons);
  let busy = false;
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (busy) return;
    busy = true;
    try {
      if (!onOk || (await onOk())) {
        dialog.close("ok");
      }
    } finally {
      busy = false;
    }
  });
}

function newDialog(titleText, idBase) {
  const dialog = document.createElement("dialog");
  const heading = document.createElement("h2");
  heading.id = `${idBase}-title`;
  heading.textContent = titleText;
  dialog.setAttribute("aria-labelledby", heading.id);
  dialog.append(heading);
  const form = document.createElement("form");
  form.method = "dialog";
  dialog.append(form);
  // Return in a list presses the default button, as in the desktop program's dialogs (a browser submits a form
  // only from a text field).
  form.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.altKey && !e.ctrlKey && e.target.tagName === "SELECT") {
      e.preventDefault();
      form.requestSubmit();
    }
  });
  return { dialog, form };
}

function closedPromise(dialog) {
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => {
      // "ok", the id of a button that closes the dialog itself (as a string), or "cancel".
      resolve(dialog.returnValue || "cancel");
      dialog.remove();
    });
  });
}

// A shown dialog placed a third of the way down the room the window leaves below it, centred across.
function placeThird(dialog) {
  const room = Math.max(window.innerHeight - dialog.offsetHeight, 0);
  dialog.style.marginTop = `${Math.floor(room / 3)}px`;
  dialog.style.marginBottom = "auto";
}

function focusFirst(container) {
  const first = [...container.querySelectorAll("input:not([type=hidden]), select, textarea, button")].find(
    (el) => !el.disabled && !el.closest("[hidden]"),
  );
  first?.focus();
  // A text field a dialog opens on has its text selected, as in the desktop program's dialogs: typing replaces it.
  if (first?.tagName === "INPUT" && first.type === "text") first.select();
}

// Dialog `rid` shown modally: `setup` fills it, `onOk` checks it. With `memory`, the values chosen last are put back
// after `setup` and kept on OK, as the desktop program's settings dialogs do. Answers how it was closed and its
// helpers.
// Dialog controls whose values are kept between runs as settings (spec 18 section 18.6): per
// dialog, [kind, control id, "Section.Key", words, default]. A flag is written "yes" or "no"; a choice (a combo box)
// and radios (ids from the given one on, one per word) are written as the word at the chosen index; text as typed;
// hundredths is a number shown with two decimals and kept in hundredths (the WordPerfect indent); seconds is shown as
// mm:ss (the autosave time). The defaults are the settings descriptor table's (spec 18 section 18.2b).
// The words of the HanScriptLanguage setting, by value (0 Mandarin to 3 Korean).
const HAN_LANGUAGES = ["Mandarin", "Cantonese", "Japanese", "Korean"];
const KEPT_SETTINGS = {
  1037: [
    ["choice", 7, "Hyperlinks.Action", ["url", "text", "both", "hyper"], 2],
    ["flag", 8, "Hyperlinks.RemoveDuplicates", null, true],
    ["flag", 19, "Hyperlinks.AutoTagCBC", null, true],
    ["flag", 20, "ImportOptions.AddSpaces", null, false],
    ["flag", 21, "ImportOptions.RemoveDashSpaces", null, false],
    ["choice", 23, "Word.UnknownCharacters", ["ignore", "asterisc", "hex"], 0],
    ["flag", 24, "Word.HanScriptAuto", null, true],
    // The setting has four words; Zhuyin is reached only through the zhtwcp table (spec A15.11). INFERENCE: the
    // dialog's fifth button, Zhuyin (30), has no word, so choosing it keeps the default.
    ["radios", 26, "Word.HanScriptLanguage", HAN_LANGUAGES, 0],
  ],
  1035: [
    ["flag", 6, "WordPerfect.CBCForCourier", null, false],
    ["flag", 7, "WordPerfect.PreserveHardPageBreaks", null, true],
    ["flag", 8, "WordPerfect.CompressSkippedLines", null, true],
    ["flag", 9, "WordPerfect.MarkPrintPageBreaks", null, false],
    ["radios", 11, "WordPerfect.ParagraphStyle", ["None", "SkipLine", "NewlineTag", "SkipAndTag"], 1],
    ["hundredths", 16, "Wordperfect.Indent", null, 25],
    ["radios", 18, "WordPerfect.Units", ["Inches", "Centimeters"], 0],
  ],
  // The UEB typeform mappings (controls 20 to 36) stay in the dialog memory: they are kept under
  // no key the descriptor table names.
  1036: [
    ["flag", 6, "Word.CBC", null, false],
    ["flag", 7, "Word.PreserveHardPageBreaks", null, true],
    ["flag", 8, "Word.PreserveSkippedLines", null, false],
    ["choice", 10, "Word.PrintPages", ["None", "Internal", "Scanned", "Publishers"], 0],
    ["flag", 11, "Word.BrailleFonts", null, false],
    ["flag", 12, "Word.AllowEmbeddedCodes", null, true],
    ["flag", 14, "Word.IgnoreBold", null, false],
    ["flag", 15, "Word.IgnoreItalics", null, false],
    ["flag", 16, "Word.IgnoreUnderline", null, false],
    ["flag", 17, "Word.IgnoreLanguageSwitches", null, true],
  ],
  // Global, File Locations. The two folders are kept as settings only: the browser's file pickers cannot be
  // started in a typed folder, and the autosaved documents are kept by the browser (IndexedDB), not in a folder.
  1060: [
    ["text", 7, "AutoSave.openPath", null, ""],
    ["text", 9, "AutoSave.savePath", null, ""],
    ["flag", 10, "AutoSave.Enabled", null, true],
    ["text", 12, "AutoSave.path", null, "[MYDOCS]\\BrailleCraft Autosaves"],
    ["seconds", 14, "AutoSave.Time", null, 300],
  ],
};

const two = (n) => String(n).padStart(2, "0");

// The autosave time as File Locations shows it: whole minutes alone (300 is "5"), any other time as minutes and
// two-digit seconds (90 is "1:30").
const secondsText = (s) => (s % 60 === 0 ? String(s / 60) : `${Math.trunc(s / 60)}:${two(Math.abs(s % 60))}`);

// A time typed as "m:s" or as minutes alone, read as C's "%d:%d" and "%d" read it (text after the numbers is left);
// null when it starts with no number.
function parseSeconds(text) {
  const m = /^\s*([-+]?\d+)(?::\s*([-+]?\d+))?/.exec(text);
  return m ? Number(m[1]) * 60 + (m[2] === undefined ? 0 : Number(m[2])) : null;
}

// Global, File Locations: the autosave controls (11 to 14) only while Enable autosave (10) is checked, and OK only
// while it is unchecked or the time reads as more than no time, checked again on every change of the time.
function fileLocationsEnabling(d) {
  const update = () => {
    const on = d.checked(10);
    for (let id = 11; id <= 14; id++) d.enable(id, on);
    d.enable(1, !on || (parseSeconds(d.text(14)) ?? 0) > 0);
  };
  update();
  d.get(10)?.addEventListener("change", update);
  d.get(14)?.addEventListener("input", update);
}

// The dialog's controls set from the kept settings, or their defaults.
function restoreKept(d, kept) {
  const word = (key, words, dflt) => {
    const v = (store.get(key) ?? "").trim().toLowerCase();
    const i = words.findIndex((w) => w.toLowerCase() === v);
    return i >= 0 ? i : dflt;
  };
  for (const [kind, id, key, words, dflt] of kept) {
    const v = store.get(key);
    switch (kind) {
      case "flag":
        d.setChecked(id, v === null ? dflt : v.trim().toLowerCase() === "yes");
        break;
      case "choice": {
        const s = d.get(id);
        if (s) s.selectedIndex = word(key, words, dflt);
        break;
      }
      case "radios":
        d.setRadio(words.map((_, k) => id + k), word(key, words, dflt));
        break;
      case "text":
        d.setText(id, v ?? dflt);
        break;
      case "hundredths": {
        const n = v !== null && /^-?\d+$/.test(v.trim()) ? Number(v) : dflt;
        d.setText(id, `${Math.trunc(n / 100)}.${two(Math.abs(n % 100))}`);
        break;
      }
      case "seconds":
        d.setText(id, secondsText(v !== null && /^-?\d+$/.test(v.trim()) ? Number(v) : dflt));
        break;
    }
  }
}

// The dialog's controls written to the kept settings (on OK).
function saveKept(d, kept) {
  for (const [kind, id, key, words, dflt] of kept) {
    switch (kind) {
      case "flag":
        store.set(key, d.checked(id) ? "yes" : "no");
        break;
      case "choice": {
        const i = d.get(id)?.selectedIndex ?? -1;
        store.set(key, words[i >= 0 && i < words.length ? i : dflt]);
        break;
      }
      case "radios": {
        const i = d.radio(words.map((_, k) => id + k));
        store.set(key, words[i ?? dflt]);
        break;
      }
      case "text":
        store.set(key, d.text(id));
        break;
      case "hundredths": {
        const x = Number(d.text(id).trim());
        store.set(key, String(Number.isFinite(x) && d.text(id).trim() !== "" ? Math.round(x * 100) : dflt));
        break;
      }
      case "seconds":
        store.set(key, String(parseSeconds(d.text(id)) ?? dflt));
        break;
    }
  }
}

// With `focus`, focus starts in that control (its text selected) in place of the first one; with `third`, the
// dialog stands a third of the way down the room the window has below it, as the desktop program places
// Code List, Page Numbering, Shortcut Preferences and the learning step list.
async function showDialog(rid, { title, setup, onOk, memory, after, closes = [], focus, third } = {}) {
  const { dialog, form } = newDialog(title ?? DIALOGS[rid].title, `dlg${rid}`);
  const { controls, keys } = buildControls(rid, form);
  const d = dialogHelpers(controls);
  addButtons(rid, dialog, form, controls, onOk && (() => onOk(d)));
  // Buttons that close the dialog with their own id (the desktop program's closes_with).
  for (const id of closes) d.get(id)?.addEventListener("click", () => dialog.close(String(id)));
  wireKeys(dialog, () => keys);
  $("dialogs").append(dialog);
  const closed = closedPromise(dialog);
  setup?.(d);
  if (memory !== undefined) d.restore(store.json(`dialog.${rid}.${memory}`, null));
  // The settings kept between runs, over the dialog memory (spec 18 section 18.6).
  const kept = KEPT_SETTINGS[rid];
  if (kept) restoreKept(d, kept);
  after?.(d);
  dialog.showModal();
  if (third) placeThird(dialog);
  const start = focus !== undefined ? d.get(focus) : null;
  if (start) {
    start.focus();
    start.select?.();
  } else {
    focusFirst(form);
  }
  const result = await closed;
  if (result === "ok" && kept) saveKept(d, kept);
  if (result === "ok" && memory !== undefined) store.set(`dialog.${rid}.${memory}`, JSON.stringify(d.values()));
  return { result, d };
}

// A message box (the desktop program's message, yes_no, yes_no_cancel and ok_cancel): OK, or Yes and No, or Yes, No
// and Cancel, or OK and Cancel; Escape answers No, or Cancel when there is one. Answers "ok", "no" or "cancel".
async function messageBox(text, yesNo = false, cancel = false, okCancel = false) {
  const { dialog, form } = newDialog("BrailleCraft", "message");
  dialog.setAttribute("role", "alertdialog");
  const p = document.createElement("p");
  p.className = "message";
  p.id = "message-text";
  p.textContent = text;
  dialog.setAttribute("aria-describedby", p.id);
  form.append(p);
  const buttons = document.createElement("div");
  buttons.className = "buttons";
  const choices = okCancel ? [["OK", "ok"], ["Cancel", "cancel"]] : !yesNo ? [["OK", "ok"]] : cancel ? [["Yes", "ok"], ["No", "no"], ["Cancel", "cancel"]] : [["Yes", "ok"], ["No", "cancel"]];
  for (const [label, value] of choices) {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = label;
    if (value === "ok") b.className = "default";
    b.addEventListener("click", () => dialog.close(value));
    buttons.append(b);
  }
  form.append(buttons);
  $("dialogs").append(dialog);
  const closed = closedPromise(dialog);
  dialog.showModal();
  buttons.firstChild.focus();
  const answer = await closed;
  return cancel ? answer : answer === "ok";
}
const message = (text) => messageBox(text, false);

// A whole number field checked as a dialog checks one (an onOk): string 2644 when it is not a
// number, else `low` or `high` with the limit for %d when it is out of 1 ... 2147483647. On a failure the message is
// shown, the field gets focus with its text selected, and the dialog stays open.
// The number is read by the dialogs' whole-number rule (an optional "-" and digits, no spaces, no "+"), after the
// first `width` characters when a width is given.
async function checkNumber(d, field, low, high, width = 0) {
  const MAX = 0x7fffffff;
  const n = numberField(d.text(field), width);
  const text = n === null ? "The number entered is not valid." : n < 1 ? low.replace("%d", "1") : n > MAX ? high.replace("%d", String(MAX)) : null;
  if (text === null) return true;
  await message(text);
  const el = d.get(field);
  el?.focus();
  el?.select?.();
  return false;
}
const yesNo = (text) => messageBox(text, true);
const okCancel = (text) => messageBox(text, false, false, true);
// A number field's number by the whole-number rule (bc_core::number_field), or null.
function numberField(text, width = 0) {
  if (core) return core.number_field(text, width) ?? null;
  const cut = width ? [...text].slice(0, width).join("") : text;
  return /^-?\d+$/.test(cut) ? Number(cut) : null;
}
// Yes, No or Cancel: true, false, or null for Cancel.
const yesNoCancel = async (text) => ({ ok: true, no: false })[await messageBox(text, true, true)] ?? null;

// The dialog a menu command opens: the one titled "Menu: Item".
function dialogFor(menuLabel, itemLabel) {
  const want = `${plain(menuLabel)}: ${plain(itemLabel)}`.replace(/\.\.\.$/, "").toLowerCase();
  for (const [rid, spec] of Object.entries(DIALOGS)) {
    if (!spec.panel && spec.title.replace(/\.\.\.$/, "").toLowerCase() === want) {
      return Number(rid);
    }
  }
  return null;
}

// ------------------------------------------------------------------ the embosser setup sheet

// The setup sheet (dialogs 10000 to 10004, spec 25 2d): `setup` shown, a changed copy answered on OK (null when
// cancelled). As spec 25 15 and 16 give, all four pages are made; a change of model fills the forms and
// the Device Settings and Advanced pages from the new model, makes the port list follow its ports, shows or hides the
// TranSend SE tab (only for a model with TranSend), and keeps the output, serial and TranSend SE values.
async function editSetup(setup, cat) {
  const lists = JSON.parse(core.setup_lists());
  const pages = [
    [10001, "General"],
    [10002, "Device Settings"],
    [10003, "TranSend SE"],
    [10004, "Advanced"],
  ];
  const title = `Embosser Setup - ${shownName(setup) || "Untitled Configuration"}`;
  const { dialog, form } = newDialog(title, "dlg10000");
  const tablist = document.createElement("div");
  tablist.setAttribute("role", "tablist");
  tablist.setAttribute("aria-label", title);
  form.append(tablist);
  const built = [];
  const tabs = [];
  pages.forEach(([rid, name], i) => {
    const tab = document.createElement("button");
    tab.type = "button";
    tab.setAttribute("role", "tab");
    tab.id = `sheet-tab-${rid}`;
    tab.textContent = name;
    tablist.append(tab);
    tabs.push(tab);
    const panel = document.createElement("div");
    panel.setAttribute("role", "tabpanel");
    panel.id = `sheet-page-${rid}`;
    panel.className = "page";
    panel.setAttribute("aria-labelledby", tab.id);
    tab.setAttribute("aria-controls", panel.id);
    form.append(panel);
    const { controls, keys } = buildControls(rid, panel);
    built.push({ panel, d: dialogHelpers(controls), keys });
    tab.addEventListener("click", () => showPage(i));
  });
  let current = 0;
  function showPage(i, focus = true) {
    current = i;
    tabs.forEach((t, k) => {
      t.setAttribute("aria-selected", String(k === i));
      t.tabIndex = k === i ? 0 : -1;
      built[k].panel.hidden = k !== i;
    });
    if (focus) tabs[i].focus();
  }
  // The tabs pattern: arrows, Home and End move between pages; Ctrl+PageDown and Ctrl+PageUp anywhere in the sheet.
  // A tab taken away (TranSend SE for a model without it) is passed over.
  const shownTabs = () => tabs.map((t, k) => k).filter((k) => !tabs[k].hidden);
  const step = (by) => {
    const shown = shownTabs();
    const at = Math.max(0, shown.indexOf(current));
    showPage(shown[(at + by + shown.length) % shown.length]);
  };
  tablist.addEventListener("keydown", (e) => {
    const shown = shownTabs();
    const moves = { ArrowRight: () => step(1), ArrowLeft: () => step(-1), Home: () => showPage(shown[0]), End: () => showPage(shown.at(-1)) };
    if (moves[e.key]) {
      e.preventDefault();
      moves[e.key]();
    }
  });
  dialog.addEventListener("keydown", (e) => {
    if (e.ctrlKey && (e.key === "PageDown" || e.key === "PageUp")) {
      e.preventDefault();
      step(e.key === "PageDown" ? 1 : -1);
    }
  });
  // Shows or takes away the TranSend SE tab; its page keeps its values either way (spec 25 15.1 and 15.4).
  const setTranSendTab = (on) => {
    tabs[2].hidden = !on;
    if (!on && current === 2) showPage(0, false);
  };
  const sheetControls = new Map();
  const general = built[0].d;
  const device = built[1].d;
  const advanced = built[3].d;
  const transend = built[2].d;
  const fill = (s, models) => fillSheet(general, device, advanced, s, cat, lists, models);
  // The sheet places Configuration location (8, 9) and Default Form (10, 11) at the same spot and shows
  // Configuration location (audit print-emboss E10); the setup's form follows its model.
  general.hide(10);
  general.hide(11);
  fill(setup, true);
  fillTranSend(transend, setup, lists);
  const hasModel = () => !(general.selected(5) ?? "(").startsWith("(");
  setTranSendTab(cat.flag(setup.model, "TranSend"));
  // The model prints only through its system printer driver: the Tiger driver out of Legacy Mode (spec A17.6).
  const printerOnly = () =>
    hasModel() && cat.setup_driver(general.selected(5), device.checked(24)) === "Emb_TigerEmbosserDriver";
  showGeneral(general, hasModel(), printerOnly());
  showOutput(general, advanced, hasModel(), printerOnly());
  // The Device Settings groups shown again for the model, version and escape codes now chosen.
  const refreshDevice = () => {
    const model = hasModel() ? general.selected(5) : "";
    showDevice(device, cat, model, chosenVersion(device, cat, model), device.checked(6));
  };
  device.get(5).addEventListener("change", refreshDevice);
  device.get(6).addEventListener("change", refreshDevice);
  // The output controls follow the output and the port chosen and Initialize Serial Port.
  for (const cid of [13, 14, 15, 17, 21]) {
    general.get(cid)?.addEventListener("change", () => showOutput(general, advanced, hasModel(), printerOnly()));
  }
  // Legacy Mode: out of it the output becomes Send to Printer again; Write to Port and Write to File are made usable
  // or not only when the sheet opens or the model changes (spec A17.6).
  device.get(24)?.addEventListener("change", () => showOutput(general, advanced, hasModel(), printerOnly()));
  // The setup the pages show for model `model` (whose forms the form list holds), over `base` for what no page shows.
  const readSheet = (base, model) => {
    const s = structuredClone(base);
    s.model = model;
    s.name = general.text(7).trim();
    s.all_users = general.selected(9) === LOCATIONS[1];
    const out = general.radio([13, 14, 15]);
    s.output =
      out === 0
        ? { Printer: general.selected(16) ?? "" }
        : out === 2
          ? { File: general.text(18).trim() }
          : // With no port chosen (the list made again for a new model) the port stored before is kept (spec 25 16.3).
            { Port: general.selected(17) ?? base.output.Port ?? cat.ports(model)[0] ?? "" };
    s.serial_init = general.checked(21);
    s.baud = Number(general.selected(23)) || s.baud;
    s.parity = general.chosenIn(25, lists.parities) ?? s.parity;
    s.data_bits = Number(general.selected(27)) || s.data_bits;
    s.stop_bits = Number(general.selected(29)) || s.stop_bits;
    s.handshaking = general.chosenIn(31, lists.handshaking) ?? s.handshaking;
    s.version = chosenVersion(device, cat, model);
    s.escape_codes = device.checked(6);
    s.et_speaks_manage = device.checked(7);
    s.et_speaks_on = device.checked(8);
    s.seek_top_of_form = device.checked(9);
    s.tiger_legacy = device.checked(24);
    s.interpoint_enabled = device.checked(34);
    s.encoding = device.selected(37) ?? s.encoding;
    s.high_punctuation = device.chosenIn(39, lists.cases) === 1;
    s.upper_case = device.chosenIn(41, lists.cases) === 1;
    s.reverse_order = device.checked(44);
    s.z_fold = device.checked(52);
    s.device_other = [16, 21, 23, 27, 30, 33, 43].map((c) => [c, device.text(c)]);
    s.custom_codes = advanced.checked(5);
    s.start_codes = advanced.text(7);
    s.end_codes = advanced.text(9);
    s.end_of_line = advanced.chosenIn(12, lists.lineEnds) ?? 0;
    s.end_of_page = advanced.chosenIn(14, lists.pageEnds) ?? 0;
    s.eol_on_full_line = advanced.checked(15);
    s.eop_on_full_page = advanced.checked(16);
    s.pad_lines = advanced.checked(17);
    s.prompt_new_sheet = advanced.checked(18);
    s.one_at_a_time = advanced.checked(20);
    s.text_delay = Math.max(0, advanced.number(22) ?? 0);
    s.control_delay = Math.max(0, advanced.number(24) ?? 0);
    // Kept across a change of model (spec 25 15.4), so read whether the tab shows or not.
    readTranSend(transend, s, lists);
    return s;
  };
  // The port list is made again only when the model's ports differ, and then nothing in it is chosen (spec 25 16.1).
  let lastPorts = JSON.stringify(cat.ports(setup.model));
  // The model the pages show (the form list holds its forms).
  let shownModel = setup.model;
  general.get(5).addEventListener("change", () => {
    const model = hasModel() ? general.selected(5) : "";
    if (model) {
      // The setup as the pages show it, changed to the new model (spec 25 18.3).
      const shown = readSheet(setup, shownModel);
      fill(JSON.parse(cat.with_model(JSON.stringify(shown), model, european())), false);
      shownModel = model;
    }
    const ports = cat.ports(model);
    if (JSON.stringify(ports) !== lastPorts) {
      general.setItems(17, ports, -1);
      lastPorts = JSON.stringify(ports);
    }
    showGeneral(general, !!model, printerOnly());
    showOutput(general, advanced, !!model, printerOnly());
    setTranSendTab(!!model && cat.flag(model, "TranSend"));
    refreshDevice();
  });
  let answer = null;
  addButtons(10000, dialog, form, sheetControls, async () => {
    const model = general.selected(5) ?? "";
    if (model.startsWith("(")) {
      await message("Please select an embosser model.");
      return false;
    }
    const s = readSheet(setup, model);
    if (setupNames().some((n) => n === shownName(s) && n !== shownName(setup))) {
      await message(`You already have a configuration named "${shownName(s)}".  Please give a different name.`);
      return false;
    }
    answer = s;
    return true;
  });
  wireKeys(dialog, () => built[current].keys);
  $("dialogs").append(dialog);
  const closed = closedPromise(dialog);
  showPage(0, false);
  dialog.showModal();
  focusFirst(built[0].panel);
  return (await closed) === "ok" ? answer : null;
}

// Page 10001 with or without a model, as the desktop program sets it when the sheet opens and on a change of model
// (spec 25 15.3, spec A17.6): with none, every control below the model choice is greyed; with one, the setup name,
// the configuration location and the form (6, 7, 10 to 13) are usable again, and Write to Port and Write to File
// (14, 15) unless the model prints only through its printer driver.
function showGeneral(general, hasModel, printerOnly = false) {
  if (!hasModel) {
    for (let cid = 6; cid <= 31; cid++) general.enable(cid, false);
    return;
  }
  for (const cid of [6, 7, 10, 11, 12, 13]) general.enable(cid, true);
  general.enable(14, !printerOnly);
  general.enable(15, !printerOnly);
}

// The output controls for the output now chosen (spec 25 16.3), as in the desktop program: only the chosen output's
// own field is usable; the Serial group and Initialize Serial Port only for a port beginning "com", the serial
// settings only while it is checked; Advanced's Send one character at a time (20) only for a port or a file.
function showOutput(general, advanced, hasModel, printerOnly = false) {
  // A model that prints only through its system printer driver (the driver table marks the Tiger driver alone, out
  // of Legacy Mode) is sent to a printer: the output becomes Send to Printer (spec A17.6, audit print-emboss E12).
  if (hasModel && printerOnly && general.radio([13, 14, 15]) !== 0) general.setRadio([13, 14, 15], 0);
  const kind = hasModel ? general.radio([13, 14, 15]) : null;
  general.enable(16, kind === 0);
  general.enable(17, kind === 1);
  general.enable(18, kind === 2);
  general.enable(19, kind === 2);
  const serial = kind === 1 && (general.selected(17) ?? "").startsWith("com");
  general.enable(20, serial);
  general.enable(21, serial);
  for (let cid = 22; cid <= 31; cid++) general.enable(cid, serial && general.checked(21));
  advanced.enable(20, kind !== null && kind !== 0);
}

// The TranSend SE page (sheet page 10003) showing `s` (spec 25 section 14). A browser cannot list the system's
// printers or fonts, so lists 6 and 8 hold the setup's own ink printer and font (web build); the printer choice, the
// font size and the two ink adjustments are as in the desktop program.
function fillTranSend(d, s, lists) {
  d.setItems(5, lists.transendFilters, Math.min(s.transend_filter, 2));
  d.setItems(6, s.ink_printer ? [s.ink_printer] : [], 0);
  d.setItems(8, s.interline_font ? [s.interline_font] : [], 0);
  d.setRadio([10, 11, 12], Math.min(s.transend_font_size, 2));
  d.setText(15, String(s.ink_adjust_h));
  d.setText(17, String(s.ink_adjust_v));
}

// The TranSend SE page's values read into `s`: a list with nothing chosen leaves the value as it was; an adjustment
// that is not a whole number is stored as 0, as in the desktop program.
function readTranSend(d, s, lists) {
  s.transend_filter = d.chosenIn(5, lists.transendFilters) ?? s.transend_filter;
  s.ink_printer = d.selected(6) ?? s.ink_printer;
  s.interline_font = d.selected(8) ?? s.interline_font;
  s.transend_font_size = d.radio([10, 11, 12]) ?? s.transend_font_size;
  s.ink_adjust_h = d.number(15) ?? 0;
  s.ink_adjust_v = d.number(17) ?? 0;
}

// Shows setup `s` on the sheet's pages. `models` is set when the sheet opens: the model list, the setup name and the
// output are filled only then, because a change of model leaves them as they are (spec 25 15.3);
// the forms, the serial settings and the other pages are filled each time.
function fillSheet(general, device, advanced, s, cat, lists, models) {
  if (models) {
    const names = ["(Please select a model)", ...cat.names()];
    general.setItems(5, names, Math.max(0, names.indexOf(s.model)));
    general.setText(7, s.name);
    // A browser cannot list the system's printers, so the printer list holds the setup's own printer when it has
    // one (web build); no other entry is added to this list (spec 25 14.6).
    const [kind, value] = Object.entries(s.output)[0];
    general.setItems(16, kind === "Printer" && value ? [value] : [], 0);
    general.setRadio([13, 14, 15], kind === "Printer" ? 0 : kind === "Port" ? 1 : 2);
    if (kind === "File") general.setText(18, value);
    // The model's ports (spec 25 16.1): with Write to Port the setup's port is chosen, else LPT1, either found without
    // regard to case; one not listed gives the first port.
    const ports = cat.ports(s.model);
    const want = (kind === "Port" ? value : "LPT1").toLowerCase();
    const at = ports.findIndex((p) => p.toLowerCase() === want);
    general.setItems(17, ports, at >= 0 ? at : ports.length ? 0 : -1);
  }
  // The serial settings: a change of model gives the new model's, apart from parity and Initialize Serial Port
  // (spec 25 18.3).
  general.setChecked(21, s.serial_init);
  general.setItems(23, lists.bauds, lists.bauds.indexOf(String(s.baud)));
  general.setItems(25, lists.parities, Math.min(s.parity, 3));
  general.setItems(27, ["7", "8"], s.data_bits === 8 ? 1 : 0);
  general.setItems(29, ["1", "2"], s.stop_bits === 2 ? 1 : 0);
  general.setItems(31, lists.handshaking, Math.min(s.handshaking, 2));
  const forms = JSON.parse(cat.forms(s.model));
  general.setItems(11, forms.map((f) => f.name), forms.findIndex((f) => f.id === s.form));
  general.enable(11, forms.length > 0);
  general.setItems(9, LOCATIONS, s.all_users ? 1 : 0);
  // Device settings: the version list holds the model's own versions.
  const versions = JSON.parse(cat.version_list(s.model));
  device.setItems(5, versions.items, versions.single || s.version === 0 ? 0 : Math.max(0, s.version - versions.first) + 1);
  device.setChecked(6, s.escape_codes);
  device.setChecked(7, !!s.et_speaks_manage);
  device.setChecked(8, !!s.et_speaks_on);
  device.setChecked(9, s.seek_top_of_form);
  device.setChecked(24, !!s.tiger_legacy);
  device.setChecked(34, s.interpoint_enabled);
  let encodings = cat.encodings(s.model);
  if (!encodings.length) encodings = ["North American English"];
  device.setItems(37, encodings, Math.max(0, encodings.indexOf(s.encoding)));
  device.setItems(39, lists.cases, s.high_punctuation ? 1 : 0);
  device.setItems(41, lists.cases, s.upper_case ? 1 : 0);
  device.setChecked(44, s.reverse_order);
  device.setChecked(52, s.z_fold);
  // The page's other fields as the setup has them; one it does not hold is empty (spec 25 18.3).
  for (const cid of [16, 21, 23, 27, 30, 33, 43]) device.setText(cid, s.device_other.find(([c]) => c === cid)?.[1] ?? "");
  showDevice(device, cat, s.model, s.version, s.escape_codes);
  advanced.setChecked(5, s.custom_codes);
  advanced.setText(7, s.start_codes);
  advanced.setText(9, s.end_codes);
  advanced.setItems(12, lists.lineEnds, Math.min(s.end_of_line, 2));
  advanced.setItems(14, lists.pageEnds, Math.min(s.end_of_page, 5));
  advanced.setChecked(15, s.eol_on_full_line);
  advanced.setChecked(16, s.eop_on_full_page);
  advanced.setChecked(17, s.pad_lines);
  advanced.setChecked(18, s.prompt_new_sheet);
  advanced.setChecked(20, s.one_at_a_time);
  advanced.setText(22, String(s.text_delay));
  advanced.setText(24, String(s.control_delay));
}

// The General page's Configuration location choices, in their order; the second is the setup's all_users.
const LOCATIONS = ["Current user", "All users on this computer"];

// The version number chosen in the Device Settings version list (5), filled for `model`.
function chosenVersion(device, cat, model) {
  const { items, first, single } = JSON.parse(cat.version_list(model));
  const place = Math.max(0, items.indexOf(device.selected(5)));
  return single ? first : place === 0 ? 0 : place + first - 1;
}

// The Device Settings page (spec 25, resource 10002; the desktop program's show_device):
// one group of controls per embosser driver at the same places, only the model's driver's group shown; the margin
// rows, the interpoint box and the encoding rows by the driver and by whether escape codes are in use; the Braillo
// rows by the model's SubModel number (15 to 18 for 1, 4, 5, 6 and 10; 19 to 23 for 9 and 11).
function showDevice(device, cat, model, version, escapes) {
  const driver = model ? cat.driver(model) : "";
  const index = driver === "Emb_IndexEmbosserDriver";
  const enabling = driver === "Emb_EnablingEmbosserDriver";
  const braillo = driver === "Emb_BrailloEmbosserDriver";
  const tiger = driver === "Emb_TigerEmbosserDriver";
  const elekul = driver === "Emb_ElekulEmbosserDriver";
  const flag = (k) => !!model && cat.flag(model, k);
  const inUse = flag("UsingEscapeCodes") || escapes;
  let margins, interpoint, encoding;
  if (tiger || elekul) [margins, interpoint, encoding] = [false, false, false];
  else if (index) [margins, interpoint, encoding] = [version < 4, true, true];
  else if (enabling) [margins, interpoint, encoding] = [!escapes, !escapes, true];
  else if (braillo) [margins, interpoint, encoding] = [false, false, true];
  else [margins, interpoint, encoding] = [!inUse, !inUse, true];
  const rows = margins && !!model;
  const sub = (set) => !!model && set.includes(cat.number(model, "SubModel"));
  const groups = [
    [4, 5, index],
    [6, 6, enabling],
    [7, 11, enabling && inUse],
    [12, 14, braillo],
    [15, 18, braillo && sub([1, 4, 5, 6, 10])],
    [19, 23, braillo && sub([9, 11])],
    [24, 24, tiger],
    [25, 25, margins],
    [26, 28, rows && !flag("NoLeftMargin")],
    [29, 31, rows && !flag("NoRightMargin")],
    [32, 33, rows && !flag("NoTopMargin")],
    [34, 34, interpoint],
    [35, 41, encoding],
    [42, 51, elekul],
    [52, 52, index],
  ];
  for (const [first, last, on] of groups) for (let cid = first; cid <= last; cid++) device.show(cid, on);
  for (let cid = 6; cid <= 31; cid++) device.enable(cid, !!model);
  device.enable(7, inUse && flag("ETSpeaks"));
  device.enable(8, inUse && flag("ETSpeaks"));
  device.enable(9, inUse && flag("AllowSeekTOF"));
  device.enable(34, flag("CanDoInterpoint"));
  device.enable(52, flag("IndexZfold"));
}

// ------------------------------------------------------------------ Embosser Setup (dialog 1032)

const NO_EMBOSSER = "(No Target Embosser)";

function setupForms(cat, name) {
  const setup = name && loadSetup(name);
  return setup ? JSON.parse(cat.forms(setup.model)) : [];
}

function fillSetupList(d, selected) {
  const names = [NO_EMBOSSER, ...setupNames()];
  d.setItems(7, names, Math.max(0, names.indexOf(selected)));
}

// The form box sorts its items, so they go in sorted and the choice is found by text.
function fillSetupForms(d, cat, form) {
  const name = d.selected(7);
  const forms = setupForms(cat, name === NO_EMBOSSER ? "" : name);
  const names = forms.map((f) => f.name).sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
  const chosen = forms.find((f) => f.id === form)?.name;
  d.setItems(13, names, names.length ? Math.max(0, names.indexOf(chosen)) : -1);
  d.enable(13, names.length > 0);
}

// Embosser Setup's page block ({ layout, cells, lines }) as its fields hold it, `form` its paper.
function readBlock(d, form, base) {
  const field = (cid, current) => {
    const n = d.number(cid);
    return n !== null && n >= 0 ? n : current;
  };
  const layout = {
    ...base.layout,
    form,
    top_margin: field(19, base.layout.top_margin),
    binding: field(21, base.layout.binding),
    interpoint: d.checked(22),
    banner: d.checked(23),
    interline: d.checked(24) ? 1 : 0,
  };
  return { layout, cells: field(15, base.cells), lines: field(17, base.lines) };
}

function showBlock(d, b) {
  d.setText(15, String(b.cells));
  d.setText(17, String(b.lines));
  d.setText(19, String(b.layout.top_margin));
  d.setText(21, String(b.layout.binding));
  d.setChecked(22, b.layout.interpoint);
  d.setChecked(23, b.layout.banner);
  d.setChecked(24, b.layout.interline !== 0);
}

// A page block corrected to the setup chosen in the list (none for no target).
function fitToChoice(d, cat, b) {
  const name = d.selected(7);
  const s = name && name !== NO_EMBOSSER ? loadSetup(name) : null;
  return JSON.parse(cat.fit_block(s ? JSON.stringify(s) : "", JSON.stringify(b.layout), b.cells, b.lines));
}

// Document / Global: Embosser Setup (dialog 1032, spec 25 section 2b). Document: the document's setup and its
// embosser page layout. Global: the default setup and the layout new documents take; New, Modify Embosser and
// Remove (Global only) open the setup sheet and change the kept setups.
async function embosserSetup(global) {
  const cat = await embossers();
  if (!cat) {
    await message("The embosser models were not found.");
    return;
  }
  const [defLayout, defCells, defLines] = defaultLayout();
  const [setupName, layout, cells, lines] = global
    ? [defaultSetup(), defLayout, defCells, defLines]
    : // A braille document shows its own cells and lines; a print document the default layout's.
      [doc.embosser, doc.layout, doc.kind === "braille" ? doc.cells : defCells, doc.kind === "braille" ? doc.lines : defLines];
  let chosen = null;
  // The block as the dialog opened, corrected to its setup; the reformatting question compares with it on OK.
  let opened = null;
  const shownFormId = (d) => {
    const name = d.selected(7) === NO_EMBOSSER ? "" : d.selected(7) ?? "";
    // With "(No Target Embosser)" the paper stays the block's own (spec 25 12.1).
    if (!name) return layout.form;
    return setupForms(cat, name).find((f) => f.name === d.selected(13))?.id ?? "";
  };
  const { result } = await showDialog(1032, {
    title: global ? "Global: Embosser Setup" : undefined,
    setup(d) {
      fillSetupList(d, setupName);
      fillSetupForms(d, cat, layout.form);
      opened = fitToChoice(d, cat, { layout: { ...layout }, cells, lines });
      showBlock(d, opened);
      // A new choice: the block read from the fields, the form box filled for the new setup, the block corrected to
      // it silently and shown.
      d.get(7).addEventListener("change", () => {
        const typed = readBlock(d, shownFormId(d), opened);
        fillSetupForms(d, cat, loadSetup(d.selected(7))?.form ?? "");
        showBlock(d, fitToChoice(d, cat, { ...typed, layout: { ...typed.layout, form: shownFormId(d) } }));
      });
      if (!global) {
        [8, 9, 10].forEach((b) => d.hide(b));
        return;
      }
      const sheet = async (modify) => {
        const old = d.selected(7) === NO_EMBOSSER ? null : d.selected(7);
        if (modify && !old) return;
        const s = (modify && old && loadSetup(old)) || JSON.parse(cat.new_setup("", european()));
        const changed = await editSetup(s, cat);
        if (changed) {
          saveSetup(changed, modify ? old : null);
          fillSetupList(d, shownName(changed));
          fillSetupForms(d, cat, changed.form);
        }
        d.get(modify ? 9 : 8).focus();
      };
      d.get(8).addEventListener("click", () => sheet(false));
      d.get(9).addEventListener("click", () => sheet(true));
      d.get(10).addEventListener("click", async () => {
        const name = d.selected(7);
        if (!name || name === NO_EMBOSSER) return;
        const text =
          setupNames().length === 1
            ? `Are you sure you want to remove embosser configuration "${name}"?  This is currently your only configuration; you will need to create another one before you can emboss.`
            : `Are you sure that you want to remove embosser configuration "${name}"?`;
        if (await yesNo(text)) {
          removeSetup(name);
          fillSetupList(d, NO_EMBOSSER);
          fillSetupForms(d, cat, "");
        }
        d.get(10).focus();
      });
    },
    async onOk(d) {
      const name = d.selected(7) === NO_EMBOSSER ? "" : d.selected(7) ?? "";
      // OK corrects the block to the setup: values it had to change are shown in message 3771 with the dialog kept
      // open; otherwise in Document mode cells or lines unlike the document's ask question 3773 first.
      const typed = readBlock(d, shownFormId(d), opened);
      const fitted = fitToChoice(d, cat, typed);
      const table = core.embosser_change_table(JSON.stringify(typed), JSON.stringify(fitted), true);
      if (table) {
        await message(`BrailleCraft has updated desired document settings to fit the selected page size.${table}`);
        showBlock(d, fitted);
        return false;
      }
      if (!global) {
        const change = core.embosser_change_table(JSON.stringify(opened), JSON.stringify(fitted), false);
        if (change && !(await okCancel(`Your document will be reformatted.${change}`))) return false;
      }
      chosen = { name, layout: fitted.layout, cells: fitted.cells, lines: fitted.lines };
      return true;
    },
  });
  if (result !== "ok" || !chosen) {
    return;
  }
  if (global) {
    setDefaultSetup(chosen.name);
    saveDefaultLayout(chosen.layout, chosen.cells, chosen.lines);
    return;
  }
  doc.embosser = chosen.name;
  doc.layout = chosen.layout;
  if (doc.kind === "braille") {
    doc.cells = chosen.cells;
    doc.lines = chosen.lines;
  } else {
    doc.embPage = [chosen.cells, chosen.lines];
  }
}

// At start-up, Global, Embosser Setup when no embosser is set up (spec 25 section 11).
async function embosserSetupAtStart() {
  if (setupNames().length === 0) {
    await embosserSetup(true);
    editor.focus();
  }
}

// ------------------------------------------------------------------ page ranges (File Print and File Emboss)

const PRINT_RANGE = [9, 10, 11, 12, 14];
const EMBOSS_RANGE = [9, 10, 11, 12, 13];

// All chosen, and the two range fields enabled only with the page numbers and page sequence numbers radios.
function wirePageRange(d, radios, fields) {
  d.setRadio(radios, 0);
  d.enable(fields[0], false);
  d.enable(fields[1], false);
  // The two fields sit beside their radio buttons and have no label of their own: each is named
  // by its radio button.
  d.get(fields[0]).setAttribute("aria-label", d.get(radios[3]).closest("label").textContent);
  d.get(fields[1]).setAttribute("aria-label", d.get(radios[4]).closest("label").textContent);
  radios.forEach((r, i) =>
    d.get(r).addEventListener("change", () => {
      d.enable(fields[0], i === 3);
      d.enable(fields[1], i === 4);
    }),
  );
}

async function copiesField(d, cid) {
  const n = d.number(cid);
  if (n !== null && n >= 1) {
    return n;
  }
  await message("You must print or emboss at least 1 copy, or press Cancel to avoid printing any.");
  return null;
}

// The pages the page range group asks for ({ pages: indexes, -1 for a blank page; text: the selection's own text
// when Selection lays it out as a document of its own, else null }), or null when a message was shown and the
// dialog stays open; the caret and the selection are the editor's as the dialog opened.
async function chosenPages(d, radios, fields, braille, caret, from, to) {
  const count = core.page_count(editorText(), doc.cells, doc.lines, braille);
  const pageOf = (pos) => core.page_of(editorText(), doc.cells, doc.lines, braille, Math.max(0, pos));
  let range;
  switch (d.radio(radios) ?? 0) {
    case 0:
      range = { kind: "pages", pages: [...Array(count).keys()] };
      break;
    case 1:
      range = { kind: "pages", pages: [pageOf(caret) - 1] };
      break;
    case 2: {
      // The selection laid out as a document of its own, all of its pages; no page: its
      // message -1, which has no text.
      const part = core.editor_part(from, to);
      const n = core.page_count(part, doc.cells, doc.lines, braille);
      if (!n) {
        await message("Error message -1");
        return null;
      }
      return { pages: [...Array(n).keys()], text: part };
    }
    case 3:
      // Page numbers are matched against the pages' own numbers; sequence numbers count the pages in order.
      range = JSON.parse(core.parse_page_numbers(d.text(fields[0]), editorText(), doc.cells, doc.lines, braille));
      break;
    default:
      range = JSON.parse(core.parse_range(d.text(fields[1]), count));
  }
  if (range.kind === "malformed") {
    await message(
      "The page range specified is not written properly.  Type your page range as in the following example, which requests pages 1 through 7, a blank page, and then page 9 only: 1-7,b,9",
    );
    return null;
  }
  if (range.kind === "empty") {
    await message("You must enter at least one page number.");
    return null;
  }
  if (range.kind === "none") {
    await message("That page range does not match any pages found in your document.");
    return null;
  }
  if (
    range.missing &&
    !(await yesNo("The page range you entered must be adjusted because not all pages you specified were found in this document. Do you wish to proceed anyway?"))
  ) {
    return null;
  }
  if (range.repeated && !(await yesNo("The page range you have specified implies that one or more pages will be printed more than once.  Do you wish to proceed?"))) {
    return null;
  }
  return { pages: range.pages, text: null };
}

// ------------------------------------------------------------------ File Print (dialog 1040)

// Whether a font named "RNIB Moon" is installed: text measured in it differs from the generic fallbacks. Looked up
// once.
function moonInstalled() {
  if (moonInstalled.found !== undefined) return moonInstalled.found;
  const ctx = document.createElement("canvas").getContext("2d");
  if (!ctx) return (moonInstalled.found = false);
  const width = (family) => {
    ctx.font = `24px ${family}`;
    return ctx.measureText("abcdefghijklmnopqrstuvwxyz0123456789").width;
  };
  moonInstalled.found = ["monospace", "serif"].every((g) => width(`"RNIB Moon", ${g}`) !== width(g));
  return moonInstalled.found;
}

// The CSS family a page prints in (spec A16.1): the view font's printing face, the Print font's face, "Braille",
// "SimBraille", or "RNIB Moon" for a braille view font when the document's print-to-braille table name starts with
// "ENMOCP" and the font is installed. The braille faces fall back to the program's own braille font.
function printFaceOf(font, table) {
  if (font === "print") return faceOf("print")[0];
  const own = `"${brailleFamily(font === "braille" ? "Braille" : "SimBraille")}", monospace`;
  if (moonInstalled() && table.slice(0, 6).toUpperCase() === "ENMOCP") return `"RNIB Moon", ${own}`;
  // BrailleCraft's own braille or SimBraille font for the display encoding (the fonts the program ships).
  return own;
}

// The height of a line of `family` at `points` (its ascent and descent), in CSS pixels; null when it cannot be
// measured.
function lineHeightOf(family, points) {
  const ctx = (lineHeightOf.canvas ??= document.createElement("canvas")).getContext("2d");
  if (!ctx) return null;
  ctx.font = `${points}pt ${family}`;
  const m = ctx.measureText("a");
  const h = (m.fontBoundingBoxAscent ?? 0) + (m.fontBoundingBoxDescent ?? 0);
  return h > 0 ? h : null;
}

// The document's chosen pages printed through the browser's print dialog, `copies` times, a page per sheet; a
// braille document with each line's print above it, in 9-point Times New Roman, when interline print is chosen, and
// with its line numbers in a column on each side, in the editing window's font, when Global, View Preferences shows
// them (spec A16.5).
async function print() {
  if (!core) {
    window.print();
    return;
  }
  const braille = doc.kind === "braille";
  const [caret, from, to] = [editor.selectionStart, editor.selectionStart, editor.selectionEnd];
  let job = null;
  const { result } = await showDialog(1040, {
    setup(d) {
      d.setText(7, "1");
      wirePageRange(d, PRINT_RANGE, [15, 16]);
      // On a braille document the page-number choice reads "Braille page number(s)": the dialog copies control
      // 13's text into control 12 (spec 24, 2a); control 13 itself is not used.
      if (braille) d.get(12).closest("label").lastChild.textContent = d.get(13).closest("label").textContent;
      d.hide(13);
      // No page-number range when two pages carry the same page number; Interline Print starts from the
      // document's embosser layout and is the job's choice only (spec 24).
      d.enable(12, !core.page_numbers_repeat(editorText(), doc.cells, doc.lines, braille));
      // Interline Print starts checked when the layout has interline or DoubleSizePrint (spec A16.5).
      d.setChecked(17, braille && (doc.layout.interline !== 0 || (doc.doubleSize ?? 0) !== 0));
      d.enable(17, braille);
    },
    async onOk(d) {
      const copies = await copiesField(d, 7);
      if (copies === null) return false;
      const chosen = await chosenPages(d, PRINT_RANGE, [15, 16], braille, caret, from, to);
      if (chosen === null) return false;
      job = { copies, pages: chosen.pages, text: chosen.text, interline: braille && d.checked(17) };
      return true;
    },
  });
  if (result !== "ok") return;
  const pages = JSON.parse(core.page_lines(job.text ?? editorText(), doc.cells, doc.lines, braille, Int32Array.from(job.pages)));
  const back = job.interline ? await documentEngine(true) : null;
  const area = $("print-area");
  area.replaceChildren();
  // The pages print in the document's view font in its printing face, at the Global, Fonts printed size unless that
  // is set to be found.
  const family = printFaceOf(doc.view.font, (await documentFile()).tables[0] ?? "");
  const size = doc.view.font === "print" ? settings.fonts.print_output_size : settings.fonts.braille_output_size;
  area.style.fontFamily = family;
  area.style.fontSize = settings.fonts.print_auto ? "" : `${size}pt`;
  // The Braille and SimBraille printing faces step nine tenths of a line from one line to the next.
  const tight = doc.view.font !== "print" && !family.startsWith('"RNIB Moon"');
  const height = tight ? lineHeightOf(family, settings.fonts.print_auto ? 12 : size) : null;
  // Line numbers: a braille document with the setting on, in coded view too, in the editing window's font.
  const numbers = braille && settings.lineNumbers;
  // The number column is as wide as "25" in that font; the text starts half that width after it, and the right
  // column half that width after the text block (spec 24, 3d).
  const [windowFamily, windowSize] = faceOf(doc.view.font);
  const ctx = document.createElement("canvas").getContext("2d");
  if (ctx) ctx.font = `${windowSize}pt ${windowFamily}`;
  const numberWidth = ctx ? ctx.measureText("25").width : 0;
  const number = (n, side) => {
    const span = document.createElement("span");
    span.className = "printed-number";
    span.style.fontFamily = windowFamily;
    span.style.fontSize = `${windowSize}pt`;
    span.style.width = `${numberWidth}px`;
    span.style[side === "left" ? "marginRight" : "marginLeft"] = `${numberWidth / 2}px`;
    span.textContent = String(n);
    return span;
  };
  for (let copy = 0; copy < job.copies; copy++) {
    for (const lines of pages) {
      const page = document.createElement("section");
      page.className = "printed-page";
      for (const [i, line] of (lines ?? []).entries()) {
        // The interline print goes above its braille line.
        if (back?.e && line.trim()) {
          const ink = document.createElement("div");
          ink.className = "printed-ink";
          ink.style.fontFamily = '"Times New Roman", serif';
          ink.style.fontSize = "9pt";
          // From the line's first cell.
          if (numbers) ink.style.marginLeft = `${numberWidth * 1.5}px`;
          try {
            // A laid-out line holds no codes: its brackets are braille cells.
            ink.textContent = core.running_text(await translateText(back.e, toNA(core.plain_text(line.trimEnd())), back.file, true, true));
          } catch {
            ink.textContent = "";
          }
          page.append(ink);
        }
        const row = document.createElement("div");
        row.className = "printed-line";
        if (height) row.style.lineHeight = `${height * 0.9}px`;
        const text = document.createElement("span");
        text.className = "printed-text";
        text.style.width = `${doc.cells}ch`;
        text.textContent = line || " ";
        if (numbers) row.append(number(i + 1, "left"), text, number(i + 1, "right"));
        else row.append(text);
        page.append(row);
      }
      area.append(page);
    }
  }
  // The page size and margins of the document's Printer Setup, else the program's.
  const page = document.createElement("style");
  page.textContent = core.printer_page_css(printerSetupOf(false));
  document.head.append(page);
  window.print();
  page.remove();
  area.replaceChildren();
}

// ------------------------------------------------------------------ Printer Setup (dialog 1071)

// The kept setup (its settings text): the document's own when it has one, else the program's (Global, Printer
// Setup), as in the desktop program.
const printerSetupOf = (global) => (!global && doc.printer) || store.get("printer.setup") || "";

// Document, Printer Setup and Global, Printer Setup (dialog 1071, spec 24 2b): the paper, orientation and margins,
// and the formatted page size. Every document keeps its own page size here: a print document is laid out again on
// it; a braille document, laid out on its embosser page, only keeps it, and its file keeps it (spec A16.2).
// The browser chooses the printer in its own print dialog, so Printer... says so (web build).
async function printerSetup(global) {
  let setup = printerSetupOf(global);
  let size = [0, 0];
  const view = JSON.parse(core.printer_setup_view(setup));
  const own = doc.kind === "print" ? [doc.lines, doc.cells] : doc.printPage ?? globalPrintPage();
  const [lines, chars] = global ? [view.lines, view.chars] : own;
  const margins = [1155, 1156, 1157, 1158];
  const { result, d } = await showDialog(1071, {
    title: global ? "Global: Printer Setup..." : "Document: Printer Setup...",
    setup(d) {
      d.setItems(1137, view.papers, view.papers.indexOf(view.paper));
      d.setItems(1138, ["Automatically Select"], 0);
      d.setRadio([1056, 1057], view.landscape ? 1 : 0);
      margins.forEach((c, i) => d.setText(c, view.margins[i]));
      d.setText(2003, String(lines));
      d.setText(2005, String(chars));
      d.get(1026)?.addEventListener("click", () => message("In the browser, the printer and its own settings are chosen in the browser's print dialog when you print."));
      d.get(1038)?.addEventListener("click", () => message(PLANNED_HELP));
    },
    async onOk(d) {
      try {
        setup = core.printer_setup_read(setup, d.selected(1137) ?? "", d.radio([1056, 1057]) === 1, margins.map((c) => d.text(c)));
      } catch (e) {
        await message(e.message);
        return false;
      }
      // The page size: a corrected size is shown and asked about (question 1000); Cancel
      // keeps the dialog open on the corrected field.
      const [l, c, corrected] = JSON.parse(core.printer_page_check(d.text(2003), d.text(2005)));
      if (corrected) {
        d.setText(2003, String(l));
        d.setText(2005, String(c));
        if (!(await okCancel(core.printer_page_question(l, c)))) {
          const field = d.get(corrected === 1 ? 2003 : 2005);
          field.focus();
          field.select();
          return false;
        }
      }
      size = [l, c];
      return true;
    },
  });
  if (result !== "ok") return;
  const [newLines, newChars] = size;
  if (global) {
    store.set("printer.setup", core.printer_setup_page(setup, newLines, newChars));
  } else {
    // A print document is laid out on the new size; a braille document keeps it.
    if (doc.kind === "print") {
      doc.lines = newLines;
      doc.cells = newChars;
    } else {
      doc.printPage = [newLines, newChars];
    }
    doc.printer = setup;
  }
  applyView();
}

// ------------------------------------------------------------------ File Emboss (dialog 1039)

// Sends a job through Web Serial, the browser's serial port access, with the setup's serial settings (spec A17.10).
// Hardware handshaking is the browser's RTS/CTS flow control (Web Serial has none by DTR and DSR). Software
// handshaking (XON/XOFF) is done here: the port's answers are read while the job is written in pieces of 64 bytes,
// and writing stops after XOFF (0x13) until XON (0x11) comes; RTS and DTR are on. With no handshaking RTS and DTR
// are off. The job goes a page at a time (`job`: the page numbers and the pieces, start codes first and end codes
// last), the progress window showing each page; a job cancelled there stops at the next page with "Cancelling...".
async function sendSerial(setup, job, progress) {
  const port = await navigator.serial.requestPort();
  const options = { baudRate: setup.baud, dataBits: setup.data_bits, stopBits: setup.stop_bits };
  const software = setup.serial_init && setup.handshaking === 1;
  if (setup.serial_init) {
    options.parity = ["none", "even", "odd", "none"][setup.parity] ?? "none";
    options.flowControl = setup.handshaking === 0 ? "hardware" : "none";
  }
  await port.open(options);
  let reader = null;
  let reading = null;
  try {
    if (software) await port.setSignals({ dataTerminalReady: true, requestToSend: true });
    if (setup.serial_init && setup.handshaking === 2) {
      await port.setSignals({ dataTerminalReady: false, requestToSend: false });
    }
    // XON/XOFF: `paused` is set by XOFF and cleared by XON; `resume` wakes the writer waiting on it.
    let paused = false;
    let resume = null;
    if (software && port.readable) {
      reader = port.readable.getReader();
      reading = (async () => {
        for (;;) {
          const { value, done } = await reader.read();
          if (done) return;
          for (const b of value ?? []) {
            if (b === 0x13) paused = true;
            else if (b === 0x11) {
              paused = false;
              resume?.();
            }
          }
        }
      })().catch(() => {});
    }
    const writer = port.writable.getWriter();
    const write = async (bytes) => {
      if (!software) return writer.write(bytes);
      for (let at = 0; at < bytes.length; at += 64) {
        while (paused) await new Promise((r) => (resume = r));
        await writer.write(bytes.subarray(at, at + 64));
      }
    };
    try {
      const pieces = job.pieces.map((p) => Uint8Array.from(p));
      const last = pieces.length - 1;
      await write(pieces[0]);
      for (let k = 1; k < last; k++) {
        if (progress.cancelled) {
          progress.status(28004, 0);
          break;
        }
        progress.status(28002, job.pages[k - 1] ?? k);
        await write(pieces[k]);
      }
      if (last > 0) await write(pieces[last]);
    } finally {
      writer.releaseLock();
    }
  } finally {
    if (reader) {
      await reader.cancel().catch(() => {});
      await reading;
      reader.releaseLock();
    }
    await port.close();
  }
}

// The embossing progress window (dialog 28000), shown without blocking the page while a job is sent: the job's name
// (11) and its status (12, announced to screen readers as it changes). Cancel, Enter, Space and Escape cancel.
function embossProgress(name) {
  const { dialog, form } = newDialog(DIALOGS["28000"].title, "dlg28000");
  const { controls } = buildControls("28000", form);
  const d = dialogHelpers(controls);
  let cancelled = false;
  let done = false;
  addButtons("28000", dialog, form, controls, null);
  const status = d.get(12);
  status?.setAttribute("role", "status");
  status?.setAttribute("aria-live", "polite");
  d.setText(11, name);
  d.setText(12, "");
  const cancel = () => {
    if (cancelled) return;
    cancelled = true;
    d.setText(12, core.progress_text(28004, 0));
  };
  d.get(2)?.addEventListener("click", cancel, { capture: true });
  dialog.addEventListener("keydown", (e) => {
    if (["Escape", "Enter", " "].includes(e.key)) {
      e.preventDefault();
      cancel();
    }
  });
  // The Cancel button would close the window; it stays until the job ends.
  dialog.addEventListener("close", () => {
    if (!done) {
      cancel();
      dialog.show();
    }
  });
  $("dialogs").append(dialog);
  dialog.show();
  placeThird(dialog);
  d.get(2)?.focus();
  return {
    get cancelled() {
      return cancelled;
    },
    status(msgid, page) {
      if (msgid === 28004) cancel();
      else d.setText(12, core.progress_text(msgid, page));
    },
    close() {
      done = true;
      dialog.close();
      dialog.remove();
    },
  };
}

// File, Emboss (dialog 1039, spec 25 2a): the checks before a job in the order of spec 25 9.1, then the
// document's laid-out pages in the chosen range, `copies` times, through its setup's byte pipeline. A serial port
// goes through Web Serial where the browser has it; the browser reaches no printer queue, parallel port or USB
// port, so those jobs, and Write to File, are saved as a file.
async function emboss() {
  if (!core) {
    await message("Embossing is not available in this copy of the web build.");
    return;
  }
  // A document opened with a setup that is not kept has none (spec 25 10.1). INFERENCE: a document whose setup is
  // removed while it is open is treated as having no setup chosen (spec A17.7).
  const setup = doc.embosser ? loadSetup(doc.embosser) : null;
  if (!setup) {
    await message(core.no_setup_message());
    return;
  }
  const cat = await embossers();
  if (!cat) {
    await message("The embosser models were not found.");
    return;
  }
  const [setupJson, layoutJson] = [JSON.stringify(setup), JSON.stringify(doc.layout)];
  const verdict = cat.check(setupJson, layoutJson, doc.cells, doc.lines);
  if (verdict.startsWith("stop:")) {
    await message(verdict.slice(5));
    return;
  }
  if (verdict.startsWith("ask:") && !(await yesNo(verdict.slice(4)))) {
    return;
  }
  const braille = doc.kind === "braille";
  const [caret, from, to] = [editor.selectionStart, editor.selectionStart, editor.selectionEnd];
  let job = null;
  const { result } = await showDialog(1039, {
    setup(d) {
      d.setText(7, "1");
      wirePageRange(d, EMBOSS_RANGE, [14, 15]);
      d.enable(12, !core.page_numbers_repeat(editorText(), doc.cells, doc.lines, braille));
      d.setText(23, shownName(setup));
      d.setText(25, core.formName(doc.layout.form));
    },
    async onOk(d) {
      const copies = await copiesField(d, 7);
      if (copies === null) return false;
      const chosen = await chosenPages(d, EMBOSS_RANGE, [14, 15], braille, caret, from, to);
      if (chosen === null) return false;
      job = { copies, pages: chosen.pages, text: chosen.text };
      return true;
    },
  });
  if (result !== "ok") return;
  // A ManualInterpoint embosser with interpoint on embosses the fronts, then after prompt 3761 the backs (spec 25 19):
  // two braille passes, each a whole job (side 1 the fronts, 2 the backs; 0 every side).
  const sides = cat.manual_interpoint(setupJson, layoutJson) ? [1, 2] : [0];
  const codes = await encodingCodes(setup.encoding);
  const [kind, target] = Object.entries(setup.output)[0];
  for (const [n, side] of sides.entries()) {
    if (n > 0 && !(await okCancel("Please re-insert the stack of embossed sheets to emboss the second side."))) return;
    const text = job.text === null ? storedText() : braille ? toNA(job.text) : job.text;
    const args = [setupJson, layoutJson, text, braille, doc.cells, doc.lines, job.copies, Int32Array.from(job.pages), codes, side];
    if (kind === "Port" && /^com/i.test(target) && "serial" in navigator) {
      // Sent a page at a time with the progress window: "Starting Document", then "Embossing Page N".
      const progress = embossProgress(doc.name);
      progress.status(28000, 0);
      try {
        await sendSerial(setup, JSON.parse(cat.job_pieces(...args)), progress);
        if (progress.cancelled) return;
        continue;
      } catch (err) {
        if (err?.name === "NotFoundError") return; // no port was chosen
        progress.close();
        await message(String(err.message ?? err));
        return;
      } finally {
        progress.close();
      }
    }
    const bytes = cat.job(...args);
    const named = kind === "File" && target.trim() ? target.trim().split(/[\\/]/).pop() : `${stem()}.brl`;
    download(named, bytes);
    if (kind !== "File") {
      statusLine.textContent = `This browser cannot send to ${kind === "Printer" ? "a printer" : target}; the job was saved as ${named}.`;
    }
  }
}

// ------------------------------------------------------------------ importing files

// The kinds read through an import chain (kinds as the core names them).
const CHAIN_KINDS = ["CodedText", "FormattedText", "CodedBraille", "FormattedBraille", "Orbit"];

// What the desktop program says for a kind it cannot import: the core's refusal (an older Excel workbook, RTF, any
// other unsupported file), else BrailleCraft's own words for a kind with no importer yet.
function importRefusal({ kind, filter }, bytes) {
  return core.import_refusal(kind, bytes) || `BrailleCraft cannot import ${filter} files yet.`;
}

// The templates as File, New lists them (templates/index.tsv), in alphabetical order.
let templateList = null;
async function templateNames() {
  if (!templateList) {
    try {
      const index = await (await fetch("templates/index.tsv")).text();
      templateList = index
        .split("\n")
        .filter((l) => l.trim() && !l.startsWith("#"))
        .map((l) => l.split("\t"))
        .filter((c) => c.length >= 3)
        .map((c) => ({ code: c[0].trim(), file: c[1], name: c[2].trim() }))
        .sort((a, b) => a.name.localeCompare(b.name));
    } catch {
      templateList = [];
    }
  }
  return templateList;
}

// The template list of a dialog and its buttons (make default, hide, show always, region, show hidden), as File,
// New has them in the desktop program; `ids` names the controls. Kept between runs as the desktop program keeps
// them: the default template (DefaultTemplate), the hidden ones (HiddenTemplates, names separated by |) and Show
// Hidden Templates (ShowAllTemplates, File New only). Returns the state; `current` is the template under the cursor.
const NEW_TEMPLATES = { list: 10, default: 11, hide: 12, show: 13, region: 14, showHidden: 15, kept: true };
const IMPORT_TEMPLATES = { list: 7, default: 8, hide: 9, show: 10, region: 11, showHidden: 12, kept: false };
const REGION_NAMES = ["Western Hemisphere", "Europe", "Asia", "Africa", "Pacific", "All Regions/All Languages"];
const DEFAULT_TEMPLATE = "English (UEB) - basic";
function templateChooser(d, ids, all) {
  const allRegions = core.LanguageList.region_bit(5);
  const s = {
    hidden: (store.get("HiddenTemplates") ?? "").split("|").filter(Boolean),
    default: store.get("DefaultTemplate") ?? DEFAULT_TEMPLATE,
    showHidden: ids.kept && store.get("ShowAllTemplates") === "yes",
    region: allRegions,
    names: [],
    current: null,
  };
  // A template whose table is a listed language's belongs to its regions; any other is listed in every region.
  const regionsOf = (t) => (languageList ? languageList.template_regions(t.code) : allRegions);
  const listed = () =>
    all
      .filter((t) => s.showHidden || !s.hidden.includes(t.name))
      .filter((t) => (regionsOf(t) & s.region) !== 0)
      .map((t) => t.name);
  // Hide Template and Show Template Always share a place: the second shows for a hidden template.
  const updateHide = () => {
    const hidden = s.current !== null && s.hidden.includes(s.current);
    d.show(ids.hide, !hidden);
    d.show(ids.show, hidden);
  };
  const fill = (keep) => {
    const names = listed();
    const want = keep ?? s.default;
    let sel = names.indexOf(want);
    if (sel < 0 && names.length) sel = 0;
    d.setItems(ids.list, names, sel >= 0 ? sel : -1);
    s.names = names;
    s.current = sel >= 0 ? names[sel] : null;
    updateHide();
  };
  d.setChecked(ids.showHidden, s.showHidden);
  fill(null);
  d.get(ids.list)?.addEventListener("change", () => {
    s.current = d.selected(ids.list) ?? null;
    updateHide();
  });
  d.get(ids.default)?.addEventListener("click", async () => {
    const name = s.current;
    if (name === null) return;
    if (s.hidden.includes(name)) {
      await message('This template is normally hidden, so it cannot be made the default.  You must first use the "Show Template Always" button.');
      return;
    }
    store.set("DefaultTemplate", name);
    s.default = name;
    fill(name);
  });
  d.get(ids.hide)?.addEventListener("click", async () => {
    const name = s.current;
    if (name === null) return;
    if (s.default === name) {
      await message("This is the default template.  It cannot be hidden unless you first choose another default.");
      return;
    }
    s.hidden.push(name);
    store.set("HiddenTemplates", s.hidden.join("|"));
    fill(name);
  });
  d.get(ids.show)?.addEventListener("click", () => {
    const name = s.current;
    if (name === null) return;
    s.hidden = s.hidden.filter((n) => n !== name);
    store.set("HiddenTemplates", s.hidden.join("|"));
    fill(name);
  });
  d.get(ids.region)?.addEventListener("click", async () => {
    const current = [0, 1, 2, 3, 4, 5].find((i) => core.LanguageList.region_bit(i) === s.region) ?? 5;
    const { result, d: r } = await showDialog(1018, { setup: (r) => r.setItems(7, REGION_NAMES, current) });
    const chosen = r.chosenIn(7, REGION_NAMES);
    if (result === "ok" && chosen !== null) {
      s.region = core.LanguageList.region_bit(chosen);
      fill(s.current);
    }
  });
  d.get(ids.showHidden)?.addEventListener("change", () => {
    s.showHidden = d.checked(ids.showHidden);
    if (ids.kept) store.set("ShowAllTemplates", s.showHidden ? "yes" : "no");
    fill(s.current);
  });
  return s;
}

// File, New (dialog 1017): print or braille, and the template list with its buttons; the document takes the
// template chosen, as the desktop program's does.
async function fileNew() {
  const all = await templateNames();
  let state = null;
  const { result, d } = await showDialog(1017, {
    setup(d) {
      d.setRadio([7, 8], 0);
      state = templateChooser(d, NEW_TEMPLATES, all);
    },
  });
  if (result !== "ok") return;
  const template = all.find((t) => t.name === state?.current) ?? null;
  await newDocument(d.radio([7, 8]) === 1, template);
}

// A new empty document of a kind, from `template` (its file and name) or the default template.
async function newDocument(braille, template) {
  await letGo();
  doc.kind = braille ? "braille" : "print";
  doc.name = braille ? "Untitled Braille Document 1" : "Untitled Print Document 1";
  doc.file = null;
  doc.ebraille = null;
  doc.saved = false;
  doc.documentFile = false;
  doc.printer = null;
  doc.hyphen = 2902;
  doc.template = template;
  doc.dirty = false;
  embosserDefaults();
  doc.view = newView(braille);
  setEditorText("");
  core?.editor_load_plain("");
  showTitle();
  editor.focus();
  applyView();
}

// The import dialog (1043): the templates, the import filters with the kind found chosen, and the code page (text
// kinds) or braille encoding (braille kinds) to read the file in. Null when cancelled.
async function importDialog(detected) {
  const filters = core
    .import_filters()
    .split("\n")
    .map((l) => {
      const [kind, name, marks = ""] = l.split("\t");
      return { kind, name, marks };
    });
  const names = filters.map((f) => f.name);
  const templates = await templateNames();
  let chooser = null;
  const pages = core.code_pages();
  const encs = await encodings();
  const encodingLabels = encs.map((e) => e.name);
  const codePage = store.get("CodePage") ?? core.default_import_code_page();
  const encodingAt = encs.findIndex((e) => e.id === encodingNow().id);
  const showFor = (d, name) => {
    const marks = filters.find((f) => f.name === name)?.marks ?? "";
    const [label, items, at] = marks.includes("c")
      ? ["Code Page (Character Set):", pages, pages.indexOf(codePage)]
      : marks.includes("e")
        ? ["Braille Encoding:", encodingLabels, encodingAt]
        : ["", [], -1];
    d.setText(15, label);
    d.setItems(16, items, at >= 0 ? at : null);
    for (const id of [15, 16]) {
      const el = d.get(id);
      if (el) (el.closest(".row") ?? el).hidden = items.length === 0;
    }
  };
  const { result, d } = await showDialog(1043, {
    setup(d) {
      chooser = templateChooser(d, IMPORT_TEMPLATES, templates);
      const at = filters.findIndex((f) => f.kind === detected);
      d.setItems(14, names, at >= 0 ? at : names.length - 1);
      showFor(d, d.selected(14));
    },
    after(d) {
      d.get(14)?.addEventListener("change", () => showFor(d, d.selected(14)));
    },
  });
  if (result !== "ok") return null;
  const chosen = filters.find((f) => f.name === d.selected(14));
  const kind = chosen?.kind ?? detected;
  const filter = chosen?.name ?? detected;
  const marks = chosen?.marks ?? "";
  const choice = {
    kind,
    filter,
    braille: marks.includes("b"),
    codePage,
    encoding: encodingNow().id,
    template: templates.find((t) => t.name === chooser?.current) ?? null,
  };
  if (marks.includes("c") && d.selected(16)) {
    choice.codePage = d.selected(16);
    store.set("CodePage", choice.codePage);
  } else if (marks.includes("e")) {
    const i = d.chosenIn(16, encodingLabels);
    if (i !== null) choice.encoding = encs[i].id;
  }
  // Formatted braille read with interpretation may be laid out again: the notice, until it is turned off.
  if (kind === "FormattedBraille" && !braideSettings().preserve && !(await reformatNotice())) return null;
  return choice;
}

// The notice that formatted braille may be reformatted (1054), with its "Do not show this message again" box, kept
// as the desktop program keeps it (Mutable, BrailleReformatWarning: the answer plus one). False when closed without
// OK.
async function reformatNotice() {
  return onceNotice(
    "BrailleReformatWarning",
    'BrailleCraft may reformat your braille document. To avoid this, set the "Read formatted braille without interpretation" option in the Global: Formatted Braille Importer dialog.',
  );
}

// A notice (1054) with its "Do not show this message again" box, kept under Mutable by `key`
// keeps it (the answer plus one). True on OK, or when OK was kept.
async function onceNotice(key, text) {
  const kept = Number(store.get(`Mutable.${key}`));
  if (kept > 0) return kept - 1 === 1;
  const { result, d } = await showDialog(1054, { title: "BrailleCraft", setup: (d) => d.setText(5, text) });
  const answer = result === "ok" ? 1 : 2;
  if (d.checked(6)) store.set(`Mutable.${key}`, String(answer + 1));
  return answer === 1;
}

// Global, Printer Setup and Global, Embosser Setup first say that their changes do not affect the open document
// (the web build always has one), and open only on OK.
const GLOBAL_PRINTER_WARNING =
  "Changes that you make through Global, Printer Setup will not affect this document.  Use Document, Printer Setup instead to change document settings.";
const GLOBAL_EMBOSSER_WARNING =
  "Changes that you make through Global, Embosser Setup will not affect this document.  Use Document, Embosser Setup instead to change document settings.";

// Global, Formatted Braille Importer, kept under fixed names; a value that is missing
// or not understood keeps its default (main page numbers lower right, reference page numbers upper right, no running
// head or foot, 30 lines of 40 cells, formatting not preserved).
const BRAIDE_CORNERS = ["NoPages", "UpperLeft", "UpperRight", "LowerLeft", "LowerRight"];
const BRAIDE_SIDES = ["None", "Odd", "Even", "Both"];
function braideSettings() {
  const get = (key) => store.get(`FormattedBrailleImporter.${key}`);
  const word = (key, list, d) => {
    const i = list.findIndex((w) => w.toLowerCase() === (get(key) ?? "").trim().toLowerCase());
    return i >= 0 ? i : d;
  };
  const num = (key, d) => {
    const n = Number(get(key));
    return Number.isInteger(n) && n > 0 ? n : d;
  };
  return {
    oddMain: word("OddMainPages", BRAIDE_CORNERS, 4),
    evenMain: word("EvenMainPages", BRAIDE_CORNERS, 4),
    oddReference: word("OddReferencePages", BRAIDE_CORNERS, 2),
    evenReference: word("EvenReferencePages", BRAIDE_CORNERS, 2),
    header: word("Header", BRAIDE_SIDES, 0),
    footer: word("Footer", BRAIDE_SIDES, 0),
    lines: num("MaxLen", 30),
    cells: num("MaxWidth", 40),
    preserve: (get("PreserveFormatting") ?? "no").trim().toLowerCase() === "yes",
  };
}
function saveBraideSettings(s) {
  const set = (key, v) => store.set(`FormattedBrailleImporter.${key}`, String(v));
  set("OddMainPages", BRAIDE_CORNERS[s.oddMain]);
  set("EvenMainPages", BRAIDE_CORNERS[s.evenMain]);
  set("OddReferencePages", BRAIDE_CORNERS[s.oddReference]);
  set("EvenReferencePages", BRAIDE_CORNERS[s.evenReference]);
  set("Header", BRAIDE_SIDES[s.header]);
  set("Footer", BRAIDE_SIDES[s.footer]);
  set("MaxLen", s.lines);
  set("MaxWidth", s.cells);
  set("PreserveFormatting", s.preserve ? "yes" : "no");
}

// The document kind an imported file asked for itself (a "(this_is_a_braille_document)" or
// "(this_is_a_print_document)" paragraph, taken out by the import): 1 braille, 0 print, -1 nothing, when the file's
// own kind decides.
function forcedKind(imported, kindBraille) {
  return imported.forced_braille === 1 ? true : imported.forced_braille === 0 ? false : kindBraille;
}

// The Word importer's Unknown Characters setting, which coded text import uses: its list words by position
// ("ignore" 0 marks with "*", "asterisc" 1 writes nothing, "hex" 2 the Unicode value); 0 when it is not kept.
function unknownCharacters() {
  const i = ["ignore", "asterisc", "hex"].indexOf((store.get("Word.UnknownCharacters") ?? "").trim().toLowerCase());
  return i >= 0 ? i : 0;
}

// The file's braille characters in North American braille ASCII, as the formatted braille import's first step reads
// them: each character that is not a control character and that the file's encoding lists becomes the North
// American character of the same cell; any other byte stays as it is.
async function northAmerican(bytes, encodingId) {
  const enc = (await encodings()).find((e) => e.id === encodingId);
  if (!enc || encodingId === "north-american") return bytes;
  const cellOf = new Map();
  enc.cells.forEach((chars, p) => {
    for (const c of chars ?? "") if (!cellOf.has(c)) cellOf.set(c, p);
  });
  const na = naCells();
  return bytes.map((b) => {
    if (b < 0x80 && (b < 0x20 || b === 0x7f)) return b;
    const p = cellOf.get(String.fromCharCode(b));
    const c = p === undefined ? undefined : [...(na[p] ?? "")][0];
    return c && c.codePointAt(0) < 0x100 ? c.codePointAt(0) : b;
  });
}

// The print-to-braille table a document made from this template text names, for the Han script's language.
function forwardTableOf(templateSource) {
  try {
    return core.new_document(false, "", templateSource).tables[0] ?? "";
  } catch {
    return "";
  }
}

// The style names a document made from this template text has, for the Word recipe's math steps.
function styleNamesOf(templateSource) {
  try {
    return core.new_document(false, "", templateSource).style_names;
  } catch {
    return [];
  }
}

// A file as the import dialog's choice says to read it: { braille, running } (text with codes) or { braille, plain },
// with the template chosen; null after saying why it cannot be read. A PDF becomes print with codes, its styles named
// as the template names them; coded and formatted text, coded and formatted braille and Orbit braille go through the
// import chain of their kind; the kinds BrailleCraft cannot import say so.
async function readImport(bytes, choice) {
  const template = choice.template;
  if (choice.kind === "Pdf") {
    try {
      return { braille: false, running: core.import_pdf(bytes, (await styleNames()).join("\n"), JSON.stringify(pdfOptionsOf(pdfSettings()))), template };
    } catch (err) {
      statusLine.textContent = String(err.message ?? err);
      return null;
    }
  }
  // A Word document goes through the Word recipe with the Global Word Importer and Import Options settings and the
  // style map the chosen template names.
  if (choice.kind === "Docx") {
    let imported;
    try {
      await importFiles();
      const templateSource = await templateTextNamed(template);
      const map = core.word_map_name(templateSource);
      const options = JSON.stringify(wordOptions(forwardTableOf(templateSource), styleNamesOf(templateSource)));
      imported = core.import_docx(bytes, options, map, unknownCharacters());
    } catch (err) {
      statusLine.textContent = String(err.message ?? err);
      return null;
    }
    if (imported.errors) await message(imported.errors);
    return { braille: forcedKind(imported, false), running: imported.running, template };
  }
  // A Markdown file becomes HTML and goes through the HTML recipe; an HTML or SGML file goes through it as it is.
  // Both with the Word importer's hyperlink and unknown-character settings.
  if (choice.kind === "Markdown" || choice.kind === "Sgml") {
    let imported;
    try {
      await importFiles();
      const options = wordOptions(forwardTableOf(await templateTextNamed(template)));
      const run = choice.kind === "Sgml" ? core.import_html : core.import_markdown;
      imported = run(bytes, JSON.stringify(options), unknownCharacters());
    } catch (err) {
      statusLine.textContent = String(err.message ?? err);
      return null;
    }
    if (imported.errors) await message(imported.errors);
    return { braille: forcedKind(imported, false), running: imported.running, template };
  }
  if (CHAIN_KINDS.includes(choice.kind)) {
    let imported;
    try {
      imported = await importChain(bytes, choice);
    } catch (err) {
      statusLine.textContent = String(err.message ?? err);
      return null;
    }
    if (imported.long_page) await message("Page depth exceeded.");
    if (imported.wide_line) await message("Page width exceeded.");
    if (imported.errors) await message(imported.errors);
    return { braille: forcedKind(imported, choice.braille), running: imported.running, template };
  }
  await message(importRefusal(choice, bytes));
  return null;
}

// The files the import chains read, fetched once.
let importFilesLoaded = false;
async function importFiles() {
  if (!importFilesLoaded) {
    for (const name of core.import_file_names()) core.import_add_file(name, await fetchBytes(`tables/${name}`));
    importFilesLoaded = true;
  }
}

// The text of the template named (an import dialog choice), else the File, New default template's text.
async function templateTextNamed(name) {
  if (!name) return templateText();
  try {
    const index = await (await fetch("templates/index.tsv")).text();
    const row = index.split("\n").map((l) => l.split("\t")).find((c) => (c[2] ?? "").trim() === name);
    return row ? await (await fetch(`templates/${row[1]}.template`)).text() : templateText();
  } catch {
    return templateText();
  }
}

// The Word importer's settings as the Global Word Importer (1036) and Import Options (1037) dialogs last kept them
// (showDialog memory), the defaults where a dialog was never confirmed; the JSON shape of the core's
// Word options.
function wordOptions(forwardTable = "", styles = []) {
  const kept = (rid) => {
    try {
      return new Map(JSON.parse(store.get(`dialog.${rid}.`) ?? "[]"));
    } catch {
      return new Map();
    }
  };
  const w = kept(1036);
  const i = kept(1037);
  const flag = (m, id, d) => (typeof m.get(id) === "boolean" ? m.get(id) : d);
  const PAGES = ["Ignore", "Internal", "Scanned", "Publishers"];
  const pageItems = ["Ignore", "Mark from Word document", "Treat hard page breaks as print page breaks", "Use publishers' <ipp no=#> reference page tags"];
  const linkItems = ["Text Only", "URLs Only", "Both Text and URLs", "Structured Hyperlinks"];
  const pages = Math.max(0, pageItems.indexOf(w.get(10)));
  const links = linkItems.indexOf(i.get(7));
  const hanLanguage = (store.get("Word.HanScriptLanguage") ?? "").trim().toLowerCase();
  return {
    braille_fonts: flag(w, 11, false),
    cbc: flag(w, 6, false),
    ignore_bold: flag(w, 14, false),
    ignore_italics: flag(w, 15, false),
    ignore_underline: flag(w, 16, false),
    // The defaults (the settings descriptor table): language switches ignored, hard page breaks kept,
    // embedded codes [[*...*]] allowed, print page breaks ignored.
    ignore_lang: flag(w, 17, true),
    hard_pages: flag(w, 7, true),
    skipped_lines: flag(w, 8, false),
    print_pages: PAGES[pages],
    hyperlinks: links >= 0 ? links : 2,
    remove_duplicates: flag(i, 8, true),
    auto_tag_cbc: flag(i, 19, true),
    codes: flag(w, 12, true),
    add_spaces: flag(i, 20, false),
    remove_dash_spaces: flag(i, 21, false),
    // The Han script: the language by the document's print-to-braille table (HanScriptAuto), else HanScriptLanguage.
    han_auto: (store.get("Word.HanScriptAuto") ?? "yes").trim().toLowerCase() === "yes",
    han_language: Math.max(0, HAN_LANGUAGES.findIndex((w) => w.toLowerCase() === hanLanguage)),
    forward_table: forwardTable,
    styles,
  };
}

// A file through the import chain of its kind, the chain's tables fetched once.
async function importChain(bytes, choice) {
  await importFiles();
  const s = braideSettings();
  const input = choice.kind === "FormattedBraille" ? await northAmerican(bytes, choice.encoding) : bytes;
  const braide = [s.oddMain, s.evenMain, s.oddReference, s.evenReference, s.header, s.footer, s.lines, s.cells, s.preserve ? 1 : 0];
  return core.import_file(input, choice.kind, choice.codePage, unknownCharacters(), new Int32Array(braide));
}

// ------------------------------------------------------------------ commands

const fileInput = $("file-input");
fileInput.addEventListener("change", async () => {
  const file = fileInput.files[0];
  fileInput.value = "";
  if (file) await openBytes(file.name, new Uint8Array(await file.arrayBuffer()));
});

// A file opened in place of the document (File, Open; a recent document; with `recovered`, an autosaved document
// from the recovery wizard, given back what it was). The document it replaces is let go of first (letGo).
async function openBytes(name, bytes, recovered = null) {
  const file = { name };
  const ext = file.name.split(".").pop().toLowerCase();
  // The file's kind from its name and first bytes, as the desktop program finds it; a file that is not a document
  // file goes through the import dialog. The open document is left as it is until the file has been read.
  const detected = core ? core.detect_file(file.name, bytes) : "";
  let d = null;
  let read = null;
  if (detected === "Document") {
    try {
      d = core.read_document(bytes);
    } catch (err) {
      statusLine.textContent = String(err.message ?? err);
      return;
    }
  } else if (core && detected === "Pef") {
    // A PEF file opens as a braille document with no import dialog, as in the desktop program.
    try {
      read = { braille: true, plain: core.pef_open(new TextDecoder().decode(bytes)), saved: true };
    } catch (err) {
      await message(String(err.message ?? err));
      return;
    }
  } else if (core) {
    const choice = await importDialog(detected);
    if (!choice) return;
    read = await readImport(bytes, choice);
    if (!read) return;
  } else {
    // With no core, any file as text, a braille one by its name.
    read = { braille: /^(brf|brl|bru)$/.test(ext), plain: decodeTextFile(bytes).replaceAll("\f", "\n") };
  }
  await letGo();
  // A document file opened leaves the recent documents; its bytes are kept to reopen it from there later.
  if (d && !recovered) {
    inRecentList = recent.includes(name);
    recentRemove(name);
    keepRecentBytes(name, bytes);
  }
  embosserDefaults();
  doc.printer = null;
  doc.hyphen = 2902;
  doc.template = null;
  doc.dirty = false;
  if (d) {
    doc.kind = d.braille ? "braille" : "print";
    // The editor takes the file's codes as codes; brackets in its text stay text.
    const text = core.editor_load(d.running);
    setEditorText(d.braille ? fromNA(text) : text);
    if (d.braille) core.editor_convert(editorText());
    doc.file = d;
    // The document's eBraille metadata, as the file keeps it.
    doc.ebraille = d.ebraille_metadata ? JSON.parse(d.ebraille_metadata) : null;
    doc.saved = true;
    // Read from a document file: File, Save writes it again without asking (spec A15.10).
    doc.documentFile = true;
    // The caret where the document was saved (settings +22, spec 05 section A10), only for a file whose saved view
    // is taken (see below); else the start.
    const at = core.restores_view(inRecentList, d.view_font) ? d.caret : 0;
    editor.setSelectionRange(at, at);
    // The document's own embosser setup and page layout, as the file keeps them; cells and lines are taken by a
    // braille document.
    // A setup no longer kept gives message 2358 and leaves the name empty (spec 25 10.1).
    if (d.embosser) {
      if (loadSetup(d.embosser)) doc.embosser = d.embosser;
      else {
        const l = d.embosser_layout ? JSON.parse(d.embosser_layout) : null;
        await message(
          `This document is set to use paper size "${l ? core.formName(l.layout.form) : ""}" on an undefined embosser ("${d.embosser}").  You must fix this in the "Document: Embosser Settings" dialogue before embossing the document; you should choose an embosser and paper size capable of fitting ${l?.lines ?? doc.lines} lines of ${l?.cells ?? doc.cells} cells to avoid reformatting the document.`,
        );
      }
    }
    if (d.embosser_layout) {
      const l = JSON.parse(d.embosser_layout);
      doc.layout = l.layout;
      // The DoubleSizePrint byte (layout +11), for File, Print's Interline Print box.
      doc.doubleSize = l.double_size ?? 0;
      if (d.braille) {
        doc.cells = l.cells;
        doc.lines = l.lines;
      } else {
        doc.embPage = [l.cells, l.lines];
      }
    }
    // The document's own Printer Setup page, the Global, Printer Setup page size when the file holds none (spec
    // A16.4 unknown 6). A print document is laid out on it; a braille document keeps it (spec A16.2).
    const [lines, chars] = d.print_page(...globalPrintPage());
    if (d.braille) {
      doc.printPage = [lines, chars];
    } else {
      doc.lines = lines;
      doc.cells = chars;
    }
  } else {
    doc.kind = read.braille ? "braille" : "print";
    if (read.running !== undefined) {
      const text = core.editor_load(read.running);
      setEditorText(read.braille ? fromNA(text) : text);
      if (read.braille) core.editor_convert(editorText());
    } else {
      // A text file holds no codes: all of it is text.
      setEditorText(read.braille ? fromNA(read.plain) : read.plain);
      core?.editor_load_plain(editorText());
    }
    doc.file = null;
    doc.ebraille = null;
    doc.saved = read.saved ?? false;
    // An imported file is not a document file: File, Save asks (spec A15.10).
    doc.documentFile = false;
    doc.template = read.template ?? null;
    // An imported file is a new document: the caret at its start, as in the desktop program (setting a text
    // field's value leaves the caret at the end).
    editor.setSelectionRange(0, 0);
  }
  doc.name = file.name;
  // A document opens in its kind's default view (Global, Default Views). A document file's saved view (its
  // translated line) is taken only when it is reopened from the recent documents list with a valid view font in it
  // (spec 27 section 9, unknown 6).
  doc.view = newView(doc.kind === "braille");
  if (doc.file && core.restores_view(inRecentList, doc.file.view_font)) {
    doc.view.translated = doc.file.translated_line;
    // Six-key entry is kept in the file for both kinds of document.
    doc.view.six_key = doc.file.six_key;
  }
  inRecentList = false;
  if (recovered) {
    // Its document file (Save writes it again), the file it was imported from (Save asks), or untitled; changed.
    doc.name = recovered.kind === "untitled" ? `Untitled ${recovered.braille ? "Braille" : "Print"} Document ${recovered.untitled}` : recovered.name;
    doc.documentFile = recovered.kind === "saved";
    doc.saved = recovered.kind !== "untitled";
    doc.dirty = true;
  }
  showTitle();
  editor.focus();
  applyView();
}

// File, Translate: a new document made from this one, as the desktop program does: the whole text through the
// document's own rules table with its initial codes and styles; the new document keeps the tables, initial codes,
// styles and page numbering, and a braille one takes 40 cells.
async function translateDocument() {
  if (!core) {
    statusLine.textContent = "Translation is not available in this copy of the web build.";
    return;
  }
  if (!editorText().trim()) {
    await message("The file that you are trying to translate is empty.");
    return;
  }
  const toBraille = doc.kind === "print";
  const { e, file, code } = await documentEngine();
  if (!e) {
    await message(`Translation failed: the table ${code} is not in this copy of the web build.`);
    return;
  }
  let out;
  try {
    // In running form, so the translation's codes are codes in the new document.
    out = await translateText(e, storedText(), file, !toBraille, true);
  } catch (err) {
    await message(`Translation failed: ${err.message ?? err}`);
    return;
  }
  const untitled = !doc.saved && /^Untitled /.test(doc.name);
  const text = core.editor_load(out);
  doc.file = file.as_kind(toBraille, text);
  setEditorText(toBraille ? fromNA(text) : text);
  if (toBraille) core.editor_convert(editorText());
  // The translation is a new document: the caret at its start, as in the desktop program.
  editor.setSelectionRange(0, 0);
  doc.kind = toBraille ? "braille" : "print";
  doc.name = untitled ? `Untitled ${toBraille ? "Braille" : "Print"} Document 1` : `${stem()}${toBraille ? ".brf" : ".txt"}`;
  if (toBraille) doc.cells = 40;
  doc.saved = false;
  doc.dirty = true;
  doc.view = newView(doc.kind === "braille");
  showTitle();
  applyView();
}

// ------------------------------------------------------------------ File, Save and Save As

// The document's text as files keep it: braille in North American ASCII braille, whatever encoding it is shown in.
const storedText = () => (doc.kind === "braille" ? toNA(editorText()) : editorText());

// The Save As file types, in a fixed order for each kind of document (spec 04d-B); the first is
// chosen when the dialog opens, every time.
const SAVE_TYPES = {
  braille: [
    ["Braille document (*.dxb)", "dxb", "document"],
    ["Braille document, older format (*.dxb)", "dxb", "older"],
    ["Formatted braille (*.brf)", "brf", "formatted"],
    ["Coded braille (*.bru)", "bru", "coded"],
    ["Refreshable braille (*.lbf)", "lbf", "refreshable"],
    ["eBraille (*.ebrl)", "ebrl", "ebraille"],
    ["Markdown (*.md)", "md", "markdown"],
  ],
  print: [
    ["Print document (*.dxp)", "dxp", "document"],
    ["Print document, older format (*.dxp)", "dxp", "older"],
    ["Formatted text (*.prf)", "prf", "formatted"],
    ["Coded text (*.txt)", "txt", "coded"],
    ["EPUB 3 (*.epub)", "epub", "ebraille"],
    ["HTML (*.htm)", "htm", "html"],
    ["LaTeX (*.tex)", "tex", "latex"],
    ["HTML with MathJax (*.htm)", "htm", "mathjax"],
    ["Markdown (*.md)", "md", "markdown"],
  ],
};

// A small dialog of the web build's own (the desktop program uses the system's file and choice dialogs here):
// labelled rows that `fill` adds to the form, OK and Cancel. Answers whether OK closed it.
async function ownDialog(title, fill) {
  const { dialog, form } = newDialog(title, `own-${title.replace(/\W+/g, "-").toLowerCase()}`);
  fill(form);
  const buttons = document.createElement("div");
  buttons.className = "buttons";
  const ok = document.createElement("button");
  ok.type = "submit";
  ok.className = "default";
  ok.textContent = "OK";
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.textContent = "Cancel";
  cancel.addEventListener("click", () => dialog.close("cancel"));
  buttons.append(ok, cancel);
  form.append(buttons);
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    dialog.close("ok");
  });
  $("dialogs").append(dialog);
  const closed = closedPromise(dialog);
  dialog.showModal();
  focusFirst(form);
  return (await closed) === "ok";
}

function labelledRow(form, id, labelText, field) {
  const row = document.createElement("div");
  row.className = "row";
  const label = document.createElement("label");
  label.htmlFor = id;
  label.textContent = labelText;
  field.id = id;
  row.append(label, field);
  form.append(row);
  return field;
}

// A choice from a list, item `selected` chosen (the first unless given; the desktop program's single choice
// dialog): its index, or null.
async function chooseFrom(title, prompt, items, selected = 0) {
  let list;
  const ok = await ownDialog(title, (form) => {
    list = labelledRow(form, "choice-list", prompt, document.createElement("select"));
    list.size = Math.min(10, items.length);
    list.replaceChildren(...items.map((t) => new Option(t, t)));
    list.selectedIndex = selected;
  });
  return ok && list.selectedIndex >= 0 ? list.selectedIndex : null;
}

// The document's file: the one it was read from or last saved as, else a new one from the template chosen when it
// was imported or the default template, kept so the styles and page numbering changed in the Document menu stay with
// the document.
async function documentFile() {
  if (!doc.file && doc.template) {
    try {
      doc.file = core.new_document(doc.kind === "braille", editorText(), await (await fetch(`templates/${doc.template.file}.template`)).text());
    } catch {
      // The template could not be read: the default template, as for a new document.
    }
  }
  doc.file ??= core.new_document(doc.kind === "braille", editorText(), await templateText());
  return doc.file;
}

// Writes the document as a document file (.dxp or .dxb) in format `version` (1, or 0 for the older format), with
// its embosser setup name and layout. In the current format the document then takes that file as its own; a file
// in the older format is a copy only, and the document keeps its earlier file and name (spec A15.10).
async function writeDocument(name, version) {
  const bytes = await documentBytes(version);
  download(name, bytes);
  if (version === 0) return;
  doc.documentFile = true;
  doc.file = core.read_document(bytes);
  doc.name = name;
  doc.saved = true;
  doc.dirty = false;
  showTitle();
  // The bytes kept to reopen the file from the recent documents; Save As onto a name takes it out of them.
  keepRecentBytes(name, bytes);
  recentRemove(name);
}

// The document as a document file in format `version` (1, or 0 for the older format): what Save writes and what
// autosave keeps.
async function documentBytes(version) {
  const file = await documentFile();
  // The view saved with the document: its translated line and six-key entry.
  file.set_view(!!doc.view.translated, !!doc.view.six_key);
  // The eBraille metadata, kept in the file's version 2 section.
  file.set_ebraille(doc.ebraille ? JSON.stringify(doc.ebraille) : "");
  const layout = doc.layout.form ? JSON.stringify(doc.layout) : "";
  const [printLines, printChars] = doc.printPage ?? globalPrintPage();
  const bytes = file.save_version(
    storedText(),
    doc.embosser,
    layout,
    doc.cells,
    doc.lines,
    version,
    doc.embPage[0],
    doc.embPage[1],
    editor.selectionStart,
    printLines,
    printChars,
  );
  return bytes;
}

// Formatted braille (.brf): the encoding of the file chosen from "12.4 Compatible (CAPS)", the national encodings
// and "Unicode Braille Patterns", the first chosen every time (spec 04d-B (a), (f)). Formatted text (.prf): the code
// page chosen (spec 04d-E). Answers the bytes, or null when cancelled.
async function formattedBytes(braille) {
  if (!braille) {
    const pages = core.code_pages();
    const i = await chooseFrom("Formatted text", "Code page of the file:", pages);
    return i === null ? null : core.formatted_text(editorText(), doc.cells, doc.lines, pages[i]);
  }
  const all = await encodings();
  const na = all.find((e) => e.id === "north-american");
  const i = await chooseFrom("Formatted braille", "Braille encoding of the file:", ["12.4 Compatible (CAPS)", ...all.map((e) => e.name), "Unicode Braille Patterns"]);
  if (i === null) return null;
  const first = (enc, p) => [...(enc?.cells[p] ?? "")][0];
  let codes;
  if (i === 0) {
    // North American ASCII braille in capitals; a character outside ASCII is written '#'.
    codes = [...Array(64).keys()].map((p) => {
      const c = (first(na, p) ?? " ").toUpperCase().codePointAt(0);
      return c < 128 ? c : 0x23;
    });
  } else if (i <= all.length) {
    codes = [...Array(64).keys()].map((p) => first(all[i - 1], p)?.codePointAt(0) ?? 0x2800 + p);
  } else {
    codes = [];
  }
  // Each character goes to its cell through the encoding the document is shown in.
  const shownCells = encodingNow().cells.length ? encodingNow().cells : na.cells;
  return core.formatted_braille(editorText(), doc.cells, doc.lines, JSON.stringify(shownCells), Uint32Array.from(codes));
}

// File, Save As: the name and type chosen, then the file written as that type and saved by the browser. A name
// typed with the extension of another type gives the first type with that extension. The dialog is titled
// "Save as:" for File, Save As and "Save:" when File, Save asks (spec A15.10).
async function saveAs(title = "Save as:") {
  const braille = doc.kind === "braille";
  const types = SAVE_TYPES[braille ? "braille" : "print"];
  let nameField;
  let typeField;
  const ok = await ownDialog(title, (form) => {
    nameField = labelledRow(form, "save-name", "File name:", document.createElement("input"));
    nameField.type = "text";
    nameField.value = `${stem()}.${types[0][1]}`;
    typeField = labelledRow(form, "save-type", "Save as type:", document.createElement("select"));
    typeField.replaceChildren(...types.map(([label]) => new Option(label, label)));
    typeField.selectedIndex = 0;
    // Choosing a type changes the extension of the name, as file dialogs do.
    typeField.addEventListener("change", () => {
      nameField.value = nameField.value.replace(/(\.[^.\\/]*)?$/, `.${types[typeField.selectedIndex][1]}`);
    });
  });
  if (!ok || !nameField.value.trim()) return;
  let name = nameField.value.trim().split(/[\\/]/).pop();
  const filter = types[typeField.selectedIndex];
  const typed = /\.([^.]+)$/.exec(name)?.[1]?.toLowerCase();
  const type = typed && typed !== filter[1] ? types.find((t) => t[1] === typed) ?? filter : filter;
  if (typed !== type[1]) {
    name = `${name.replace(/\.[^.]*$/, "")}.${type[1]}`;
  }
  try {
    let bytes;
    switch (type[2]) {
      case "document":
        return await writeDocument(name, 1);
      case "older":
        return await writeDocument(name, 0);
      case "formatted":
        bytes = await formattedBytes(braille);
        break;
      case "coded":
        bytes = core.coded(
          storedText(),
          await fetchBytes(`tables/rule-scripts/${braille ? "coded-braille" : "coded-text"}.rules`),
          await fetchBytes("tables/rule-scripts/coded-wrap.rules"),
        );
        break;
      case "refreshable":
        bytes = (await documentFile()).refreshable(storedText());
        break;
      case "ebraille":
        bytes = await ebraillePackage();
        break;
      case "html":
        bytes = core.html(
          editorText(),
          await fetchBytes("tables/rule-scripts/html-tags.rules"),
          await fetchBytes("tables/rule-scripts/html-italics.rules"),
          await fetchBytes("tables/rule-scripts/html-styles.rules"),
          await fetchBytes("tables/rule-scripts/html-fix.rules"),
        );
        break;
      case "markdown":
        // Print: the HTML export turned into Markdown. Braille: the pages as Unicode braille in fenced blocks.
        if (braille) {
          const e = encodingNow();
          const shownCells = e.cells.length ? e.cells : naCells();
          bytes = core.markdown_braille(editorText(), doc.cells, doc.lines, JSON.stringify(shownCells));
        } else {
          bytes = core.markdown(
            editorText(),
            await fetchBytes("tables/rule-scripts/html-tags.rules"),
            await fetchBytes("tables/rule-scripts/html-italics.rules"),
            await fetchBytes("tables/rule-scripts/html-styles.rules"),
            await fetchBytes("tables/rule-scripts/html-fix.rules"),
          );
        }
        break;
      default:
        // LaTeX, or HTML with MathJax: the text export table, with the document's styles and the language
        // switches of the table list, and that type's own rule script.
        bytes = core[type[2]](
          editorText(),
          new TextDecoder().decode(await fetchBytes("tables/table-txxtxa-binary-forward.bmt")),
          await fetchBytes(`tables/rule-scripts/${type[2]}.rules`),
          await documentFile(),
          (await languages()) ?? new core.LanguageList(""),
        );
    }
    // The document itself keeps its own file and name.
    if (bytes) download(name, bytes);
  } catch (err) {
    await message(String(err.message ?? err));
  }
}

// eBraille and EPUB 3 (spec 23). The package metadata of the open document (Document, eBraille Metadata), with
// the desktop program's field names; the web form shows the same fields as rows (the desktop dialog's pages and
// its language, braille system and attribute dialogs are rows here: one entry per line).
const EBRAILLE_QUALITY_WARNING =
  "eBraille exports should be reliably working to produce readable eBraille files, but the quality is not yet reliable. Any sample files sent to the support team will help us to improve the quality.";

function ebrailleMeta() {
  if (!doc.ebraille) {
    doc.ebraille = {
      titles: [], creators: [], producers: [], languages: [], braille_systems: [], date: "", id_type: 2, identifier: "",
      date_copyrighted: "", tactile_graphics: false, complete_transcription: false, publishers: [], education_levels: [],
      description: "", rights: [], source_type: 0, source: "", subjects: [], properties: [], field_attributes: [], edited_xml: null,
    };
  }
  return doc.ebraille;
}

// The document's translation table entry's eBraille language and braille system: [language, system], each null
// when the entry sets none.
async function ebrailleDefaults() {
  const list = await languages();
  if (!list) return [null, null];
  return JSON.parse(list.ebraille_defaults((await documentFile()).tables));
}

// The package bytes: the quality notice first (kept under Mutable, EBrailleQualityWarning until its box is checked;
// Cancel stops the save), then Document, eBraille Metadata (Cancel stops the save), then the core builds the package
// from the document, its styles, its metadata and its translation table's language and braille system.
async function ebraillePackage() {
  if (!(await onceNotice("EBrailleQualityWarning", EBRAILLE_QUALITY_WARNING))) return null;
  if (!(await ebrailleMetadata())) return null;
  const e = encodingNow();
  const cells = e.cells.length ? e.cells : naCells();
  const [language, system] = await ebrailleDefaults();
  return (await documentFile()).ebraille(editorText(), doc.kind === "braille", JSON.stringify(ebrailleMeta()), JSON.stringify(cells), language ?? "", system ?? "", doc.name ?? "");
}

// File, Save eBraille (1107) is a developer build's test save (a file in a test folder named by a setting); the web
// build is not one, so the item does nothing, as in a release build.
async function saveEBraille() {}

// Document, eBraille Metadata (2606): the fields of the desktop dialog's Required, Recommended and Additional pages
// as labelled rows; lists are one entry per line, braille systems as "language grade specialization", properties
// as name=value; the XML preview is shown from the fields.
async function ebrailleMetadata() {
  const stored = JSON.stringify(ebrailleMeta());
  // The translation table's eBraille language and braille system where the metadata names none.
  const [language, system] = await ebrailleDefaults();
  const m = ebrailleMeta();
  if (!m.languages.some((l) => l.trim()) && language) m.languages = [language];
  if (!m.braille_systems.some((s) => `${s.language}${s.grade}`.trim()) && system) m.braille_systems = [{ language: system, grade: "", specialization: "" }];
  const fields = {};
  const area = (form, id, label, value) => {
    const t = labelledRow(form, id, label, document.createElement("textarea"));
    t.rows = 3;
    t.value = value;
    return t;
  };
  const input = (form, id, label, value, type = "text") => {
    const i = labelledRow(form, id, label, document.createElement("input"));
    i.type = type;
    if (type === "checkbox") i.checked = value;
    else i.value = value;
    return i;
  };
  const choice = (form, id, label, items, selected) => {
    const s = labelledRow(form, id, label, document.createElement("select"));
    s.replaceChildren(...items.map((it) => new Option(it, it)));
    s.selectedIndex = selected;
    return s;
  };
  const ok = await ownDialog("eBraille Metadata", (form) => {
    fields.titles = area(form, "ebr-title", "Titles (1 per line):", m.titles.join("\n"));
    fields.creators = area(form, "ebr-creator", "Creators (1 per line):", m.creators.join("\n"));
    fields.producers = area(form, "ebr-producer", "Producers (1 per line):", m.producers.join("\n"));
    fields.languages = area(form, "ebr-languages", "Languages (1 per line):", m.languages.join("\n"));
    // A system with no grade is a system text taken as it is (a translation table's, or one read from a file).
    fields.systems = area(form, "ebr-systems", "Braille systems (language grade specialization, 1 per line):", m.braille_systems.map((s) => (s.grade ? `${s.language} ${s.grade} ${s.specialization}` : s.language)).join("\n"));
    fields.date = input(form, "ebr-date", "Date:", m.date, "date");
    fields.idType = choice(form, "ebr-idtype", "ID type:", ["ISBN", "DOI", "UUID"], m.id_type);
    fields.identifier = input(form, "ebr-id", "Identifier:", m.identifier);
    fields.copyrighted = input(form, "ebr-copyrighted", "Date copyrighted:", m.date_copyrighted, "date");
    fields.tactile = input(form, "ebr-tactile", "Tactile Graphics", m.tactile_graphics, "checkbox");
    fields.complete = input(form, "ebr-complete", "Complete Transcription", m.complete_transcription, "checkbox");
    fields.publishers = area(form, "ebr-publisher", "Publishers (1 per line):", m.publishers.join("\n"));
    fields.education = area(form, "ebr-education", "Education Levels (1 per line):", m.education_levels.join("\n"));
    fields.description = input(form, "ebr-description", "Description:", m.description);
    fields.rights = area(form, "ebr-rights", "Rights (1 per line):", m.rights.join("\n"));
    fields.sourceType = choice(form, "ebr-sourcetype", "Source type:", ["ISBN", "DOI", "UUID"], m.source_type);
    fields.source = input(form, "ebr-source", "Source:", m.source);
    fields.subjects = area(form, "ebr-subject", "Subjects (1 per line):", m.subjects.join("\n"));
    fields.properties = area(form, "ebr-properties", "Additional properties (name=value, 1 per line):", m.properties.map((p) => `${p.name}=${p.value}`).join("\n"));
  });
  if (!ok) {
    doc.ebraille = JSON.parse(stored);
    return false;
  }
  const lines = (t) => t.value.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
  m.titles = lines(fields.titles);
  m.creators = lines(fields.creators);
  m.producers = lines(fields.producers);
  m.languages = lines(fields.languages);
  const GRADES = ["grade0", "grade1", "grade2", "no-grade"];
  m.braille_systems = lines(fields.systems).map((l) => {
    const [language = "", grade = "", specialization = "None", ...rest] = l.split(/\s+/);
    if (!GRADES.includes(grade) || rest.length) return { language: l, grade: "", specialization: "" };
    return { language, grade, specialization };
  });
  m.date = fields.date.value;
  m.id_type = fields.idType.selectedIndex;
  m.identifier = fields.identifier.value;
  m.date_copyrighted = fields.copyrighted.value;
  m.tactile_graphics = fields.tactile.checked;
  m.complete_transcription = fields.complete.checked;
  m.publishers = lines(fields.publishers);
  m.education_levels = lines(fields.education);
  m.description = fields.description.value;
  m.rights = lines(fields.rights);
  m.source_type = fields.sourceType.selectedIndex;
  m.source = fields.source.value;
  m.subjects = lines(fields.subjects);
  m.properties = lines(fields.properties).map((l) => {
    const at = l.indexOf("=");
    return at < 0 ? { name: l, value: "", attributes: [] } : { name: l.slice(0, at), value: l.slice(at + 1), attributes: [] };
  });
  // The document is marked changed only when its metadata changed.
  if (JSON.stringify(m) !== stored) doc.dirty = true;
  return true;
}

// File, Save: a document read from or saved as a document file (.dxp, .dxb) is saved as that file again; any other
// is asked about (Save As).
async function save(asNew) {
  if (!core) {
    download(`${stem()}.txt`, editorText(), "text/plain");
    return;
  }
  // A document read from or saved to a document file in the current format is written again, whatever its
  // extension; any other asks with Save As, titled "Save:" (spec A15.10).
  if (!asNew && doc.documentFile && doc.name) {
    await writeDocument(doc.name, 1);
    return;
  }
  await saveAs(asNew ? "Save as:" : "Save:");
}

// ------------------------------------------------------------------ View and Global

function setView(change) {
  change(doc.view);
  applyView();
}

// Message 2634: a view switch or a font the editing area cannot hold one row of; message 2636: the translated line
// turned on in an editing area that would then hold no row.
const TOO_SMALL_FOR_FONT =
  "The window is not large enough to use that font.  " +
  "Make the window larger, or make the font size smaller, and try again.";
const TOO_SMALL_FOR_LINE =
  "The window is not large enough for a translated line.  " +
  "Make the window larger, or make the font size smaller, and try again.";

// The height of one row of the editing area's font.
function rowHeight() {
  const style = getComputedStyle(editor);
  const line = parseFloat(style.lineHeight);
  return Number.isFinite(line) ? line : parseFloat(style.fontSize) * 1.2;
}

// View, Full-Page, the fonts, Codes and Translated Line (spec 27 section 9): the view changes, and when the editing
// area then holds no whole row the old view comes back with the message (none when `allowed` says the change is
// never refused, as turning the translated line off).
async function fittedView(change, allowed = false, refusal = TOO_SMALL_FOR_FONT) {
  const old = { ...doc.view };
  setView(change);
  if (!core || core.translated_line_allowed(allowed, editor.clientHeight, Math.ceil(rowHeight()), 0)) return;
  doc.view = old;
  applyView();
  await message(refusal);
}

const FONT_FACES = ["print", "braille", "simbraille"];

// Global, Default Views (dialog 1033): the view new documents of each kind take.
async function defaultViews() {
  const v = settings.views;
  let chosen = null;
  await showDialog(1033, {
    setup(d) {
      d.setRadio([7, 8, 9], Math.max(0, FONT_FACES.indexOf(v.translated_font)));
      d.setChecked(10, v.print_full_page);
      d.setChecked(11, v.print_codes);
      d.setChecked(12, v.print_translated);
      d.setRadio([14, 15, 16], Math.max(0, FONT_FACES.indexOf(v.braille_font)));
      d.setChecked(17, v.braille_full_page);
      d.setChecked(18, v.braille_codes);
      d.setChecked(19, v.braille_translated);
      d.setChecked(20, v.six_key);
    },
    onOk(d) {
      chosen = {
        translated_font: FONT_FACES[d.radio([7, 8, 9]) ?? 0],
        print_full_page: d.checked(10),
        print_codes: d.checked(11),
        print_translated: d.checked(12),
        braille_font: FONT_FACES[d.radio([14, 15, 16]) ?? 0],
        braille_full_page: d.checked(17),
        braille_codes: d.checked(18),
        braille_translated: d.checked(19),
        six_key: d.checked(20),
      };
      return true;
    },
  });
  if (!chosen) return;
  settings.views = chosen;
  store.set("views.default", JSON.stringify(chosen));
  applyView();
}

// Global, Internationalization (dialog 1051): the braille encoding documents are shown in. Braille documents keep
// their cells: their text is rewritten in the new encoding's characters; codes keep theirs.
async function internationalization() {
  const all = await encodings();
  const current = settings.encoding;
  const { result, d } = await showDialog(1051, {
    memory: "",
    setup(d) {
      d.setItems(7, ["English"], 0);
      d.setItems(10, all.map((e) => e.name), all.findIndex((e) => e.id === current));
      d.setRadio([11, 12], settings.preferPatterns ? 1 : 0);
      d.setChecked(14, settings.uia);
      d.setRadio([15, 16], settings.accessPatterns ? 1 : 0);
    },
  });
  if (result !== "ok") return;
  settings.uia = d.checked(14);
  store.set("uia", settings.uia ? "1" : "0");
  settings.preferPatterns = d.radio([11, 12]) === 1;
  store.set("PreferBraillePatterns", settings.preferPatterns ? "1" : "0");
  settings.accessPatterns = d.radio([15, 16]) === 1;
  store.set("AccessUseBraillePatterns", settings.accessPatterns ? "1" : "0");
  const chosen = all.find((e) => e.name === d.selected(10));
  if (chosen && chosen.id !== current) {
    if (doc.kind === "braille" && core) {
      const text = core.convert_braille(editorText(), JSON.stringify(encodingNow().cells), JSON.stringify(chosen.cells));
      settings.encoding = chosen.id;
      currentEncoding = chosen;
      setEditorText(text);
      // Converted cell for cell: the codes stay where they are.
      core.editor_convert(text);
    }
    settings.encoding = chosen.id;
    store.set("encoding", chosen.id);
    currentEncoding = chosen;
  }
  // The field shows the patterns, or the characters, as now chosen.
  reshowEditor();
  applyView();
}

// Global, PDF Importer (XRC dialog 3116, as the desktop program shows it): the
// settings kept under "PDF" with the standard keys and defaults (numbers in thousandths), the fields in the
// dialog's order (its pages General, Headers, Footers, Fonts, Scraping Setup as groups). The dialog also
// shows a script choice, height units, header and footer trigger texts and "may be shorter" boxes that its code
// never reads; those are shown too and do nothing, as there.
const PDF_DEFAULTS = {
  ShowAtImport: false, PDF_A: true, DetectLedgers: true, FixLeaderDots: true, Script: "", HeaderHeight: 0, FooterHeight: 0,
  UseHeaderFontHints: true, RetainBold: true, RetainItalic: true, RetainUnderline: true,
  "PDFMiner.Advanced": true, "PDFMiner.LineOverlap": 500, "PDFMiner.CharMargin": 2000, "PDFMiner.WordMargin": 100, "PDFMiner.LineMargin": 500, "PDFMiner.BoxesFlow": 500,
  "PDFMiner.DetectVertical": false, "PDFMiner.AllTexts": false,
};
function pdfSettings() {
  const s = { ...PDF_DEFAULTS };
  for (const k of Object.keys(PDF_DEFAULTS)) {
    const v = store.get(`PDF.${k}`);
    if (v === null || v === undefined) continue;
    s[k] = typeof PDF_DEFAULTS[k] === "boolean" ? v === "1" || v === "yes" : typeof PDF_DEFAULTS[k] === "number" ? Number(v) : v;
  }
  return s;
}
function savePdfSettings(s) {
  for (const k of Object.keys(PDF_DEFAULTS)) store.set(`PDF.${k}`, typeof s[k] === "boolean" ? (s[k] ? "1" : "0") : String(s[k]));
}
// The importer's options from the settings, as the desktop program makes them (spec 29 3b): heights from
// thousandths of an inch to points, layout numbers from thousandths; without Advanced, no box flow.
function pdfOptionsOf(s) {
  const k = (v) => v / 1000;
  return {
    allow_tagged: s.PDF_A,
    header_height: s.HeaderHeight * 0.072,
    footer_height: s.FooterHeight * 0.072,
    la: {
      line_overlap: k(s["PDFMiner.LineOverlap"]),
      char_margin: k(s["PDFMiner.CharMargin"]),
      line_margin: k(s["PDFMiner.LineMargin"]),
      word_margin: k(s["PDFMiner.WordMargin"]),
      boxes_flow: s["PDFMiner.Advanced"] ? k(s["PDFMiner.BoxesFlow"]) : null,
      detect_vertical: s["PDFMiner.DetectVertical"],
      all_texts: s["PDFMiner.AllTexts"],
    },
  };
}
const shownThousandth = (v) => String(v / 1000);
async function pdfOptions() {
  const s = pdfSettings();
  const f = {};
  const input = (form, id, label, value, type = "text") => {
    const i = labelledRow(form, id, label, document.createElement("input"));
    i.type = type;
    if (type === "checkbox") i.checked = value;
    else i.value = value;
    return i;
  };
  const choice = (form, id, label, items, selected) => {
    const sel = labelledRow(form, id, label, document.createElement("select"));
    sel.replaceChildren(...items.map((it) => new Option(it, it)));
    sel.selectedIndex = selected;
    return sel;
  };
  const heading = (form, text) => {
    const h = document.createElement("h3");
    h.textContent = text;
    form.append(h);
  };
  const ok = await ownDialog("PDF Importer", (form) => {
    heading(form, "General");
    f.show = input(form, "pdf-show", "Show This Dialog for Each Import", s.ShowAtImport, "checkbox");
    f.pdfa = input(form, "pdf-a", "Use PDF/A Markup When Possible", s.PDF_A, "checkbox");
    f.ledgers = input(form, "pdf-ledgers", "Detect Ledger Format When Scraping", s.DetectLedgers, "checkbox");
    f.leader = input(form, "pdf-leader", "Fix Leader Dots", s.FixLeaderDots, "checkbox");
    f.script = input(form, "pdf-script", "Custom Python Script:", s.Script);
    choice(form, "pdf-engine", "Reader:", ["pdfminer.six", "PyPDF"], 0);
    heading(form, "Headers");
    f.header = input(form, "pdf-header", "Header Height:", shownThousandth(s.HeaderHeight));
    choice(form, "pdf-header-units", "Units:", ["inches", "centimeters"], 0);
    input(form, "pdf-header-max", "Header May Be Shorter", false, "checkbox");
    input(form, "pdf-header-triggers", "End Header After Any Text Matching:", "");
    input(form, "pdf-header-required", "Page Has No Header if No Text Matches", false, "checkbox");
    heading(form, "Footers");
    f.footer = input(form, "pdf-footer", "Footer Height:", shownThousandth(s.FooterHeight));
    choice(form, "pdf-footer-units", "Units:", ["inches", "centimeters"], 0);
    input(form, "pdf-footer-max", "Footer May Be Shorter", false, "checkbox");
    input(form, "pdf-footer-triggers", "Begin Footer With Any Text Matching:", "");
    input(form, "pdf-footer-required", "Page Has No Footer if No Text Matches", false, "checkbox");
    heading(form, "Fonts");
    const note = document.createElement("p");
    note.textContent = "Note: Use of any of these font options can substantially slow down the PDF importer";
    form.append(note);
    f.hints = input(form, "pdf-hints", "Use Font Hints for Headers", s.UseHeaderFontHints, "checkbox");
    f.bold = input(form, "pdf-bold", "Retain Bold", s.RetainBold, "checkbox");
    f.italic = input(form, "pdf-italic", "Retain Italic", s.RetainItalic, "checkbox");
    f.underline = input(form, "pdf-underline", "Retain Underline", s.RetainUnderline, "checkbox");
    heading(form, "Scraping Setup: Layout Analysis Parameters");
    f.advanced = input(form, "pdf-advanced", "Advanced Layout Analysis", s["PDFMiner.Advanced"], "checkbox");
    f.overlap = input(form, "pdf-overlap", "Line Overlap", shownThousandth(s["PDFMiner.LineOverlap"]));
    f.charMargin = input(form, "pdf-char", "Character Margin", shownThousandth(s["PDFMiner.CharMargin"]));
    f.wordMargin = input(form, "pdf-word", "Word Margin", shownThousandth(s["PDFMiner.WordMargin"]));
    f.lineMargin = input(form, "pdf-line", "Line Margin", shownThousandth(s["PDFMiner.LineMargin"]));
    f.boxes = input(form, "pdf-boxes", "Boxes Flow", shownThousandth(s["PDFMiner.BoxesFlow"]));
    f.vertical = input(form, "pdf-vertical", "Detect Vertical", s["PDFMiner.DetectVertical"], "checkbox");
    f.allTexts = input(form, "pdf-alltexts", "All Texts", s["PDFMiner.AllTexts"], "checkbox");
  });
  if (!ok) return;
  // The number fields checked as numbers (a number from 0 to its limit); Line Overlap is shown and not
  // read back.
  const num = (field, label, max, old) => {
    const v = Number(field.value.trim());
    if (field.value.trim() === "" || Number.isNaN(v) || v < 0 || v > max) {
      message(`${label} must be a number between 0.000000 and ${max.toFixed(6)}`);
      return null;
    }
    return Math.round(v * 1000);
  };
  const header = num(f.header, "Header Height:", 10, s.HeaderHeight);
  if (header === null) return;
  const footer = num(f.footer, "Footer Height:", 10, s.FooterHeight);
  if (footer === null) return;
  const charMargin = num(f.charMargin, "Character Margin", 4, s["PDFMiner.CharMargin"]);
  if (charMargin === null) return;
  const wordMargin = num(f.wordMargin, "Word Margin", 1, s["PDFMiner.WordMargin"]);
  if (wordMargin === null) return;
  const lineMargin = num(f.lineMargin, "Line Margin", 1, s["PDFMiner.LineMargin"]);
  if (lineMargin === null) return;
  let boxes = s["PDFMiner.BoxesFlow"];
  if (f.advanced.checked) {
    boxes = num(f.boxes, "Boxes Flow", 1, s["PDFMiner.BoxesFlow"]);
    if (boxes === null) return;
  }
  savePdfSettings({
    ...s,
    ShowAtImport: f.show.checked,
    PDF_A: f.pdfa.checked,
    DetectLedgers: f.ledgers.checked,
    FixLeaderDots: f.leader.checked,
    Script: f.script.value,
    HeaderHeight: header,
    FooterHeight: footer,
    UseHeaderFontHints: f.hints.checked,
    RetainBold: f.bold.checked,
    RetainItalic: f.italic.checked,
    RetainUnderline: f.underline.checked,
    "PDFMiner.Advanced": f.advanced.checked,
    "PDFMiner.CharMargin": charMargin,
    "PDFMiner.WordMargin": wordMargin,
    "PDFMiner.LineMargin": lineMargin,
    "PDFMiner.BoxesFlow": boxes,
    "PDFMiner.DetectVertical": f.vertical.checked,
    "PDFMiner.AllTexts": f.allTexts.checked,
  });
}

// Global, Python Options (XRC dialog 3110): the custom library path, Include
// Built-In Library, the logging level and the log file, kept under "Python" with the standard keys and
// defaults. BrailleCraft reads PDFs with its own reader and runs no Python, so these have no effect on an import.
const PYTHON_LEVELS = ["Critical", "Error", "Warning", "Info", "Debug"];
async function pythonOptions() {
  const s = {
    Lib: store.get("Python.Lib") ?? "",
    IncludeBuiltinLib: (store.get("Python.IncludeBuiltinLib") ?? "yes") !== "no",
    LogLevel: Math.min(4, Number(store.get("Python.LogLevel") ?? 3) || 0),
    LogFile: store.get("Python.LogFile") ?? "",
  };
  const f = {};
  const ok = await ownDialog("Python Options", (form) => {
    f.lib = labelledRow(form, "py-lib", "Custom Library Path:", document.createElement("input"));
    f.lib.value = s.Lib;
    f.builtin = labelledRow(form, "py-builtin", "Include Built-In Library", document.createElement("input"));
    f.builtin.type = "checkbox";
    f.builtin.checked = s.IncludeBuiltinLib;
    f.level = labelledRow(form, "py-level", "Logging Level:", document.createElement("select"));
    f.level.replaceChildren(...PYTHON_LEVELS.map((it) => new Option(it, it)));
    f.level.selectedIndex = s.LogLevel;
    f.log = labelledRow(form, "py-log", "Logging Destination (Leave Blank for Internal Dialog):", document.createElement("input"));
    f.log.value = s.LogFile;
  });
  if (!ok) return;
  store.set("Python.Lib", f.lib.value);
  store.set("Python.IncludeBuiltinLib", f.builtin.checked ? "yes" : "no");
  store.set("Python.LogLevel", String(Math.max(0, f.level.selectedIndex)));
  store.set("Python.LogFile", f.log.value);
}

// Global, Fonts (dialog 1049): the print font, the braille fonts' size, printed output sizes and the status line font.
async function globalFonts() {
  const f = settings.fonts;
  const faces = ["Courier New", "Consolas", "Arial", "Times New Roman", "Verdana"];
  const status = ["Segoe UI", "Arial", "Courier New", "Verdana"];
  const { result, d } = await showDialog(1049, {
    setup(d) {
      d.setItems(7, faces, Math.max(0, faces.indexOf(f.print_face)));
      d.setText(9, String(f.print_size));
      d.setText(11, String(f.braille_size));
      // Printed output (spec 24, 2c): auto-determine the point size, or the print font's and the braille fonts' sizes.
      d.setChecked(12, f.print_auto);
      d.setText(14, String(f.print_output_size));
      d.setText(16, String(f.braille_output_size));
      d.setItems(18, status, Math.max(0, status.indexOf(f.status_face)));
      d.setText(20, String(f.status_size));
    },
  });
  if (result !== "ok") return;
  const within = (n, lo, hi) => n !== null && n >= lo && n <= hi;
  f.print_auto = d.checked(12);
  if (within(d.number(14), 1, 72)) f.print_output_size = d.number(14);
  if (within(d.number(16), 1, 72)) f.braille_output_size = d.number(16);
  if (d.selected(7)) f.print_face = d.selected(7);
  if (within(d.number(9), 6, 72)) f.print_size = d.number(9);
  if (within(d.number(11), 6, 72)) f.braille_size = d.number(11);
  if (within(d.number(20), 6, 36)) f.status_size = d.number(20);
  if (d.selected(18)) f.status_face = d.selected(18);
  store.set("fonts", JSON.stringify(f));
  applyView();
}

// Global, View Preferences (dialog 1034): verbose labels (the status line's character field gives the character's
// Unicode value), line numbers beside a printed braille page, and the six keys of six-key entry.
async function viewPreferences() {
  const { result, d } = await showDialog(1034, {
    memory: "",
    setup(d) {
      d.setText(7, "0");
      d.setChecked(9, settings.verbose);
      d.setChecked(10, settings.lineNumbers);
      d.setItems(17, ["Hide All", "Hide Idle", "Show All"], 0);
      d.setText(19, settings.sixKeySet);
    },
    // The six-key set, when changed from the one kept, is checked first (bc_core::sixkey): message 2768, the field
    // selected, and the dialog stays open.
    async onOk(d) {
      const keys = d.text(19);
      const problem = keys === settings.sixKeySet || !core ? "" : core.six_key_set_problem(keys);
      if (!problem) return true;
      await message(problem);
      d.get(19)?.focus();
      d.get(19)?.select?.();
      return false;
    },
  });
  if (result !== "ok") return;
  settings.verbose = d.checked(9);
  store.set("verbose", settings.verbose ? "1" : "0");
  settings.lineNumbers = d.checked(10);
  store.set("Display.ShowLineNumbers", settings.lineNumbers ? "yes" : "no");
  showStatus();
  const keys = d.text(19);
  settings.sixKeySet = keys.toLowerCase();
  store.set("sixkey.set", settings.sixKeySet);
}

// The message shown about a shortcut code field, or null when it is accepted: empty, longer than 99, or codes
// not properly started or ended (a code started inside a code, ended outside one, or left open).
function shortcutFieldProblem(text) {
  if (text.length === 0) return "The shortcut string is empty.";
  if (text.length > 99) return "The shortcut string must not be longer than 99 characters.";
  let open = false;
  for (const c of text) {
    if (c === "[") {
      if (open) return "Some codes are not properly started or terminated.";
      open = true;
    } else if (c === "]") {
      if (!open) return "Some codes are not properly started or terminated.";
      open = false;
    }
  }
  return open ? "Some codes are not properly started or terminated." : null;
}

// Global, Shortcut Preferences (dialog 1022): the codes the editor's shortcut keys insert and the page keys' moves.
async function shortcutPreferences() {
  const s = settings.shortcuts;
  let chosen = null;
  await showDialog(1022, {
    third: true,
    focus: 7,
    setup(d) {
      // The page-navigation radio buttons are kept as soon as one is clicked; Cancel does not take them back.
      for (const [key, ids] of [["page_nav", [19, 20, 21]], ["control_page_nav", [23, 24, 25]]]) {
        ids.forEach((id, k) =>
          d.get(id)?.addEventListener("change", () => {
            settings.shortcuts[key] = k;
            store.set("shortcuts", JSON.stringify(settings.shortcuts));
          }),
        );
      }
      d.setText(7, s.grade0);
      d.setText(9, s.flush_right_leader);
      d.setText(11, s.cbc);
      d.setText(13, s.new_line);
      d.setText(15, s.new_page);
      d.setText(17, s.new_ref_page);
      d.setRadio([19, 20, 21], Math.min(s.page_nav, 2));
      d.setRadio([23, 24, 25], Math.min(s.control_page_nav, 2));
    },
    async onOk(d) {
      // Each code field is checked in order: not empty, at most 99 characters, codes
      // properly started and ended.
      for (const field of [7, 9, 11, 13, 15, 17]) {
        const problem = shortcutFieldProblem(d.text(field));
        if (problem) {
          await message(problem);
          d.get(field)?.focus();
          d.get(field)?.select?.();
          return false;
        }
      }
      chosen = {
        grade0: d.text(7),
        flush_right_leader: d.text(9),
        cbc: d.text(11),
        new_line: d.text(13),
        new_page: d.text(15),
        new_ref_page: d.text(17),
      };
      return true;
    },
  });
  if (!chosen) return;
  // OK takes the six codes; the page-navigation choices were kept as they were clicked.
  settings.shortcuts = { ...settings.shortcuts, ...chosen };
  store.set("shortcuts", JSON.stringify(settings.shortcuts));
}

// ------------------------------------------------------------------ Edit and Layout

// Messages for parts of the program that later build steps bring, as the desktop program gives them.
const PLANNED_SPELL = "The spell checker (Hunspell dictionaries) is built after the core features of version 1.";
const PLANNED_HELP = "The BrailleCraft help is written with the features it describes; it is not in this build yet.";
const PLANNED_TEMPLATES = "Templates are built with the BrailleCraft document format in step 7 of the plan.";
const PLANNED_PICTURES = "Pictures are built after the core features of version 1.";
const PLANNED_FORMATTER = "This needs the page formatter, built in step 8 of the plan.";

// Puts `text` in place of the selection as text (pasted, a file's, a sign): brackets in it are text, as typed ones
// are; the caret goes after it.
function insertText(text) {
  editor.focus();
  editor.setRangeText(text, editor.selectionStart, editor.selectionEnd, "end");
  editor.dispatchEvent(new Event("input"));
}

// Puts code text the program writes ("[p]", "[hds2]") in place of the selection, as the editor shows it (a [p] or
// [l] code with the editor's line break after it): its codes are codes. The caret goes after it.
function insertCodes(codes) {
  if (!core) return insertText(codes);
  const at = editor.selectionStart;
  insertText(core.marked_text(codes));
  core.editor_mark(at, codes);
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

// Puts codes `before` at the start of the selection and `after` at its end, each as its own edit so the selected
// text and its codes are left as they are; the same text stays selected between them (with no selection the caret
// goes between the two).
function wrapSelection(before, after) {
  editor.focus();
  const [from, to] = [editor.selectionStart, editor.selectionEnd];
  const [b, a] = core ? [core.marked_text(before), core.marked_text(after)] : [before, after];
  editor.setRangeText(a, to, to, "preserve");
  editor.dispatchEvent(new Event("input"));
  core?.editor_mark(to, after);
  editor.setRangeText(b, from, from, "preserve");
  editor.dispatchEvent(new Event("input"));
  core?.editor_mark(from, before);
  editor.setSelectionRange(from + b.length, to + b.length);
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

// Edit, Undo to Select All, as the text control carries them out.
async function editStandard(id) {
  editor.focus();
  const [from, to] = [editor.selectionStart, editor.selectionEnd];
  switch (id) {
    case 1201:
      document.execCommand("undo");
      break;
    case 1202:
      document.execCommand("redo");
      break;
    case 1203:
    case 1204:
      // The selection, codes and all, goes to the clipboard; its codes are kept, so pasting it brings them back.
      core?.editor_remember(from, to);
      try {
        await navigator.clipboard.writeText(editorText().slice(from, to));
      } catch {
        document.execCommand("copy");
      }
      if (id === 1203 && from !== to) {
        insertText("");
        // Cut ends Highlight.
        doc.highlight = null;
      }
      break;
    case 1205:
      try {
        insertText(await navigator.clipboard.readText());
      } catch {
        await message("The browser did not allow reading the clipboard; use Ctrl+V to paste.");
      }
      break;
    case 1206:
      // With no selection Clear is the Delete key: the character after the caret goes (a code
      // with it goes whole, as the editor takes codes out).
      if (from === to && to < editorText().length) {
        const next = editorText().codePointAt(to) > 0xffff ? 2 : 1;
        editor.setSelectionRange(to, to + next);
      }
      insertText("");
      break;
    case 1207:
      editor.select();
      break;
  }
  showStatus();
}

function toggleHighlight() {
  doc.highlight = doc.highlight === null || doc.highlight === undefined ? editor.selectionStart : null;
  // The Keys field shows highlighting ("H"), rewritten on this command.
  showStatus();
}

// Edit, Change Case: the text between the codes in the selection goes to capitals, or to small letters when it is
// all capitals already; the codes are left as they are.
// Edit, Change Case, as the desktop program (the core's change_case): the selection or the word at the caret; each
// letter that changes is replaced on its own so the codes stay; the caret goes to the start of the range.
function changeCase() {
  if (!core) return;
  const out = [...core.editor_change_case(editor.selectionStart, editor.selectionEnd)];
  if (!out.length) return;
  const start = out[0];
  for (let k = out.length - 2; k >= 2; k -= 2) {
    const c = String.fromCodePoint(out[k + 1]);
    const at = out[k];
    editor.setRangeText(c, at, at + (editorText().codePointAt(at) > 0xffff ? 2 : 1), "preserve");
  }
  editor.dispatchEvent(new Event("input"));
  editor.setSelectionRange(start, start);
}

// A text file's bytes as text, as the desktop program reads them: UTF-8 when they are valid UTF-8, else Windows
// code page 1252 (curly quotes, dashes, œ and the euro sign in 0x80 to 0x9F).
function decodeTextFile(bytes) {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder("windows-1252").decode(bytes);
  }
}

// Edit, Insert File: a document file with its codes, or any other file through the import dialog, put in at the
// caret; braille is refused in a print document and print in a braille one.
function insertFile() {
  const input = document.createElement("input");
  input.type = "file";
  input.addEventListener("change", async () => {
    const file = input.files[0];
    if (!file) return;
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (!core) {
      insertText(decodeTextFile(bytes));
      return;
    }
    let read;
    if (core.detect_file(file.name, bytes) === "Document") {
      try {
        const d = core.read_document(bytes);
        read = { braille: d.braille, running: d.running };
      } catch {
        await message("This document cannot be imported into BrailleCraft.  It appears to be in an unsupported file format.");
        return;
      }
    } else if (core.detect_file(file.name, bytes) === "Pef") {
      // A PEF file is inserted as braille with no import dialog, as File, Open opens it (BrailleCraft's addition).
      try {
        read = { braille: true, plain: core.pef_open(new TextDecoder().decode(bytes)) };
      } catch (err) {
        await message(String(err.message ?? err));
        return;
      }
    } else {
      const choice = await importDialog(core.detect_file(file.name, bytes));
      if (!choice) return;
      read = await readImport(bytes, choice);
      if (!read) return;
    }
    const braille = doc.kind === "braille";
    if (read.braille && !braille) {
      await message("You cannot insert a Braille document into a print document.");
      return;
    }
    if (!read.braille && braille) {
      await message("You cannot insert a print document into a Braille document.");
      return;
    }
    if (read.running !== undefined) {
      const text = core.running_text(read.running);
      const shown = braille ? fromNA(text) : text;
      const at = editor.selectionStart;
      insertText(shown);
      core.editor_mark_running(at, read.running, shown);
    } else {
      insertText(braille ? fromNA(read.plain) : read.plain);
    }
  });
  input.click();
}

// Edit, Goto (dialog 1019), as the desktop program does it: the fields start at the caret's page, line and column;
// page, line and column are checked in the dialog (not a number, below 1, above the page count limit, the lines per
// page or the cells per line), which stays open on a failure; "Use reference page number" (8) greys out 9 to 12 and
// goes to that print page number; a place with no cell asks 1004 and goes to the closest one.
async function gotoPosition() {
  if (!core) return;
  const braille = doc.kind === "braille";
  const text = editorText();
  const lines = Math.max(1, doc.lines);
  const cells = Math.max(1, doc.cells);
  const [page0, line0, column0] = Array.from(core.locate(text, doc.cells, doc.lines, braille, editor.selectionStart));
  const FIELDS = [
    [7, "You cannot go to pages with numbers less than 1.", "The page number is too great.", 0x7fffffff],
    [10, "You cannot go to lines with numbers less than 1.", "The line number is too great.", lines],
    [12, "You cannot go to columns with numbers less than 1.", "The column is too great.", cells],
  ];
  const { result, d } = await showDialog(1019, {
    setup(d) {
      d.setText(7, String(page0));
      d.setText(10, String(line0));
      d.setText(12, String(column0));
      d.setChecked(8, false);
      d.get(8)?.addEventListener("change", () => {
        for (let id = 9; id <= 12; id++) d.enable(id, !d.checked(8));
      });
    },
    async onOk(d) {
      for (const [field, low, high, max] of FIELDS) {
        const n = numberField(d.text(field));
        const problem = n === null ? "The number entered is not valid." : n < 1 ? low : n > max ? high : null;
        if (problem === null) continue;
        await message(problem);
        const el = d.get(field);
        el?.focus();
        el?.select?.();
        return false;
      }
      return true;
    },
  });
  if (result !== "ok") return;
  const [page, line, column, reference] = [numberField(d.text(7)), numberField(d.text(10)), numberField(d.text(12)), d.checked(8)];
  const go = (offset) => {
    editor.focus();
    editor.setSelectionRange(Number(offset), Number(offset));
    showStatus();
  };
  if (reference) {
    const at = Number(core.reference_page_position(text, doc.cells, doc.lines, braille, page));
    if (at < 0) await message(`Unable to go to reference page ${page}.`);
    else go(at);
    return;
  }
  const exact = Number(core.exact_position(text, doc.cells, doc.lines, braille, page, line, column));
  if (exact >= 0) {
    go(exact);
    return;
  }
  if (!(await yesNo("You cannot go to that location in the document.  Would you like to go to the closest location?"))) return;
  const closest = Number(core.closest_position(text, doc.cells, doc.lines, braille, page, line, column));
  if (closest < 0) await message("Unable to go to closest character.");
  else go(closest);
}

// Edit, Find, Find Again and Replace (bc_core find, spec 32; the desktop program's find and
// replace). The search is kept with the document.
const SEARCH_QUESTIONS = {
  rest: "BrailleCraft was unable to find the text within the selected area.  Would you like to search the rest of the document?",
  start: "BrailleCraft has finished searching to the end of the document.  Would you like to continue searching at the beginning?",
  end: "BrailleCraft has finished searching to the beginning of the document.  Would you like to continue searching at the end?",
};

// One search on the editor's text (core editor_find), its questions asked as they come. Answers the result: its
// spec, and the match with its places when found.
async function runSearch(spec, { begin = false, from = -1, step = 0, all = false } = {}) {
  const forward = editor.selectionDirection !== "backward";
  const caret = forward ? editor.selectionEnd : editor.selectionStart;
  const anchor = forward ? editor.selectionStart : editor.selectionEnd;
  let answers = "";
  for (;;) {
    const out = JSON.parse(core.editor_find(JSON.stringify(spec), begin, anchor, caret, from, step, doc.kind === "print", all, answers));
    if (!out.ask) return out;
    answers += (await yesNo(SEARCH_QUESTIONS[out.ask])) ? "y" : "n";
  }
}

// The search options of Find (case 8, backward 9, whole words 10, radios 12 to 14) and Replace (10, 11, 12, radios
// 14 to 16), filled from `s` and enabled: case only in a print document and not for a style name,
// whole words not for a style name; literal or wildcard enables both, style name disables both.
function searchOptions(d, s, caseId, wholeId, radios) {
  const print = doc.kind === "print";
  d.setChecked(caseId, !!s.match_case);
  d.setChecked(caseId + 1, !!s.backward);
  d.setChecked(wholeId, !!s.whole_words);
  d.setRadio(radios, s.stylename ? 1 : s.wildcard ? 2 : 0);
  d.enable(caseId, print && !s.stylename);
  d.enable(wholeId, !s.stylename);
  radios.forEach((id, k) =>
    d.get(id)?.addEventListener("change", () => {
      const style = k === 1;
      if (!style || print) d.enable(caseId, !style);
      d.enable(wholeId, !style);
    }),
  );
}

// The search the options say for `text` (running text); a `*` before a code's end becomes the wildcard mark when
// wildcards are chosen.
function readSearch(d, text, caseId, wholeId, radios) {
  const r = d.radio(radios) ?? 0;
  return {
    text: r === 2 ? text.replaceAll("*\u001f", "\u001b\u001f") : text,
    match_case: d.checked(caseId),
    backward: d.checked(caseId + 1),
    whole_words: d.checked(wholeId),
    stylename: r === 1,
    wildcard: r === 2,
  };
}

const fieldRefused = async (d, field, problem) => {
  if (!problem) return false;
  await message(problem);
  d.get(field)?.focus();
  d.get(field)?.select?.();
  return true;
};

// A match found by Find or Find Again: the caret at its start, past the start code for a style name.
function showFound(out) {
  const at = out.spec.style ? out.after : out.from;
  editor.focus();
  editor.setSelectionRange(at, at);
  showStatus();
}

async function find(again) {
  if (!core) return;
  const stored = doc.find ?? {};
  if (again && stored.text) {
    // Find Again: one unit on from the caret (past a hyphen code the text has none of); the state is not kept back.
    const forward = editor.selectionDirection !== "backward";
    const caret = forward ? editor.selectionEnd : editor.selectionStart;
    const out = await runSearch(stored, { from: core.editor_find_again_from(JSON.stringify(stored), caret) });
    if (out.match) showFound(out);
    else await message("The text does not appear again in the document.");
    return;
  }
  const codes = !!doc.view.codes;
  const { result, d } = await showDialog(1020, {
    setup(d) {
      d.setText(7, core.find_running_to_field(stored.text ?? "", codes));
      searchOptions(d, stored, 8, 10, [12, 13, 14]);
      // Six-key entry works in the field to find.
      sixKeyField(d.get(7));
    },
    onOk: async (d) => !(await fieldRefused(d, 7, core.find_field_problem(d.text(7), false))),
  });
  if (result !== "ok") return;
  const s = readSearch(d, core.find_field_to_running(d.text(7)), 8, 10, [12, 13, 14]);
  // A style name searches for the style's start code; one whole style code typed is searched as it is.
  s.style = s.stylename && !core.find_style_code_name(s.text);
  const out = await runSearch(s, { begin: true });
  doc.find = out.spec;
  if (out.match) showFound(out);
  else await message("Text was not found in the document.");
}


// The Replace dialog's texts as running text and whether it is a style search: with style name chosen, a whole style
// code typed is taken as its bare name; a style search when both are names or both are codes.
function replaceTexts(d) {
  let f = core.find_field_to_running(d.text(7));
  let w = core.find_field_to_running(d.text(9));
  const stylename = d.radio([14, 15, 16]) === 1;
  if (stylename) {
    const name = core.find_style_code_name(f);
    if (name) {
      const wn = core.find_style_code_name(w);
      if (!w) f = name;
      else if (wn) [f, w] = [name, wn];
    }
  }
  const style = stylename && !!core.find_style_code_name(f) === !!core.find_style_code_name(w);
  return { f, w, style };
}

async function replace() {
  if (!core) return;
  const stored = doc.replace?.text ? doc.replace : {};
  const storedWith = stored.text ? (doc.replaceWith ?? "") : "";
  const codes = !!doc.view.codes;
  const names = await styleNames();
  let proceed = true;
  const { result, d } = await showDialog(1021, {
    setup(d) {
      d.setText(7, core.find_running_to_field(stored.text ?? "", codes));
      d.setText(9, core.find_running_to_field(storedWith, codes));
      searchOptions(d, stored, 10, 12, [14, 15, 16]);
    },
    async onOk(d) {
      if (await fieldRefused(d, 7, core.find_field_problem(d.text(7), false))) return false;
      if (await fieldRefused(d, 9, core.find_field_problem(d.text(9), true))) return false;
      const { f, w, style } = replaceTexts(d);
      if (!style) return true;
      const known = (n) => names.some((s) => s.toLowerCase() === n.toLowerCase());
      // No stays in the dialog, Yes goes on, Cancel closes it with nothing replaced (the choices are kept).
      const ask = async (name) => {
        const a = await yesNoCancel(`There is no style defined with the name "${name}".  Are you sure you want to replace all occurrences of the style "${f}" with "${w}"?`);
        if (a === null) proceed = false;
        return a !== false;
      };
      if (!known(f)) return ask(f);
      if (![...w].every((c) => /[A-Za-z0-9._-]/.test(c)))
        return !(await fieldRefused(d, 9, "The style name contains invalid characters.  Style names may only contain letters (uppercase and lowercase), numbers, the period, the hyphen, and the underscore."));
      if (w && !known(w)) return ask(w);
      return true;
    },
  });
  if (result !== "ok") return;
  const { f, w, style } = replaceTexts(d);
  const s = readSearch(d, f, 10, 12, [14, 15, 16]);
  s.style = style;
  const withText = s.wildcard ? w.replaceAll("*\u001f", "\u001b\u001f") : w;
  doc.replace = s;
  doc.replaceWith = withText;
  if (proceed) await runReplace(s, withText);
}

// The replacing (spec 32 3): each match shown selected with Confirm Replacement (1024): Yes replaces it and goes on,
// No goes on, All replaces it and every later match, Stop ends at it. Then the caret and selection as they were,
// moved by the replacements before them, and the count.
async function runReplace(spec, withText) {
  const braille = doc.kind === "braille";
  const hadSel = editor.selectionStart !== editor.selectionEnd;
  let count = 0;
  let stopped = false;
  const replaceOne = (out) => {
    const r = JSON.parse(core.editor_replace(JSON.stringify(out.spec), JSON.stringify(out.match), withText, braille));
    setEditorText(r.text);
    doc.dirty = true;
    count += 1;
    return r;
  };
  let out = await runSearch(spec, { begin: true });
  if (!out.match) await message("Text was not found in the document.");
  while (out.match) {
    editor.focus();
    editor.setSelectionRange(out.from, out.to);
    showStatus();
    const answer = (await showDialog(1024, { closes: [5, 6, 7] })).result;
    if (answer === "5") {
      const r = replaceOne(out);
      out = await runSearch(r.spec, { from: r.caret });
    } else if (answer === "6") {
      out = await runSearch(out.spec, { from: out.match[0], step: out.spec.backward ? -1 : 1 });
    } else if (answer === "7") {
      while (out.match) {
        const r = replaceOne(out);
        out = await runSearch(r.spec, { from: r.caret, all: true });
      }
    } else {
      stopped = true;
      out.spec.caret = out.match[0];
      break;
    }
  }
  const s = out.spec;
  const caret = core.editor_running_place(s.caret);
  if (hadSel && !stopped) {
    const anchor = core.editor_running_place(s.anchor);
    editor.setSelectionRange(Math.min(anchor, caret), Math.max(anchor, caret), anchor <= caret ? "forward" : "backward");
  } else editor.setSelectionRange(caret, caret);
  editor.focus();
  lastLine = null;
  showStatus();
  showTranslatedLine();
  await replacementsMessage(count);
}

// "%d replacements were made." in the notice 1054, not shown again once its box was checked (Mutable, Replacements:
// the answer plus one).
async function replacementsMessage(count) {
  if (Number(store.get("Mutable.Replacements")) > 0) return;
  const { result, d } = await showDialog(1054, { title: "BrailleCraft", setup: (d) => d.setText(5, `${count} replacements were made.`) });
  if (d.checked(6)) store.set("Mutable.Replacements", String((result === "ok" ? 1 : 2) + 1));
}

// The document's styles: those of its file, else those of the template it was made from.
async function styleNames() {
  return core ? (await documentFile()).style_names : [];
}

// A style dialog's list `list` and Show choice `show` (bc_core::doctext::style_list): the choices in their order with
// "Recommended and In-Use Styles" chosen; the list holds the styles the choice lets through in list order, after
// `lead` (Add Style's "(No Style)"), `want` chosen (case ignored) when listed, else the first item. A change of the
// choice fills the list again, keeping the chosen name when it is still listed. OK is enabled only while the list
// holds an item.
function styleShow(d, file, list, show, lead = null, want = null) {
  if (!core) return;
  d.setItems(show, core.style_show_choices(), core.style_show_default());
  fillStyleList(d, file, list, show, lead, want, true);
  d.get(show)?.addEventListener("change", () => fillStyleList(d, file, list, show, lead, d.selected(list), false));
}
function fillStyleList(d, file, list, show, lead, want, fold) {
  const choice = d.get(show)?.selectedIndex ?? core.style_show_default();
  const items = [...(lead ? [lead] : []), ...file.style_list(Math.max(choice, 0))];
  const at = want === null ? -1 : items.findIndex((n) => (fold ? n.toLowerCase() === want.toLowerCase() : n === want));
  d.setItems(list, items, items.length ? Math.max(at, 0) : -1);
  d.enable(1, items.length > 0);
}

// Layout, Apply Style (dialog 1046): the chosen style's start and end codes around the selection. The dialog opens
// on the style last applied in this document; with no style listed OK is disabled; a double click in the list is OK.
async function applyStyle() {
  if (!core) return;
  const file = await documentFile();
  const { result, d } = await showDialog(1046, {
    setup(d) {
      styleShow(d, file, 7, 9, null, doc.lastApplied || null);
      const list = d.get(7);
      list?.addEventListener("dblclick", () => list.form?.requestSubmit());
    },
  });
  if (result !== "ok" || !d.selected(7)) return;
  doc.lastApplied = d.selected(7);
  applyNamedStyle(d.selected(7));
}

// The style commands (the core's style_apply, spec 31) on the editor: "apply" (a style by name),
// "join-lines", "join-styles", "new-paragraph"; the selection's anchor and the caret are passed, the new text and the
// caret come back. Nothing happens when the core answers nothing (Join with no selection).
function styleCommand(what, name = "") {
  if (!core) return;
  const forward = editor.selectionDirection !== "backward";
  const caret = forward ? editor.selectionEnd : editor.selectionStart;
  const anchor = forward ? editor.selectionStart : editor.selectionEnd;
  const out = core.editor_style_command(what, name, anchor, caret, doc.cells, doc.lines, doc.kind === "braille");
  if (!out) return;
  const { text, caret: at } = JSON.parse(out);
  setEditorText(text);
  editor.setSelectionRange(at, at);
  doc.dirty = true;
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

// The beep (a failure that shows no message), through the browser's audio.
function beep() {
  try {
    const ctx = new AudioContext();
    const tone = ctx.createOscillator();
    tone.frequency.value = 800;
    tone.connect(ctx.destination);
    tone.start();
    tone.stop(ctx.currentTime + 0.1);
  } catch {
    // No audio: nothing to hear.
  }
}

// Layout, Style Level + and - (spec 34): inside a table the caret moves to the next or previous cell of its row;
// else the level code of the paragraph style at the caret, and every run of level codes in the selection, move by
// `step`. A failure beeps.
function styleLevel(step) {
  if (!core) return;
  const forward = editor.selectionDirection !== "backward";
  const caret = forward ? editor.selectionEnd : editor.selectionStart;
  const anchor = forward ? editor.selectionStart : editor.selectionEnd;
  const r = JSON.parse(core.editor_style_level(step, anchor, caret, doc.cells, doc.lines, doc.kind === "braille"));
  if (r.beep) beep();
  if (r.text === undefined) {
    if (r.caret !== undefined) editor.setSelectionRange(r.caret, r.caret);
    return;
  }
  if (!r.changed) return;
  setEditorText(r.text);
  editor.setSelectionRange(Math.min(r.anchor, r.caret), Math.max(r.anchor, r.caret), r.caret < r.anchor ? "backward" : "forward");
  doc.dirty = true;
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

function applyNamedStyle(name) {
  styleCommand("apply", name);
}

// Layout, Apply Last Style: the paragraph style ended last before the caret or the selection start, read from the
// text; nothing happens when there is none.
function applyLastStyle() {
  const name = core.editor_last_paragraph_style(Math.min(editor.selectionStart, editor.selectionEnd));
  if (name) applyNamedStyle(name);
}

// Document, Add Style (dialog 1025): a new style, its codes typed or taken from the style for basis ("(No Style)"
// first, then the styles the Show choice lists; item k takes the style before, in the whole sorted list, the style
// listed at k: bc_core::doctext::basis_style). A name the document has (case ignored) asks 'Replace existing
// "NAME"?': Yes puts the new style in place of the old one, No gives message 2170.
async function addStyle() {
  const file = await documentFile();
  const bypass = store.get("BypassStyleNameValidityCheck") === "1";
  // The field is checked in the dialog; on a refusal the message is shown and the field selected again.
  const refuse = async (d, field, problem) => {
    if (!problem) return true;
    await message(problem);
    d.get(field)?.focus();
    d.get(field)?.select?.();
    return false;
  };
  const { result, d } = await showDialog(1025, {
    setup(d) {
      d.setText(7, "New Style Name");
      styleShow(d, file, 10, 12, "(No Style)");
      // "Style can be nested" starts checked for a name without the period.
      d.setChecked(8, true);
      // Choosing a style for basis puts its codes in the code fields.
      d.get(10)?.addEventListener("change", () => {
        const k = Math.max(d.get(10)?.selectedIndex ?? 0, 0);
        const basis = file.basis_style(k, Math.max(d.get(12)?.selectedIndex ?? 0, 0));
        const codes = basis ? file.style_codes(basis) : "";
        const [b, e] = codes ? JSON.parse(codes) : ["", ""];
        d.setText(14, b);
        d.setText(16, e);
      });
    },
    onOk: (d) => refuse(d, 7, core.style_name_problem(d.text(7), bypass)),
  });
  if (result !== "ok") return;
  // A style that cannot be nested is one whose name ends in a period.
  let name = d.text(7);
  if (!d.checked(8) && !name.endsWith(".")) name += ".";
  // A name the period makes 32 characters long runs into the beginning codes, which Edit Style Codes then shows
  // empty.
  const begin = core.style_name_overruns_codes(name) ? "" : d.text(14);
  // Then Edit Style Codes (1044).
  const codes = await showDialog(1044, {
    setup(e) {
      e.setText(6, `Edit codes for style: ${name}`);
      e.setText(9, begin);
      e.setText(11, d.text(16));
    },
    async onOk(e) {
      for (const f of [9, 11]) if (!(await refuse(e, f, core.style_codes_problem(e.text(f))))) return false;
      return true;
    },
  });
  if (codes.result !== "ok") return;
  if (file.has_style(name) && !(await yesNo(`Replace existing "${name}"?`))) {
    await message("Unable to add style to style list.");
    return;
  }
  file.add_style(name, "", codes.d.text(9), codes.d.text(11));
  doc.dirty = true;
  lastLine = null;
  showTranslatedLine();
}

// Document, Modify Style (dialog 1026): a style's codes; Rename (dialog 1027) renames it, and its codes in the text
// when "everywhere" is checked.
async function modifyStyle() {
  const file = await documentFile();
  if (!file.style_names.length) {
    await message("This document has no styles yet. Use Document, Add Style to create one.");
    return;
  }
  let shownCodes = ["", ""];
  const load = (d) => {
    const codes = file.style_codes(d.selected(8) ?? "");
    if (!codes) return;
    shownCodes = JSON.parse(codes);
    d.setText(12, shownCodes[0]);
    d.setText(14, shownCodes[1]);
    // The code fields come disabled in the dialog resource; with a style chosen they can be edited.
    for (let c = 11; c <= 14; c++) d.enable(c, true);
  };
  const { result, d } = await showDialog(1026, {
    setup(d) {
      styleShow(d, file, 8, 10);
      load(d);
      d.get(8)?.addEventListener("change", () => load(d));
      d.get(6)?.addEventListener("click", async () => {
        const old = d.selected(8);
        if (!old) return;
        const r = await showDialog(1027, {
          setup(r) {
            r.setText(7, old);
            r.setChecked(8, true);
          },
          // Checked in the dialog: an empty name does nothing, a name the document has (in any
          // case) and invalid characters are refused with their messages.
          async onOk(r) {
            const name = [...r.text(7)].slice(0, 31).join("");
            if (!name) return false;
            let problem = null;
            if (file.style_names.some((n) => n.toLowerCase() === name.toLowerCase())) problem = `A style named ${name} already exists.`;
            else if (!/^[A-Za-z0-9._-]+$/.test(name))
              problem = "The style name contains invalid characters.  Style names may only contain letters (uppercase and lowercase), numbers, the period, the hyphen, and the underscore.";
            if (!problem) return true;
            await message(problem);
            r.get(7)?.focus();
            r.get(7)?.select?.();
            return false;
          },
        });
        const name = r.d.text(7).trim();
        if (r.result !== "ok" || !name) return;
        file.rename_style(old, name);
        if (r.d.checked(8)) {
          // The style's codes are renamed in the text.
          const [from, to] = [editor.selectionStart, editor.selectionEnd];
          // The style's codes are renamed where codes are; text that only reads like the code is left as it is.
          setEditorText(core.editor_rename_style(old, name));
          editor.setSelectionRange(Math.min(from, editorText().length), Math.min(to, editorText().length));
        }
        doc.dirty = true;
        // The list is filled again in its order, the renamed style chosen where it now stands.
        fillStyleList(d, file, 8, 10, null, name, false);
        load(d);
        d.get(8)?.focus();
      });
    },
  });
  if (result !== "ok" || !d.selected(8)) return;
  // Changed codes are asked about first (1002).
  const changed = d.text(12) !== shownCodes[0] || d.text(14) !== shownCodes[1];
  if (changed && !(await yesNo(`Are you sure you want to change the codes associated with style "${d.selected(8)}"?`))) return;
  file.set_style_codes(d.selected(8), d.text(12), d.text(14));
  if (changed) doc.dirty = true;
  lastLine = null;
  showTranslatedLine();
}

// Document, Delete Style (dialog 1047): a document keeps at least one style (2172), and the style
// chosen leaves the document only after Yes to 1026.
async function deleteStyle() {
  if (!core) return;
  const file = await documentFile();
  const { result, d } = await showDialog(1047, {
    setup(d) {
      styleShow(d, file, 7, 9);
    },
  });
  if (result !== "ok" || !d.selected(7)) return;
  if (file.style_names.length < 2) {
    await message("There must always be at least one style.");
    return;
  }
  if (!(await yesNo(`Are you sure you want to remove style "${d.selected(7)}"?`))) return;
  file.remove_style(d.selected(7));
  doc.dirty = true;
  lastLine = null;
  showTranslatedLine();
}

// Document, Page Numbering (dialog 1031): braille page numbers on even and on odd pages, print page numbers on even
// and on odd pages, each none, upper left, upper right, lower left or lower right; then the first page number.
async function pageNumbering() {
  const GROUPS = [
    [7, 8, 9, 10, 11],
    [13, 14, 15, 16, 17],
    [19, 20, 21, 22, 23],
    [25, 26, 27, 28, 29],
  ];
  // The first page field is read as its first five characters.
  const WIDTH = 5;
  const file = await documentFile();
  const numbers = [...file.page_numbers];
  // A corner one group has chosen is greyed in its partner group (even braille with even print, odd braille with
  // odd print), redone on every click; None is never greyed.
  const sync = (d) => {
    const chosen = GROUPS.map((g) => d.radio(g) ?? 0);
    const off = JSON.parse(core.page_number_disabled(Uint32Array.from(chosen)));
    GROUPS.forEach((g, k) => g.forEach((id, c) => d.enable(id, !off[k][c])));
  };
  const { result, d } = await showDialog(1031, {
    third: true,
    // Focus starts on the chosen corner of even braille pages.
    focus: GROUPS[0][numbers[0]] ?? GROUPS[0][0],
    setup(d) {
      GROUPS.forEach((g, k) => d.setRadio(g, numbers[k]));
      sync(d);
      for (const id of GROUPS.flat()) d.get(id)?.addEventListener("change", () => sync(d));
      d.setText(31, String(numbers[4]));
      d.get(31)?.select?.();
    },
    onOk: (d) =>
      checkNumber(
        d,
        31,
        "The first page to display page numbers must not be less than %d.",
        "The first page to display page numbers must not be greater than %d.",
        WIDTH,
      ),
  });
  if (result !== "ok") return;
  const chosen = GROUPS.map((g, k) => d.radio(g) ?? numbers[k]);
  chosen.push(numberField(d.text(31), WIDTH));
  // An accepted OK marks the document changed whether or not a value changed (there are no write-protected
  // documents).
  file.set_page_numbers(Uint32Array.from(chosen));
  doc.dirty = true;
  showStatus();
}

// Document, Create Template (dialog 1028): templates are built with the document format (as in the desktop program).
async function createTemplate() {
  const { result } = await showDialog(1028, {
    setup(d) {
      d.setItems(10, ["English (UEB) - Standard", "English (UEB) - Textbook Format"]);
      d.setRadio([19, 20], 0);
      d.get(18)?.addEventListener("click", () => message(PLANNED_TEMPLATES));
    },
  });
  if (result === "ok") await message(PLANNED_TEMPLATES);
}

// Table, Create Table and Table Properties (dialog 1062): the table settings, kept as the dialog's memory; Create
// Table puts the table codes around the selection.
// Table, Create and Table Properties, one command (bc_core table_cmd, spec 33; the desktop
// program's table_command): with the caret in a table it edits that table's start code, else it puts a new table at
// the caret; a selection holding a table start or end code is refused.
async function tableProperties() {
  if (!core) return;
  const forward = editor.selectionDirection !== "backward";
  const caret = forward ? editor.selectionEnd : editor.selectionStart;
  const anchor = forward ? editor.selectionStart : editor.selectionEnd;
  const info = JSON.parse(core.editor_table_info(anchor, caret));
  if (info.refused) {
    await message("You have a selection that spans across a table boundary.  Please place the cursor in a table to set table properties or outside of all tables to create a new table.");
    return;
  }
  const t = info.table;
  let swapChanged = -1;
  const { result, d } = await showDialog(1062, {
    setup(d) {
      d.setItems(7, info.types, info.type_index);
      d.setItems(9, info.swaps, info.swap_index);
      d.setText(11, String(t.rows));
      d.setText(13, String(t.header_rows));
      d.setText(15, String(t.cols));
      d.setText(17, String(t.header_cols));
      d.enable(11, info.create);
      d.enable(15, info.create);
      // Header rows and columns only for a table (not Unrelated Columns, not Matrix).
      const headers = () => {
        const on = (d.get(7)?.selectedIndex ?? 0) > 1;
        d.enable(13, on);
        d.enable(17, on);
      };
      headers();
      d.get(7)?.addEventListener("change", headers);
      // The swap letter changes only when the list is changed; untouched, the table keeps its own.
      d.get(9)?.addEventListener("change", () => (swapChanged = d.get(9).selectedIndex));
    },
  });
  if (result !== "ok") return;
  const typeIndex = d.get(7)?.selectedIndex ?? info.type_index;
  const out = core.editor_table_apply(JSON.stringify(t), Math.max(typeIndex, 0), swapChanged, d.text(11), d.text(13), d.text(15), d.text(17), anchor, caret);
  if (!out) return;
  const r = JSON.parse(out);
  setEditorText(r.text);
  editor.focus();
  if (info.create) editor.setSelectionRange(r.caret, r.caret);
  else editor.setSelectionRange(Math.min(r.anchor, r.caret), Math.max(r.anchor, r.caret), r.anchor <= r.caret ? "forward" : "backward");
  doc.dirty = true;
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

// "Document "name" not saved!  Save changes?": Yes saves (Save), No goes on, Cancel stops. Answers whether to go on.
async function confirmDiscard() {
  if (!doc.dirty) return true;
  const answer = await yesNoCancel(`Document "${doc.name}" not saved!  Save changes?`);
  if (answer === null) return false;
  if (answer) {
    await save(false);
    return !doc.dirty;
  }
  return true;
}

// File, Close: the web build holds one document, so a closed document gives way to a new empty one.
async function closeDocument() {
  if (await confirmDiscard()) await newDocument(false, null);
}

// File, Exit: after the save question, the document is let go of and the page is closed where the browser allows
// it; else the browser's tab closes BrailleCraft.
async function exitProgram() {
  if (!(await confirmDiscard())) return;
  await letGo();
  doc.dirty = false;
  window.close();
  await message("To leave BrailleCraft, close its browser tab or window.");
}

// Layout, Join Lines, Join Styles and New Paragraph, as the desktop program (spec 31).
const joinLines = () => styleCommand("join-lines");
const joinStyles = () => styleCommand("join-styles");
const newParagraph = () => styleCommand("new-paragraph");

// Layout, Page Number (dialog 1042): the page numbering style and number codes.
async function setPageNumber() {
  const { result, d } = await showDialog(1042, {
    setup(d) {
      d.setText(7, "1");
      d.setRadio([9, 10], 0);
    },
    onOk: (d) => checkNumber(d, 7, "The page number may not be set lower than %d.", "The page number may not be set to be greater than %d."),
  });
  if (result === "ok") insertCodes(`[${d.radio([9, 10]) === 1 ? "pntr" : "pnta"}][pv${d.number(7)}]`);
}

// Layout, Header and Footer (dialogs 1029, 1030): All Pages, Odd Pages, Even Pages write the
// placement code 3, 1, 2 ([svrhp3]) before the start code, the text and the end code; No Header writes the code
// that discontinues it and greys out the text. The text is checked in the dialog: 1 to 80 characters.
const HEADER = { rid: 1029, start: "[tls]", end: "[tle]", none: "[tld]", placement: "svrhp", empty: "The header text is empty", tooLong: "The header text is too long.  It must not be greater than %d characters." };
const FOOTER = { rid: 1030, start: "[rfs]", end: "[rfe]", none: "[rfd]", placement: "svrfp", empty: "The footer text is empty.", tooLong: "The footer text is too long.  It must not be greater than %d characters." };

async function headerFooter(hf) {
  const RADIOS = [9, 10, 11, 12];
  const PLACEMENT = [3, 1, 2, 0];
  const LONGEST = 80;
  const { result, d } = await showDialog(hf.rid, {
    setup(d) {
      d.setRadio(RADIOS, 0);
      // Six-key entry works in the header or footer field.
      sixKeyField(d.get(7));
      RADIOS.forEach((id, k) =>
        d.get(id)?.addEventListener("change", () => {
          d.enable(6, k !== 3);
          d.enable(7, k !== 3);
        }),
      );
    },
    async onOk(d) {
      if (d.radio(RADIOS) === 3) return true;
      const n = [...d.text(7)].length;
      const text = n === 0 ? hf.empty : n > LONGEST ? hf.tooLong.replace("%d", String(LONGEST)) : null;
      if (text === null) return true;
      await message(text);
      d.get(7)?.focus();
      d.get(7)?.select?.();
      return false;
    },
  });
  if (result !== "ok") return;
  // The codes are codes; the header or footer text typed is text.
  const place = PLACEMENT[d.radio(RADIOS) ?? 0];
  if (place === 0) {
    insertCodes(hf.none);
  } else {
    insertCodes(`[${hf.placement}${place}]${hf.start}`);
    insertText(d.text(7));
    insertCodes(hf.end);
  }
}

// Layout, Code List (dialog 1041): the category boxes (All, then Page, Line, Tab, European, Greek
// Letters, CBC, Math, Miscellaneous) choose the codes listed and are kept for the session from OK or a double-click,
// before the parameters are checked; a double-click is OK; parameters with a space are refused in the dialog.
// INFERENCE: the boxes at the start (All, Page, Line and Tab) are not known.
const CODE_BOXES = [9, 10, 11, 12, 13, 14, 15, 16];
let codeListKept = { all: true, mask: 7 };
async function codeList() {
  const list = JSON.parse(core.code_list());
  let shown = [];
  let insert = null;
  const boxes = (d) => ({ all: d.checked(8), mask: CODE_BOXES.reduce((m, b, k) => (d.checked(b) ? m | (1 << k) : m), 0) });
  // A box click refills the list; the code shown in 20 stays until another code is chosen.
  const fill = (d) => {
    const { all, mask } = boxes(d);
    shown = [...core.code_list_shown(all, mask)].map((i) => list[i]);
    d.setItems(18, shown.map(([, , label]) => label), null);
  };
  // OK and a double-click: the boxes are kept first, then the parameters checked (bc_core::menu_codes). With a
  // code chosen, a space in them is refused with the dialog left open; with none chosen the dialog closes first and
  // the parameters alone go in as text.
  const accept = async (d) => {
    codeListKept = boxes(d);
    const chosen = shown[d.get(18)?.selectedIndex ?? -1];
    const outcome = JSON.parse(core.code_list_ok(chosen ? chosen[0] : "", d.text(22)));
    if (outcome.close) insert = outcome.insert;
    if (outcome.message) await message(outcome.message);
    if (!outcome.close) {
      d.get(22)?.focus();
      d.get(22)?.select?.();
    }
    return outcome.close;
  };
  const { result } = await showDialog(1041, {
    third: true,
    // Focus starts in the list.
    focus: 18,
    setup(d) {
      d.setChecked(8, codeListKept.all);
      CODE_BOXES.forEach((b, k) => d.setChecked(b, (codeListKept.mask & (1 << k)) !== 0));
      fill(d);
      d.setText(20, "");
      d.setText(22, "");
      for (const b of [8, ...CODE_BOXES]) d.get(b)?.addEventListener("change", () => fill(d));
      const listBox = d.get(18);
      listBox?.addEventListener("change", () => d.setText(20, shown[listBox.selectedIndex] ? `[${shown[listBox.selectedIndex][0]}]` : ""));
      listBox?.addEventListener("dblclick", () => listBox.form?.requestSubmit());
      // Help keeps the dialog open.
      d.get(6)?.addEventListener("click", () => message(PLANNED_HELP));
    },
    onOk: accept,
  });
  if (result !== "ok" || !insert) return;
  const [text, isCode] = insert;
  if (isCode) insertCodes(text);
  else insertText(text);
}

// Document, Hyphen Table: the document's table; a change lays the document out again and, as in the standard
// program, does not mark the document as changed (spec 36 section 12).
function setHyphen(id) {
  doc.hyphen = id;
  refreshMenus();
  syncHyphenation();
}

// Gives the core the active document's hyphenation table (Document, Hyphen Table; French in a print document
// takes Dashes and Hyphens), each table file read once from tables/hyphenation beside the page.
const hyphenTables = new Map();
// The table text the core holds now: it is given again only when the choice changes (the status line refresh calls
// here on every change, and this is what refreshes it after a table is fetched).
let hyphenGiven = null;
async function syncHyphenation() {
  if (!core) return;
  const d = doc;
  const file = core.hyphenation_file((d.hyphen ?? 2902) - 2901, d.kind !== "braille");
  if (!hyphenTables.has(file)) {
    try {
      hyphenTables.set(file, await (await fetch(`tables/hyphenation/${file}.hct`)).text());
    } catch {
      hyphenTables.set(file, "");
    }
  }
  if (d !== doc) return;
  const text = hyphenTables.get(file);
  if (text === hyphenGiven) return;
  hyphenGiven = text;
  core.set_hyphenation(text);
  showStatus();
}

// ------------------------------------------------------------------ Document, Translation and Learning Tables

// The region Document, Translation Tables lists (a region bit, or all); Western Hemisphere at first, kept in the
// browser as the desktop program keeps it.
const region = () => Number(store.get("translation.region") ?? 1) >>> 0;
const LANGUAGE_BASE = 9000;
// Every course of the language gets an item, one id each from 9500 (the ids up to 9999 are free for them).
const COURSE_BASE = 9500;
const COURSE_MAX = 500;

// The menu items the program puts in a submenu at run time: removed, then `entries` put in before `before` (at
// the end when null).
function setDynamicItems(menu, entries, before, menuLabel) {
  menu.querySelectorAll(":scope > li[data-dynamic]").forEach((li) => li.remove());
  for (const entry of entries) {
    const li = itemNode(entry, menuLabel, 2);
    li.dataset.dynamic = "1";
    menu.insertBefore(li, before);
  }
}

// Document, Translation Tables: the region marked, and after the regions the languages listed for it (and the
// document's own), the document's checked. Document, Learning Tables: its language's courses between Uncontracted
// and Fully Contracted, the one in use (or a grade) checked; the submenu is available only when the language has
// learning steps.
async function refreshTableMenus() {
  const list = await languages();
  const regionItem = menubar.querySelector('[data-id="2701"]');
  const learningItem = menubar.querySelector('[data-id="2801"]');
  if (!list || !regionItem || !learningItem) return;
  const r = region();
  // The sixth region item's bit is all regions.
  for (let i = 0; i < 6; i++) {
    menubar.querySelector(`[data-id="${2701 + i}"]`)?.setAttribute("aria-checked", String(core.LanguageList.region_bit(i) === r));
  }
  const file = await documentFile();
  const m = JSON.parse(list.menus(r, file.tables, file.initial_codes));
  const tablesMenu = regionItem.closest("[role=menu]");
  setDynamicItems(
    tablesMenu,
    m.languages.length ? [{ separator: true }, ...m.languages.map(([entry, name]) => ({ id: LANGUAGE_BASE + entry, label: name.replaceAll("&", "&&"), kind: "check" }))] : [],
    null,
    "Translation Tables",
  );
  if (m.active !== null) menubar.querySelector(`[data-id="${LANGUAGE_BASE + m.active}"]`)?.setAttribute("aria-checked", "true");
  const learningMenu = learningItem.closest("[role=menu]");
  const courses = m.courses.slice(0, COURSE_MAX).map((name, i) => ({ id: COURSE_BASE + i, label: `${name.replaceAll("&", "&&")}...`, kind: "check" }));
  // Between Uncontracted and Fully Contracted: put in before Fully Contracted, which the program never removes.
  const fullyContracted = menubar.querySelector('[data-id="2802"]').parentElement;
  setDynamicItems(learningMenu, courses.length ? [{ separator: true }, ...courses, { separator: true }] : [], fullyContracted, "Learning Tables");
  const checked = typeof m.checked === "number" && m.checked < COURSE_MAX ? COURSE_BASE + m.checked : m.checked;
  for (const item of learningMenu.querySelectorAll("[data-id]")) item.setAttribute("aria-checked", String(Number(item.dataset.id) === checked));
  // The Learning Tables item itself is available only when the language has learning steps.
  const parent = learningMenu.parentElement.querySelector(":scope > [role^=menuitem]");
  if (m.hasSteps) parent.removeAttribute("aria-disabled");
  else parent.setAttribute("aria-disabled", "true");
}

// Gives the document `tables` (when given) and initial codes changed by `initial`; the translated line follows.
async function setDocumentTables(tables, initial) {
  const file = await documentFile();
  if (tables) file.set_tables(tables);
  file.set_initial_codes(initial(file.initial_codes));
  doc.dirty = true;
  lastLine = null;
  await refreshTableMenus();
  showTranslatedLine();
}

// Document, Translation Tables, a region: the languages listed are that region's (and the document's own).
async function setRegion(item) {
  store.set("translation.region", String(core.LanguageList.region_bit(item)));
  await refreshTableMenus();
}

// Document, Translation Tables, a language: the document takes that entry's four tables, and every grade code
// leaves its initial codes.
async function chooseLanguage(entry) {
  const list = await languages();
  if (!list) return;
  await setDocumentTables(list.tables(entry), (i) => core.regrade(i, ""));
}

// Document, Learning Tables, Uncontracted or Fully Contracted: the document's language gives its four tables back
// (a learning step in use ends), and the grade code in the initial codes changes to `code`.
async function setGrade(code) {
  const list = await languages();
  if (!list) return;
  const file = await documentFile();
  const m = JSON.parse(list.menus(region(), file.tables, file.initial_codes));
  await setDocumentTables(m.active !== null ? list.tables(m.active) : null, (i) => core.regrade(i, code));
}

// Document, Learning Tables, a learning course: dialog 1052 lists its steps ("Choose a contraction level:"), the step
// in use chosen; OK gives the chosen step's table in place of the print-to-braille rules table, the language's
// other three tables staying, and takes every grade code out of the initial codes. A double-click in the list does
// nothing.
async function chooseLearningStep(course) {
  const list = await languages();
  if (!list) return;
  const file = await documentFile();
  const s = list.steps(file.tables, course);
  if (!s) return;
  const steps = JSON.parse(s);
  const { result, d } = await showDialog(1052, {
    third: true,
    focus: 7,
    setup(d) {
      d.setItems(7, steps.names, steps.selected);
    },
  });
  if (result !== "ok") return;
  const i = Math.max(d.get(7)?.selectedIndex ?? 0, 0);
  const tables = [...steps.language];
  tables[0] = steps.tables[i];
  await setDocumentTables(tables, (codes) => core.regrade(codes, ""));
}

// A Layout menu code item: its codes put in, or put around the selection.
// Puts `codes` before every line start inside the selection (a place after a line break, before its end) and at its
// start, from the last place back; with no selection at the caret.
function insertEachLine(codes) {
  const [from, to] = [editor.selectionStart, editor.selectionEnd];
  if (from === to) return insertCodes(codes);
  editor.focus();
  const text = editor.value;
  const places = [];
  for (let p = to - 1; p > from; p--) if (text[p - 1] === "\n") places.push(p);
  places.push(from);
  const shown = core.marked_text(codes);
  for (const p of places) {
    editor.setRangeText(shown, p, p, "preserve");
    editor.dispatchEvent(new Event("input"));
    core.editor_mark(p, codes);
  }
  editor.setSelectionRange(from + shown.length, from + shown.length);
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

// A Layout menu code item: a pair wraps the selection, an each-line item goes at each line start of it, and any
// other item, with text selected, first asks whether the selection is to be replaced (No leaves it as it is).
async function insertCode(id) {
  const code = core ? core.menu_code(id) : "";
  if (!code) return id === 1910 ? newParagraph() : undefined;
  const c = JSON.parse(code);
  if (c.pair) return wrapSelection(c.pair[0], c.pair[1]);
  if (c.each_line !== undefined) return insertEachLine(c.each_line);
  if (editor.selectionStart !== editor.selectionEnd && !(await yesNo(core.replace_question()))) return;
  // New Paragraph closes and opens the paragraph style it is in.
  if (id === 1910) return newParagraph();
  if (c.text !== undefined) insertCodes(c.text);
  else if (c.setting === "termination_sign") {
    // The signs and the hard space are text; the rest are codes.
    insertText(doc.kind === "braille" ? ",'" : "&@");
  } else if (c.setting === "hard_space") {
    insertText("\u00a0");
  } else {
    const s = settings.shortcuts;
    insertCodes({ grade0: s.grade0, cbc: s.cbc, flush_right_leader: s.flush_right_leader, new_page: s.new_page, new_ref_page: s.new_ref_page }[c.setting] ?? "");
  }
}

const COMMANDS = {
  1101: fileNew,
  1102: () => fileInput.click(),
  // An autosave pass after Save, and after Save As that leaves the document unchanged.
  1105: async () => {
    await save(false);
    await autosavePass();
  },
  1106: async () => {
    await save(true);
    if (!doc.dirty) await autosavePass();
  },
  1109: translateDocument,
  1111: print,
  1112: emboss,
  1201: () => editStandard(1201),
  1202: () => editStandard(1202),
  1203: () => editStandard(1203),
  1204: () => editStandard(1204),
  1205: () => editStandard(1205),
  1206: () => editStandard(1206),
  1207: () => editStandard(1207),
  1209: toggleHighlight,
  1211: changeCase,
  1213: insertFile,
  1215: gotoPosition,
  1216: () => find(false),
  1217: () => find(true),
  1218: replace,
  1220: () => message(PLANNED_SPELL),
  1221: () => message(PLANNED_SPELL),
  1401: applyStyle,
  1402: applyLastStyle,
  1403: () => styleLevel(1),
  1404: () => styleLevel(-1),
  1406: joinStyles,
  1407: joinLines,
  1409: setPageNumber,
  1410: () => headerFooter(HEADER),
  1411: () => headerFooter(FOOTER),
  1413: () => message(PLANNED_FORMATTER),
  1416: codeList,
  1301: () => fittedView((v) => (v.full_page = !v.full_page)),
  1303: () => fittedView((v) => (v.font = "print")),
  1304: () => fittedView((v) => (v.font = "braille")),
  1305: () => fittedView((v) => (v.font = "simbraille")),
  // Six-key entry in either kind of document.
  1307: () => setView((v) => (v.six_key = !v.six_key)),
  1309: () => fittedView((v) => (v.codes = !v.codes)),
  1310: () => {
    // Turning the translated line on is refused with message 2636 when the editing area would then hold no row;
    // turning it off never is.
    const on = doc.view.translated;
    fittedView((v) => (v.translated = !v.translated), on, TOO_SMALL_FOR_LINE);
  },
  1103: closeDocument,
  1107: saveEBraille,
  1114: exitProgram,
  1501: () => tableProperties(),
  1503: () => tableProperties(),
  2601: () => embosserSetup(false),
  2602: () => printerSetup(false),
  2604: pageNumbering,
  2701: () => setRegion(0),
  2702: () => setRegion(1),
  2703: () => setRegion(2),
  2704: () => setRegion(3),
  2705: () => setRegion(4),
  2706: () => setRegion(5),
  2801: () => setGrade("[g1l]"),
  2802: () => setGrade("[g2u]"),
  2606: ebrailleMetadata,
  2608: addStyle,
  2609: modifyStyle,
  2610: deleteStyle,
  2612: createTemplate,
  2613: () => message(PLANNED_TEMPLATES),
  2901: () => setHyphen(2901),
  2902: () => setHyphen(2902),
  2903: () => setHyphen(2903),
  2904: () => setHyphen(2904),
  2905: () => setHyphen(2905),
  3202: () => message(PLANNED_HELP),
  3204: () => message(PLANNED_HELP),
  3205: () => message(PLANNED_HELP),
  3206: () =>
    message(
      "BrailleCraft 0.1: the main window, menus, dialogs, editing, find and replace, six-key entry, the code vocabulary, braille fonts for 18 encodings, and translation with the BrailleCraft engine and its tables. The file format, printing and embossing follow in the plan.",
    ),
  3210: () => message("Updates are published on the BrailleCraft GitHub page."),
  // BrailleCraft's own Global, MCP Server item: a page cannot listen for connections, so the web build has no server.
  3220: () =>
    message(
      "The web build cannot run the MCP server: a web page cannot listen for connections. Use the desktop program (Global, MCP Server, or braillecraft-desktop --control PORT) with braillecraft-cli mcp --connect PORT, or braillecraft-cli mcp on its own.",
    ),
  3101: async () => (await onceNotice("GlobalEmbosserWarning", GLOBAL_EMBOSSER_WARNING)) && embosserSetup(true),
  3102: async () => (await onceNotice("GlobalPrinterWarning", GLOBAL_PRINTER_WARNING)) && printerSetup(true),
  3104: () => showDialog(1060, { memory: "", focus: 7, after: fileLocationsEnabling }),
  3105: defaultViews,
  3106: internationalization,
  3107: globalFonts,
  3108: viewPreferences,
  3109: shortcutPreferences,
  3111: notificationSettings,
  3113: () =>
    showDialog(1037, {
      memory: "",
      setup(d) {
        d.setItems(7, ["Text Only", "URLs Only", "Both Text and URLs", "Structured Hyperlinks"], 2);
        d.setItems(23, ["Mark with asterisk (*)", "Ignore", "Output Unicode value"], 0);
      },
    }),
  3114: () => showDialog(1035, { memory: "" }),
  3115: () =>
    showDialog(1036, {
      memory: "",
      setup(d) {
        d.setItems(10, ["Ignore", "Mark from Word document", "Treat hard page breaks as print page breaks", "Use publishers' <ipp no=#> reference page tags"], 0);
        // The kinds a UEB typeform mapping can be.
        for (const combo of [20, 23, 26, 29, 32, 35]) d.setItems(combo, ["(Unused)", "Font Is", "Font Contains", "Highlight Is", "Text Color Is", "Drop Shadow", "Outline"], 0);
      },
    }),
  3110: pythonOptions,
  3116: pdfOptions,
  // Global, Formatted Braille Importer (1045): the settings the formatted braille import reads. The four groups are
  // braille page numbers on even and on odd pages and print page numbers on even and on odd pages of the file; then
  // the running head and foot pages, the lines and cells of the file's pages. With "Read formatted braille without
  // interpretation" checked (formatting preserved) the other controls are disabled.
  3117: async () => {
    const GROUPS = [[8, 9, 10, 11, 12], [14, 15, 16, 17, 18], [20, 21, 22, 23, 24], [26, 27, 28, 29, 30]];
    const HEADER = [32, 33, 34, 35];
    const FOOTER = [37, 38, 39, 40];
    const s = braideSettings();
    const { result, d } = await showDialog(1045, {
      setup(d) {
        d.setChecked(6, s.preserve);
        [s.evenMain, s.oddMain, s.evenReference, s.oddReference].forEach((v, k) => d.setRadio(GROUPS[k], v));
        d.setRadio(HEADER, s.header);
        d.setRadio(FOOTER, s.footer);
        d.setText(42, String(s.lines));
        d.setText(44, String(s.cells));
      },
      after(d) {
        const apply = () => {
          for (let c = 7; c <= 44; c++) d.enable(c, !d.checked(6));
        };
        d.get(6)?.addEventListener("change", apply);
        apply();
      },
    });
    if (result !== "ok") return;
    const n = { ...s, preserve: d.checked(6) };
    const corners = GROUPS.map((g, k) => d.radio(g) ?? [s.evenMain, s.oddMain, s.evenReference, s.oddReference][k]);
    [n.evenMain, n.oddMain, n.evenReference, n.oddReference] = corners;
    n.header = d.radio(HEADER) ?? s.header;
    n.footer = d.radio(FOOTER) ?? s.footer;
    // A count that is not a number above 0 keeps the one there was.
    const lines = d.number(42);
    const cells = d.number(44);
    if (lines > 0) n.lines = lines;
    if (cells > 0) n.cells = cells;
    saveBraideSettings(n);
  },
  3211: async () => {
    if (core) await tableFor("");
    await showDialog(1016, {
      setup(d) {
        d.setText(8, `Version ${core ? core.version() : ""}`);
        d.setText(10, `Braille translation: the BrailleCraft engine, ${tableCount ? `${tableCount} tables` : "no tables found"}`);
      },
    });
  },
};

async function run(id, menuLabel, itemLabel) {
  closeMenus();
  // The Layout menu's code items and the pictures items (ranges, as in the desktop program).
  if (id >= 1701 && id <= 2404) {
    insertCode(id);
  } else if (id >= 2501 && id <= 2511) {
    await message(PLANNED_PICTURES);
  } else if (id >= LANGUAGE_BASE && id < LANGUAGE_BASE + 400) {
    await chooseLanguage(id - LANGUAGE_BASE);
  } else if (id >= COURSE_BASE && id < COURSE_BASE + COURSE_MAX) {
    await chooseLearningStep(id - COURSE_BASE);
  } else if (id >= RECENT_FIRST && id < RECENT_FIRST + RECENT_MAX) {
    await openRecent(id - RECENT_FIRST);
  } else if (COMMANDS[id]) {
    await COMMANDS[id]();
  } else {
    const rid = dialogFor(menuLabel, itemLabel);
    if (rid) {
      await showDialog(rid);
    } else {
      statusLine.textContent = `${plain(itemLabel)} is not in the web build yet.`;
    }
  }
  // After every command the menus' marks follow the document, as in the desktop program.
  refreshMenus();
  if (!document.querySelector("dialog[open]")) {
    editor.focus();
  }
}

// ------------------------------------------------------------------ the menu bar (WAI-ARIA menubar pattern)

const menubar = $("menubar");
const tops = [];

function itemNode(entry, menuLabel, level) {
  const li = document.createElement("li");
  li.setAttribute("role", "none");
  if (entry.separator) {
    li.setAttribute("role", "separator");
    return li;
  }
  const item = document.createElement("div");
  item.tabIndex = -1;
  if (entry.id) item.dataset.id = entry.id;
  const kind = entry.kind === "check" ? "menuitemcheckbox" : entry.kind === "radio" ? "menuitemradio" : "menuitem";
  item.setAttribute("role", kind);
  if (kind !== "menuitem") {
    item.setAttribute("aria-checked", "false");
  }
  const text = document.createElement("span");
  const label = entry.label;
  const at = label.search(/(?<!&)&[^&]/);
  if (at >= 0) {
    text.append(document.createTextNode(plain(label.slice(0, at))));
    const mn = document.createElement("span");
    mn.className = "mn";
    mn.textContent = label[at + 1];
    text.append(mn, document.createTextNode(plain(label.slice(at + 2))));
  } else {
    text.textContent = plain(label);
  }
  item.append(text);
  item.dataset.mnemonic = mnemonic(label) ?? "";
  li.append(item);
  if (entry.items) {
    item.setAttribute("aria-haspopup", "menu");
    item.setAttribute("aria-expanded", "false");
    li.append(menuNode(entry.items, plain(label), level + 1, plain(label)));
  } else {
    if (entry.shortcut) {
      const s = document.createElement("span");
      s.className = "shortcut";
      s.textContent = entry.shortcut;
      item.append(s);
      item.setAttribute("aria-keyshortcuts", entry.shortcut.replace(/\s/g, "").replace(/\bDel\b/, "Delete"));
    }
    // A greyed item (aria-disabled) does nothing, as a greyed menu item.
    item.addEventListener("click", () => {
      if (item.getAttribute("aria-disabled") !== "true") run(entry.id, menuLabel, entry.label);
    });
  }
  return li;
}

function menuNode(items, menuLabel, level, name) {
  const ul = document.createElement("ul");
  ul.setAttribute("role", "menu");
  ul.setAttribute("aria-label", name);
  ul.hidden = true;
  for (const entry of items) {
    ul.append(itemNode(entry, menuLabel, level));
  }
  return ul;
}

for (const m of MENUS) {
  const li = document.createElement("li");
  li.setAttribute("role", "none");
  const item = document.createElement("div");
  item.setAttribute("role", "menuitem");
  item.setAttribute("aria-haspopup", "menu");
  item.setAttribute("aria-expanded", "false");
  item.tabIndex = tops.length === 0 ? 0 : -1;
  item.dataset.mnemonic = mnemonic(m.label) ?? "";
  const at = m.label.indexOf("&");
  const mn = document.createElement("span");
  mn.className = "mn";
  mn.textContent = m.label[at + 1];
  item.append(document.createTextNode(m.label.slice(0, at)), mn, document.createTextNode(m.label.slice(at + 2)));
  li.append(item, menuNode(m.items, m.label, 1, plain(m.label)));
  menubar.append(li);
  tops.push(item);
  item.addEventListener("click", () => (item.getAttribute("aria-expanded") === "true" ? closeMenus() : openMenu(item, true)));
}

function submenuOf(item) {
  return item.parentElement.querySelector(":scope > [role=menu]");
}

function itemsOf(menu) {
  return [...menu.querySelectorAll(":scope > li > [role^=menuitem]")];
}

function openMenu(item, focusFirst) {
  const menu = submenuOf(item);
  if (!menu || item.getAttribute("aria-disabled") === "true") {
    return;
  }
  if (tops.includes(item)) {
    closeMenus();
  }
  item.setAttribute("aria-expanded", "true");
  menu.hidden = false;
  if (focusFirst) {
    itemsOf(menu)[0]?.focus();
  }
}

function closeMenu(item) {
  item.setAttribute("aria-expanded", "false");
  const menu = submenuOf(item);
  if (menu) {
    menu.hidden = true;
    menu.querySelectorAll("[aria-expanded=true]").forEach(closeMenu);
  }
}

function closeMenus() {
  tops.forEach(closeMenu);
}

function moveTop(from, step) {
  const i = (tops.indexOf(from) + step + tops.length) % tops.length;
  tops.forEach((t, k) => (t.tabIndex = k === i ? 0 : -1));
  const wasOpen = from.getAttribute("aria-expanded") === "true";
  closeMenus();
  tops[i].focus();
  if (wasOpen) {
    openMenu(tops[i], true);
  }
}

menubar.addEventListener("keydown", (e) => {
  const item = e.target.closest("[role^=menuitem]");
  if (!item) {
    return;
  }
  const isTop = tops.includes(item);
  const menu = item.closest("[role=menu]");
  const topOf = isTop ? item : tops.find((t) => t.parentElement.contains(item));
  let handled = true;
  if (isTop) {
    if (e.key === "ArrowRight") moveTop(item, 1);
    else if (e.key === "ArrowLeft") moveTop(item, -1);
    else if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") openMenu(item, true);
    else if (e.key === "ArrowUp") {
      openMenu(item, false);
      itemsOf(submenuOf(item)).at(-1)?.focus();
    } else if (e.key === "Escape") {
      closeMenus();
      editor.focus();
    } else if (e.key.length === 1) {
      const hit = tops.find((t) => t.dataset.mnemonic === e.key.toUpperCase());
      if (hit) {
        tops.forEach((t) => (t.tabIndex = t === hit ? 0 : -1));
        hit.focus();
        openMenu(hit, true);
      } else handled = false;
    } else handled = false;
  } else {
    const items = itemsOf(menu);
    const i = items.indexOf(item);
    if (e.key === "ArrowDown") items[(i + 1) % items.length].focus();
    else if (e.key === "ArrowUp") items[(i - 1 + items.length) % items.length].focus();
    else if (e.key === "Home") items[0].focus();
    else if (e.key === "End") items.at(-1).focus();
    else if (e.key === "ArrowRight") {
      if (item.getAttribute("aria-haspopup")) openMenu(item, true);
      else moveTop(topOf, 1);
    } else if (e.key === "ArrowLeft") {
      const parent = menu.parentElement.querySelector(":scope > [role^=menuitem]");
      if (parent && !tops.includes(parent)) {
        closeMenu(parent);
        parent.focus();
      } else moveTop(topOf, -1);
    } else if (e.key === "Escape") {
      const parent = menu.parentElement.querySelector(":scope > [role^=menuitem]");
      closeMenu(parent);
      parent.focus();
    } else if (e.key === "Enter" || e.key === " ") {
      if (item.getAttribute("aria-haspopup")) openMenu(item, true);
      else item.click();
    } else if (e.key === "Tab") {
      closeMenus();
      handled = false;
    } else if (e.key.length === 1) {
      const hit = items.find((x) => x.dataset.mnemonic === e.key.toUpperCase());
      if (hit) {
        if (hit.getAttribute("aria-haspopup")) openMenu(hit, true);
        else hit.click();
      } else handled = false;
    } else handled = false;
  }
  if (handled) {
    e.preventDefault();
    e.stopPropagation();
  }
});

document.addEventListener("click", (e) => {
  if (!menubar.contains(e.target)) {
    closeMenus();
  }
});

// F10 moves to the menu bar, as in the desktop program; the shortcuts the browser leaves free run their commands.
const SHORTCUTS = new Map();
for (const m of MENUS) {
  (function collect(items) {
    for (const entry of items) {
      if (entry.items) collect(entry.items);
      else if (entry.shortcut) SHORTCUTS.set(entry.shortcut.toLowerCase().replace(/\bdel\b/, "delete"), [entry.id, m.label, entry.label]);
    }
  })(m.items);
}

document.addEventListener("keydown", (e) => {
  if (document.querySelector("dialog[open]")) {
    return;
  }
  if (e.key === "F10" && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    const current = tops.find((t) => t.tabIndex === 0) ?? tops[0];
    current.focus();
    return;
  }
  const parts = [];
  if (e.ctrlKey) parts.push("ctrl");
  if (e.altKey) parts.push("alt");
  if (e.shiftKey) parts.push("shift");
  parts.push(e.key.length === 1 ? e.key.toLowerCase() : e.key.toLowerCase());
  const hit = SHORTCUTS.get(parts.join("+"));
  // In the editor, undo, redo and the clipboard keys stay the browser's own (the text control's, as on the desktop).
  if (hit && e.target === editor && hit[0] >= 1201 && hit[0] <= 1207) return;
  if (hit && !menubar.contains(e.target)) {
    e.preventDefault();
    run(...hit);
  }
});

// ------------------------------------------------------------------ recent documents and autosave

// IndexedDB database `name` (its object stores made at first use), opened once; then one transaction on `store` in
// which `act` makes its requests, answering the last request's result once the transaction is complete.
const databases = new Map();
async function inDatabase(name, store, mode, act) {
  if (!databases.has(name)) {
    databases.set(
      name,
      new Promise((resolve, reject) => {
        const r = indexedDB.open(name, 1);
        r.onupgradeneeded = () => r.result.createObjectStore(store);
        r.onsuccess = () => resolve(r.result);
        r.onerror = () => reject(r.error);
      }),
    );
  }
  const db = await databases.get(name);
  return new Promise((resolve, reject) => {
    const tx = db.transaction(store, mode);
    const request = act(tx.objectStore(store));
    tx.oncomplete = () => resolve(request?.result);
    tx.onerror = tx.onabort = () => reject(tx.error);
  });
}

// File, the recent documents (items 1116 to 1120, after Exit and a separator): the last five document files the
// page let go of, newest first, by name, kept as RecentDocs.0 to RecentDocs.4. A browser file has no path to open
// again, so the bytes each was last read or saved with are kept by the browser (IndexedDB "braillecraft", store
// "recent", by name). The list is read only once a page has ended a run (ConfigInUse, set as the page goes away).
const RECENT_FIRST = 1116;
const RECENT_MAX = 5;
const recent = [];
if ((store.get("ConfigInUse") ?? "").trim().toLowerCase() === "yes") {
  for (let i = 0; i < RECENT_MAX; i++) {
    const name = store.get(`RecentDocs.${i}`);
    if (!name) break;
    recent.push(name);
  }
}
const recentStore = (mode, act) => inDatabase("braillecraft", "recent", mode, act);

function keepRecent() {
  for (let i = 0; i < RECENT_MAX; i++) store.set(`RecentDocs.${i}`, recent[i] ?? "");
  const file = menubar.querySelector('[data-id="1114"]')?.closest("[role=menu]");
  const items = recent.map((name, i) => ({ id: RECENT_FIRST + i, label: `${i + 1} ${name}`.replaceAll("&", "&&") }));
  if (file) setDynamicItems(file, items.length ? [{ separator: true }, ...items] : [], null, "&File");
}

// A document file's name put at the front (the last dropped when the list is full), or every entry of it taken out.
function recentAdd(name) {
  recent.unshift(name);
  recent.length = Math.min(recent.length, RECENT_MAX);
  keepRecent();
}
function recentRemove(name) {
  for (let i = recent.length - 1; i >= 0; i--) if (recent[i] === name) recent.splice(i, 1);
  keepRecent();
}

async function keepRecentBytes(name, bytes) {
  try {
    await recentStore("readwrite", (s) => s.put(bytes, name));
  } catch {
    // Kept by name only: opening it from the list then says it cannot be opened.
  }
}

// File, recent document `i`: opened as File, Open opens it. A name whose bytes the browser no longer has gives
// message 2428 and stays listed.
async function openRecent(i) {
  const name = recent[i];
  let bytes = null;
  try {
    bytes = await recentStore("readonly", (s) => s.get(name));
  } catch {
    bytes = null;
  }
  if (bytes) await openBytes(name, bytes);
  else await message(`Unable to open file: ${name}`);
}

// The document given up (File, New, Open and Close, a recent document, Exit): with a document file it goes to the
// front of the recent documents; with no document left, every autosaved document goes.
async function letGo() {
  if (doc.documentFile && doc.name) recentAdd(doc.name);
  await autosaveRemoveAll();
}

// The page going away ends the run, as Exit does in the desktop program; the bytes are kept already.
window.addEventListener("pagehide", () => {
  if (doc.documentFile && doc.name) recentAdd(doc.name);
  store.set("ConfigInUse", "yes");
});

// Autosave, as the desktop program does it (Global, File Locations: AutoSave.Enabled and AutoSave.Time): a timer
// ticks every second and a pass runs once the time has gone by since the last one (the first at the first tick),
// and after File Save, after Save As that leaves the document unchanged, and when the document is given up. The
// browser keeps the documents (IndexedDB "braillecraft-autosave", store "files"): one record per autosave number
// {number, kind (saved, unsaved or untitled), name, braille, untitled number, bytes of the .dxp or .dxb file} and the
// record "index", the numbers the last pass wrote. Autosave runs in the first BrailleCraft page only (a Web Lock),
// so two pages never write the same records.
const WRITE_FAILED = "BrailleCraft encountered an error while writing autosave information.  Autosave will be disabled.";
const autosave = { started: false, last: null, failed: false, running: false, number: [-1, -1], kept: [] };
const autosaveStore = (mode, act) => inDatabase("braillecraft-autosave", "files", mode, act);

async function autosaveFailed() {
  if (autosave.failed) return;
  autosave.failed = true;
  await message(WRITE_FAILED);
}

// What the document is to the recovery wizard: its document file, the file it was imported from, or untitled.
function autosaveKind() {
  if (doc.documentFile) return "saved";
  return !doc.saved && /^Untitled (Braille|Print) Document \d+$/.test(doc.name) ? "untitled" : "unsaved";
}

async function autosavePass() {
  if (!autosave.started || autosave.running) return;
  autosave.running = true;
  try {
    // A changed document is written again every pass under the lowest number not in use; one not changed has its
    // record taken away.
    let record = null;
    if (doc.dirty) {
      autosave.number[1] = autosave.number[0];
      const used = [...autosave.number, ...autosave.kept];
      let n = 0;
      while (used.includes(n)) n += 1;
      autosave.number[0] = n;
      const braille = doc.kind === "braille";
      record = { number: n, kind: autosaveKind(), name: doc.name, braille, untitled: Number(/\d+$/.exec(doc.name)?.[0] ?? 1), bytes: await documentBytes(1) };
    }
    const old = record ? autosave.number[1] : autosave.number[0];
    if (!record) autosave.number[0] = -1;
    autosave.number[1] = -1;
    const entries = [...(record ? [record.number] : []), ...autosave.kept];
    await autosaveStore("readwrite", (s) => {
      if (record) s.put(record, record.number);
      if (old >= 0) s.delete(old);
      if (entries.length) s.put({ entries }, "index");
      else s.delete("index");
    });
  } catch {
    await autosaveFailed();
  } finally {
    autosave.running = false;
  }
}

// Every autosaved document taken away, but those the wizard left for the next page (`kept`).
async function autosaveRemoveAll() {
  if (!autosave.started) return;
  autosave.number = [-1, -1];
  try {
    await autosaveStore("readwrite", (s) => {
      const keys = s.getAllKeys();
      keys.onsuccess = () => {
        for (const k of keys.result) if (!autosave.kept.includes(k)) s.delete(k);
        if (autosave.kept.length) s.put({ entries: autosave.kept }, "index");
      };
    });
  } catch {
    await autosaveFailed();
  }
}

function autosaveTick() {
  if ((store.get("AutoSave.Enabled") ?? "yes").trim().toLowerCase() !== "yes") return;
  const seconds = Number(store.get("AutoSave.Time") ?? 300);
  if (autosave.last !== null && Date.now() - autosave.last < seconds * 1000) return;
  autosave.last = Date.now();
  autosavePass();
}

// The Autosave Recovery Wizard (dialog 1061), when the last page left autosaved documents: Open (4) opens the first
// of them, as the web build holds one document (the desktop program opens each in a window of its own); the others
// stay for the wizard at the next page load (BrailleCraft's choice until the web build holds several documents).
// Finish (5) asks twice and takes them all away; Exit (6, and Escape) asks, keeps them and ends as Exit does.
// Answers whether autosave starts.
async function recoveryWizard(records) {
  const labels = records.map((r) => (r.kind === "untitled" ? `Untitled ${r.braille ? "Braille" : "Print"} Document ${r.untitled}` : r.name));
  for (;;) {
    const { result } = await showDialog(1061, { closes: [4, 5, 6], focus: 8, setup: (d) => d.setItems(8, labels, 0) });
    if (result === "4") {
      autosave.kept = records.slice(1).map((r) => r.number);
      await openBytes(records[0].name, records[0].bytes, records[0]);
      await autosaveStore("readwrite", (s) => (autosave.kept.length ? s.put({ entries: autosave.kept }, "index") : s.delete("index")));
      return true;
    }
    if (result === "5") {
      if (
        (await yesNo("You have not recovered all autosaved documents.  Are you sure you want to finish the BrailleCraft recovery wizard?")) &&
        (await yesNo("Finishing the recovery wizard before all documents have been processed is not reversible.  The remaining documents cannot later be recovered.  Proceed?"))
      ) {
        await autosaveStore("readwrite", (s) => s.clear());
        return true;
      }
    } else if (await okCancel("BrailleCraft will now exit.  The recovery wizard will be displayed again when BrailleCraft is next launched.")) {
      window.close();
      await message("To leave BrailleCraft, close its browser tab or window.");
      return false;
    }
  }
}

// At page load, once the document area is ready: the wizard when the index names documents, then the timer. Not
// without the core (it reads the documents) or IndexedDB, and not in a second BrailleCraft page.
async function autosaveStart() {
  if (!core || !(await firstPage())) return;
  let records = [];
  try {
    const index = await autosaveStore("readonly", (s) => s.get("index"));
    records = (await Promise.all((index?.entries ?? []).map((n) => autosaveStore("readonly", (s) => s.get(n))))).filter(Boolean);
  } catch {
    return;
  }
  if (records.length && !(await recoveryWizard(records))) return;
  autosave.started = true;
  setInterval(autosaveTick, 1000);
}

// Whether this is the first BrailleCraft page of the browser: it holds a Web Lock for as long as it is open.
function firstPage() {
  if (!navigator.locks) return Promise.resolve(true);
  return new Promise((resolve) => {
    navigator.locks.request("braillecraft-autosave", { ifAvailable: true }, (lock) => {
      resolve(lock !== null);
      return lock ? new Promise(() => {}) : undefined;
    });
  });
}

// ------------------------------------------------------------------ start

showTitle();
showStatus();
applyView();
keepRecent();
// Unlisted bytes from earlier pages go.
recentStore("readwrite", (s) => {
  const keys = s.getAllKeys();
  keys.onsuccess = () => keys.result.filter((k) => !recent.includes(k)).forEach((k) => s.delete(k));
}).catch(() => {});
loadCore().then(async () => {
  await loadEncoding();
  applyView();
  if (core) {
    await embosserSetupAtStart();
  }
  await autosaveStart();
});

// ------------------------------------------------------------------ reading toolbar

// The reading functions of the screen reader scripts (the translated line, the style and table place, the location,
// the document name and status line, the line position and length, the last word, the table entry, the braille
// keyboard) as buttons after the status line, each answering through a polite live region, so a touch screen reader
// reaches them by swipe; their keyboard shortcuts can be turned off and changed in the Reading Keys dialog and are
// kept in the browser (WCAG 2.2 success criterion 2.1.4). Braille is announced as braille cell names, by the tables
// of the scripts.
const announceOut = $("announce");
function announce(text) {
  announceOut.textContent = "";
  setTimeout(() => {
    announceOut.textContent = text;
  }, 30);
}

const CELL_NA32 = "space;t h e sign;dot 5;number sign;e d sign;s h sign;a n d sign;dot 3;o f sign;w i t h sign;c h sign;i n g sign;dot 6;dots 3 6;dots 4 6;s t sign".split(";");
const CELL_NA48 = "low j;low a;low b;low c;low d;low e;low f;low g;low h;low i;w h sign;dots 5 6;g h sign;f o r sign;a r sign;t h sign;dot 4".split(";");
const CELL_NA91 = "o w sign;o u sign;e r sign;dots 4 5;dots 4 5 6;dot 4;a;b;c;d;e;f;g;h;i;j".split(";");
const CELL_NA107 = "k;l;m;n;o;p;q;r;s;t;u;v;w;x;y;z".split(";");
const CELL_NA123 = "dots 4 6;dots 4 5 6;dot 6;dots 1 2 4 5 6;hard space".split(";");
const CELL_RU1072 = "dot 1;dots 1 2;dots 2 4 5 6;dots 1 2 4 5;dots 1 4 5;dots 1 5;dots 2 4 5;dots 1 3 5 6;dots 2 4;dots 1 2 3 4 6;dots 1 3;dots 1 2 3;dots 1 3 4;dots 1 3 4 5;dots 1 3 5;dots 1 2 3 4".split(";");
const CELL_RU1088 = "dots 1 2 3 5;dots 2 3 4;dots 2 3 4 5;dots 1 3 6;dots 1 2 4;dots 1 2 5;dots 1 4;dots 1 2 3 4 5;dots 1 5 6;dots 1 3 4 6;dots 1 2 3 5 6;dots 2 3 4 6;dots 2 3 4 5 6;dots 2 4 6;dots 1 2 5 6;dots 1 2 4 6;dots 1 6".split(";");
const CELL_UNI = "space;a;low a;b;dot 3;k;low b;l;dot 4;c;i;f;s t sign;m;s;p;dot 5;e;low c;h;low i;o;low f;r;dots 4 5;d;j;g;a r sign;n;t;q;dot 6;c h sign;low e;g h sign;hyphen;u;low h;v;dots 4 6;s h sign;o w sign;e d sign;i n g sign;x;t h e sign;a n d sign;dots 5 6;w h sign;low d;o u sign;low j;z;low g;o f sign;dots 4 5 6;t h sign;w;e r sign;number sign;y;w i t h sign;f o r sign".split(";");

// The spoken name of one braille cell written as `ch` (North American ASCII, Russian or a Unicode braille pattern).
function cellName(ch) {
  if (!ch) return "";
  let v = ch.codePointAt(0);
  if (v === 30 || v === 160) return "hard space";
  if (v >= 32 && v < 48) return CELL_NA32[v - 32];
  if (v >= 48 && v <= 64) return CELL_NA48[v - 48];
  if (v >= 65 && v <= 90) v += 32;
  if (v >= 91 && v < 107) return CELL_NA91[v - 91];
  if (v >= 107 && v < 123) return CELL_NA107[v - 107];
  if (v >= 123 && v < 128) return CELL_NA123[v - 123];
  if (v >= 1072 && v < 1088) return CELL_RU1072[v - 1072];
  if (v >= 1088 && v < 1105) return CELL_RU1088[v - 1088];
  if (v >= 0x2800 && v < 0x2840) return CELL_UNI[v - 0x2800];
  return ch;
}

// `text` spoken cell by cell, runs of two or more spaces as a count.
function cellsOf(text) {
  const out = [];
  const chars = [...text];
  let i = 0;
  while (i < chars.length) {
    if (chars[i] === " " && chars[i + 1] === " ") {
      let j = i;
      while (j < chars.length && chars[j] === " ") j++;
      out.push(`${j - i} spaces`);
      i = j;
    } else {
      out.push(cellName(chars[i]));
      i++;
    }
  }
  return out.join(", ");
}

// The status line taken apart (Page, Line and Column are left out in the coded view).
function statusFields() {
  const t = statusLine.textContent;
  const field = (re) => {
    const m = t.match(re);
    return m ? m[1] : null;
  };
  const row = t.match(/Row\s*(\d+)\/(\d+)/);
  const col = t.match(/Col\s*(\d+)\/(\d+)/);
  const keys = t.match(/Keys:(\d)([Hh+]?)/);
  const style = field(/Current Style:\s*(.*?)(?:\s\s|$)/);
  const inTable = Boolean(row && col) || t.includes("Matrix");
  return {
    text: t,
    page: field(/Page:\s*(\S+)/),
    line: field(/Line:\s*(\d+)/),
    column: field(/Column:\s*(\d+)/),
    style: (style ?? (inTable ? t.split("  ")[0] : "")).trim(),
    row: row ? [Number(row[1]), Number(row[2])] : null,
    col: col ? [Number(col[1]), Number(col[2])] : null,
    inTable,
    coded: t !== "" && !t.includes("Page:"),
    keyboard: Boolean(keys && keys[2] === "+"),
  };
}

let lastReading = { id: null, at: 0 };
function readTwice(id) {
  const now = performance.now();
  const again = lastReading.id === id && now - lastReading.at < 700;
  lastReading = { id, at: now };
  return again;
}

const READING_COMMANDS = [
  {
    id: "translated",
    label: "Translated line",
    key: "Alt+7",
    async run() {
      if (!doc.view.translated) {
        setView((v) => (v.translated = true));
      }
      lastLine = null;
      await showTranslatedLine();
      // The first translation of a document fetches its tables and lands the text a moment later.
      let text = translated.textContent;
      for (let waited = 0; !text.trim() && waited < 3000; waited += 100) {
        await new Promise((r) => setTimeout(r, 100));
        text = translated.textContent;
      }
      if (!text.trim()) announce("blank");
      else if (text === "Error message 0") announce("translation not available");
      else announce(doc.kind === "braille" ? text : cellsOf(text));
    },
  },
  {
    id: "style",
    label: "Style",
    key: "Alt+8",
    run() {
      const s = statusFields();
      if (readTwice("style") && s.inTable && s.row && s.col) announce(`Row ${s.row[0]} Column ${s.col[0]}`);
      else announce(s.style || "no style");
    },
  },
  {
    id: "location",
    label: "Location",
    key: "Alt+9",
    run() {
      const s = statusFields();
      if (s.coded) announce("location not available in coded view, press Alt F3 to turn codes off");
      else announce(`P${s.page} L${s.line} C${s.column}`);
    },
  },
  {
    id: "document",
    label: "Document and status",
    key: "Alt+0",
    run() {
      announce(`${$("doc-title").textContent}: ${statusLine.textContent}`);
    },
  },
  {
    id: "position",
    label: "Line position",
    key: "Alt+,",
    run() {
      const s = statusFields();
      const c = caretLine();
      const pos = s.column ? Number(s.column) : c.column;
      const toEnd = c.text.length - pos + 1;
      if (readTwice("position")) announce(`${c.text.trimEnd().length - pos} to last nonspace character`);
      else announce(toEnd === 0 ? `position ${pos} end of line` : `position ${pos}`);
    },
  },
  {
    id: "length",
    label: "Line length",
    key: "Alt+.",
    run() {
      const s = statusFields();
      if (s.coded) {
        announce("coded view");
        return;
      }
      const c = caretLine();
      if (readTwice("length")) announce(`${c.text.length - c.text.trimStart().length} left spaces`);
      else announce(`${c.text.length} total characters.`);
    },
  },
  {
    id: "lastWord",
    label: "Last word",
    key: "Alt+End",
    run() {
      const s = statusFields();
      if (s.coded) {
        announce("location not available in coded view, press Alt F3 to turn codes off");
        return;
      }
      const parts = caretLine().text.trimEnd().split(/[ -]/);
      const word = parts[parts.length - 1] ?? "";
      announce(doc.kind === "braille" ? cellsOf(word) : word || "blank");
    },
  },
  {
    id: "tableEntry",
    label: "Table entry",
    key: "Alt+T",
    run() {
      const s = statusFields();
      if (!s.inTable) {
        announce("not in a table");
        return;
      }
      const line = caretLine().text;
      let col = s.col ? s.col[0] : 1;
      let parts;
      if (s.coded) parts = line.split(/\[<\]|\[>\]/);
      else if (s.style.includes("Linear")) parts = line.split(/; |: /);
      else if (s.style.includes("Listed")) {
        announce(line);
        return;
      } else parts = line.split(/ \.\.|\.\.|  /);
      parts = parts.map((p) => p.trim()).filter((p) => p);
      if (s.style.includes("Two-Page") && s.page && "13579".includes(s.page.slice(-1)) && s.col) col = col - s.col[1] - 1;
      const entry = col > 0 && col <= parts.length ? parts[col - 1] : "";
      announce(doc.kind === "braille" ? cellsOf(entry) : entry || "blank");
    },
  },
  {
    id: "keyboard",
    label: "Braille keyboard",
    key: "Alt+C",
    run() {
      announce(statusFields().keyboard ? "braille keyboard connected" : "braille keyboard not connected");
    },
  },
];

// The shortcuts as kept: on or off, and each command's key (the defaults where none was changed).
function readingKeysOn() {
  return store.get("reading.enabled") !== "no";
}
function readingKeys() {
  const kept = store.json("reading.keys", {});
  const keys = {};
  for (const c of READING_COMMANDS) keys[c.id] = kept[c.id] ?? c.key;
  return keys;
}

// A key press as the dialog and the kept keys name it: the modifiers, then the key (ArrowDown as Down, a letter in
// upper case); "" for a modifier alone.
function comboOf(e) {
  const key = e.key;
  if (["Alt", "Control", "Shift", "Meta", "AltGraph"].includes(key)) return "";
  const parts = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Meta");
  parts.push(key.length === 1 ? key.toUpperCase() : key.replace(/^Arrow/, ""));
  return parts.join("+");
}

function buildReadingToolbar() {
  const bar = $("reading");
  bar.textContent = "";
  const keys = readingKeys();
  const on = readingKeysOn();
  for (const c of READING_COMMANDS) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = c.label;
    if (on && keys[c.id]) {
      button.setAttribute("aria-keyshortcuts", keys[c.id]);
      button.title = `${c.label} (${keys[c.id]})`;
    }
    button.addEventListener("click", () => c.run());
    bar.append(button);
  }
  const keysButton = document.createElement("button");
  keysButton.type = "button";
  keysButton.textContent = "Reading keys...";
  keysButton.addEventListener("click", () => readingKeysDialog());
  bar.append(keysButton);
}

// The shortcuts, on every key press outside a dialog.
document.addEventListener("keydown", (e) => {
  if (!readingKeysOn() || e.target.closest("dialog")) return;
  const combo = comboOf(e);
  if (!combo || !(e.altKey || e.ctrlKey || e.metaKey)) return;
  const keys = readingKeys();
  const command = READING_COMMANDS.find((c) => keys[c.id] === combo);
  if (!command) return;
  e.preventDefault();
  e.stopPropagation();
  command.run();
});

// The Reading Keys dialog: the shortcuts on or off, and a field per command that takes the key pressed in it
// (Backspace or Delete clears it); Reset puts the defaults back.
async function readingKeysDialog() {
  const { dialog, form } = newDialog("Reading Keys", "reading-keys");
  const keys = readingKeys();
  const onRow = document.createElement("div");
  onRow.className = "row";
  const onBox = document.createElement("input");
  onBox.type = "checkbox";
  onBox.id = "reading-keys-on";
  onBox.checked = readingKeysOn();
  const onLabel = document.createElement("label");
  onLabel.htmlFor = onBox.id;
  onLabel.textContent = "Keyboard shortcuts on";
  onRow.append(onBox, onLabel);
  form.append(onRow);
  const note = document.createElement("p");
  note.id = "reading-keys-note";
  note.textContent = "To change a shortcut, move to its field and press the keys you want; Backspace clears it.";
  form.append(note);
  const grid = document.createElement("div");
  grid.className = "keys-table";
  const fields = new Map();
  for (const c of READING_COMMANDS) {
    const label = document.createElement("label");
    label.htmlFor = `reading-key-${c.id}`;
    label.textContent = c.label;
    const field = document.createElement("input");
    field.type = "text";
    field.id = label.htmlFor;
    field.value = keys[c.id] ?? "";
    field.readOnly = true;
    field.setAttribute("aria-describedby", note.id);
    field.addEventListener("keydown", (e) => {
      if (e.key === "Tab") return;
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Backspace" || e.key === "Delete") {
        field.value = "";
        return;
      }
      if (e.key === "Escape") {
        dialog.close("cancel");
        return;
      }
      const combo = comboOf(e);
      if (combo) field.value = combo;
    });
    grid.append(label, field);
    fields.set(c.id, field);
  }
  form.append(grid);
  const buttons = document.createElement("div");
  buttons.className = "buttons";
  const reset = document.createElement("button");
  reset.type = "button";
  reset.textContent = "Reset to defaults";
  reset.addEventListener("click", () => {
    for (const c of READING_COMMANDS) fields.get(c.id).value = c.key;
  });
  const ok = document.createElement("button");
  ok.type = "submit";
  ok.className = "default";
  ok.textContent = "OK";
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.textContent = "Cancel";
  cancel.addEventListener("click", () => dialog.close("cancel"));
  buttons.append(reset, ok, cancel);
  form.append(buttons);
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const chosen = {};
    const seen = new Map();
    for (const c of READING_COMMANDS) {
      const v = fields.get(c.id).value.trim();
      if (v && seen.has(v)) {
        announce(`${v} is used by both ${seen.get(v)} and ${c.label}; change one of them.`);
        fields.get(c.id).focus();
        return;
      }
      if (v) seen.set(v, c.label);
      chosen[c.id] = v;
    }
    store.set("reading.enabled", onBox.checked ? null : "no");
    store.set("reading.keys", JSON.stringify(chosen));
    buildReadingToolbar();
    dialog.close("ok");
  });
  $("dialogs").append(dialog);
  const closed = closedPromise(dialog);
  dialog.showModal();
  onBox.focus();
  await closed;
  editor.focus();
}

buildReadingToolbar();
