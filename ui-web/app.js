// BrailleMaker web build: the desktop program's menus and dialogs (ui-web/tables.js, generated from the desktop's own
// tables), a document editor with the translated line and status line, and the core (bm-web, WebAssembly) for
// documents, translation, embosser setups and embossing.

import { MENUS, DIALOGS } from "./tables.js";

const $ = (id) => document.getElementById(id);
const editor = $("editor");
const translated = $("translated");
const statusLine = $("status");

// ------------------------------------------------------------------ the core

let core = null;
const engines = {};

// The core's browser glue (pkg/bm_web.js) is made by wasm-bindgen when the site is built; without it the page
// still opens, edits, saves text and shows every menu and dialog.
async function loadCore() {
  try {
    const m = await import("./pkg/bm_web.js");
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
async function templateText() {
  if (defaultTemplate === null) {
    try {
      const index = await (await fetch("templates/index.tsv")).text();
      const row = index
        .split("\n")
        .map((l) => l.split("\t"))
        .find((c) => (c[2] ?? "").trim() === "English (UEB) - basic");
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
      return localStorage.getItem(`braillemaker.${key}`) ?? memory.get(key) ?? null;
    } catch {
      return memory.get(key) ?? null;
    }
  },
  set(key, value) {
    memory.set(key, value);
    try {
      if (value === null) {
        localStorage.removeItem(`braillemaker.${key}`);
      } else {
        localStorage.setItem(`braillemaker.${key}`, value);
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
};

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

// New setups take the European order of default forms for a language other than US English (spec 06).
const european = () => !/^en-US$/i.test(navigator.language || "en-US");

// ------------------------------------------------------------------ the document

// Global settings, as the desktop program keeps them; the defaults are the reference program's (spec 18).
const DEFAULT_VIEWS = {
  translated_font: "print",
  print_full_page: false,
  print_codes: false,
  print_translated: false,
  braille_font: "print",
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
  verbose: store.get("verbose") === "1",
  sixKeySet: store.get("sixkey.set") || "fdsjkl",
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

// A new document takes the default embosser setup and layout, as in the desktop program.
function embosserDefaults() {
  const [layout, cells, lines] = defaultLayout();
  doc.embosser = defaultSetup();
  doc.layout = { ...layout };
  doc.cells = cells;
  doc.lines = lines;
}
embosserDefaults();

function kindName() {
  return doc.kind === "braille" ? "Braille Document" : "Print Document";
}

function showTitle() {
  const title = `BrailleMaker - [${doc.name} -- ${kindName()}]`;
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

// The braille fonts of the shown encoding ("BrailleMaker Braille North American English" and its SimBraille),
// loaded from fonts/ beside the page when first used.
const facesAdded = new Set();
function brailleFamily(kind) {
  const e = encodingNow();
  const family = `BrailleMaker ${kind} ${e.name}`;
  if (!facesAdded.has(family) && "FontFace" in window) {
    facesAdded.add(family);
    const face = new FontFace(family, `url("fonts/BrailleMaker-${kind}-${e.id}.ttf")`);
    document.fonts.add(face);
    face.load().catch(() => {});
  }
  return family;
}

// A view font's CSS family and point size (Global, Fonts).
function faceOf(font) {
  const f = settings.fonts;
  if (font === "print") return [`"${f.print_face}", monospace`, f.print_size];
  return [`"${brailleFamily(font === "braille" ? "Braille" : "SimBraille")}", monospace`, f.braille_size];
}

// The document's view applied: its font (sized so one whole page fits the window in Full-Page view), the translated
// line's font (Global, Default Views, for print documents; the print font for braille documents) and whether it
// shows, the status line's font, and the View menu's marks.
function applyView() {
  const v = doc.view;
  let [face, size] = faceOf(v.font);
  if (v.full_page) {
    const byHeight = Math.floor((editor.clientHeight * 72) / 96 / (doc.lines + 1));
    const byWidth = Math.floor((((editor.clientWidth * 72) / 96) * 10) / 6 / (doc.cells + 1));
    size = Math.max(4, Math.min(byHeight, byWidth));
  }
  editor.style.fontFamily = face;
  editor.style.fontSize = `${size}pt`;
  const [lineFace, lineSize] = faceOf(doc.kind === "print" ? settings.views.translated_font : "print");
  translated.style.fontFamily = lineFace;
  translated.style.fontSize = `${lineSize}pt`;
  $("translated-section").hidden = !v.translated;
  statusLine.style.fontFamily = `"${settings.fonts.status_face}", system-ui, sans-serif`;
  statusLine.style.fontSize = `${settings.fonts.status_size}pt`;
  refreshMenus();
  lastLine = null;
  showStatus();
  showTranslatedLine();
}

// The View menu's check marks follow the document's view.
function refreshMenus() {
  const v = doc.view;
  const marks = { 1301: v.full_page, 1303: v.font === "print", 1304: v.font === "braille", 1305: v.font === "simbraille", 1307: v.six_key, 1309: v.codes, 1310: v.translated };
  for (const [id, on] of Object.entries(marks)) {
    menubar.querySelector(`[data-id="${id}"]`)?.setAttribute("aria-checked", String(on));
  }
}

function caretLine() {
  const before = editor.value.slice(0, editor.selectionStart);
  const line = before.split("\n").length;
  const column = before.length - before.lastIndexOf("\n");
  return { line, column, text: editor.value.split("\n")[line - 1] ?? "" };
}

// The status line, as the desktop program shows it (bm_core::status): the current style, Page, Line and Column on the
// laid-out pages (left out in coded view), the character at the caret, and Keys.
function showStatus() {
  if (!core) {
    const { line, column } = caretLine();
    statusLine.textContent = `Current Style: None  Line:${line}  Column:${column}`;
    return;
  }
  // As in the desktop program, the text as it is shown (in the shown encoding) is laid out.
  const status = core.status_line(editor.value, doc.cells, doc.lines, doc.kind === "braille", editor.selectionStart, doc.view.codes, settings.verbose, doc.view.six_key);
  if (statusLine.textContent !== status) statusLine.textContent = status;
}

let lastLine = null;
async function showTranslatedLine() {
  if (!doc.view.translated) {
    return;
  }
  const { line, text } = caretLine();
  const key = `${line}:${text}`;
  if (key === lastLine) {
    return;
  }
  lastLine = key;
  const e = await engine(doc.kind === "print" ? "en-ueb-g2-forward" : "en-ueb-g2-back");
  if (!e) {
    translated.textContent = "";
    return;
  }
  try {
    translated.textContent = doc.kind === "print" ? fromNA(e.translate(text)) : e.translate(toNA(text));
  } catch (err) {
    translated.textContent = String(err.message ?? err);
  }
}

// Six-key entry (View, Six-Key Entry, braille documents): the six keys of Global, View Preferences are dots 1 to 6;
// the keys down together make a chord, entered as its cell when the last of them comes up.
const sixDown = new Set();
let chord = 0;
const sixKeyDot = (e) => (e.key.length === 1 ? settings.sixKeySet.indexOf(e.key.toLowerCase()) : -1);
editor.addEventListener("keydown", (e) => {
  if (doc.kind !== "braille" || !doc.view.six_key || e.ctrlKey || e.altKey || e.metaKey) return;
  const dot = sixKeyDot(e);
  if (dot >= 0) {
    e.preventDefault();
    sixDown.add(dot);
    chord |= 1 << dot;
  }
});
editor.addEventListener("keyup", (e) => {
  if (doc.kind !== "braille" || !doc.view.six_key) return;
  const dot = sixKeyDot(e);
  if (dot < 0) return;
  sixDown.delete(dot);
  if (sixDown.size === 0 && chord !== 0) {
    const c = cellChar(chord);
    chord = 0;
    editor.setRangeText(c, editor.selectionStart, editor.selectionEnd, "end");
    editor.dispatchEvent(new Event("input"));
  }
});

editor.addEventListener("input", () => {
  showStatus();
  showTranslatedLine();
});
editor.addEventListener("keyup", () => {
  showStatus();
  showTranslatedLine();
});
editor.addEventListener("click", showStatus);

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
  const m = /&([^&])/.exec(label ?? "");
  return m ? m[1].toUpperCase() : null;
}

// The controls of dialog or sheet page `rid` built into `form`, in the reference program's order: each group box a
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
        // A blank label (the reference fills some at run time) names nothing: the field is named by the text
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
        // A field with no label before it is named by its group, as a screen reader would read it there.
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
      resolve(dialog.returnValue === "ok" ? "ok" : "cancel");
      dialog.remove();
    });
  });
}

function focusFirst(container) {
  const first = [...container.querySelectorAll("input:not([type=hidden]), select, textarea, button")].find(
    (el) => !el.disabled && !el.closest("[hidden]"),
  );
  first?.focus();
}

// Dialog `rid` shown modally: `setup` fills it, `onOk` checks it. With `memory`, the values chosen last are put back
// after `setup` and kept on OK, as the desktop program's settings dialogs do. Answers how it was closed and its
// helpers.
async function showDialog(rid, { title, setup, onOk, memory, after } = {}) {
  const { dialog, form } = newDialog(title ?? DIALOGS[rid].title, `dlg${rid}`);
  const { controls, keys } = buildControls(rid, form);
  const d = dialogHelpers(controls);
  addButtons(rid, dialog, form, controls, onOk && (() => onOk(d)));
  wireKeys(dialog, () => keys);
  $("dialogs").append(dialog);
  const closed = closedPromise(dialog);
  setup?.(d);
  if (memory !== undefined) d.restore(store.json(`dialog.${rid}.${memory}`, null));
  after?.(d);
  dialog.showModal();
  focusFirst(form);
  const result = await closed;
  if (result === "ok" && memory !== undefined) store.set(`dialog.${rid}.${memory}`, JSON.stringify(d.values()));
  return { result, d };
}

// A message box (the desktop program's message and yes_no): OK, or Yes and No; Escape answers No.
async function messageBox(text, yesNo = false) {
  const { dialog, form } = newDialog("BrailleMaker", "message");
  dialog.setAttribute("role", "alertdialog");
  const p = document.createElement("p");
  p.className = "message";
  p.id = "message-text";
  p.textContent = text;
  dialog.setAttribute("aria-describedby", p.id);
  form.append(p);
  const buttons = document.createElement("div");
  buttons.className = "buttons";
  for (const [label, value] of yesNo ? [["Yes", "ok"], ["No", "cancel"]] : [["OK", "ok"]]) {
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
  return (await closed) === "ok";
}
const message = (text) => messageBox(text, false);
const yesNo = (text) => messageBox(text, true);

// The dialog a menu command opens: the one titled "Menu: Item" (as the reference program titles them).
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
// cancelled). Choosing a model fills the pages with that model's own values. The TranSend SE page shows only for
// models with TranSend (INFERENCE, as in the desktop program).
async function editSetup(setup, cat) {
  const lists = JSON.parse(core.setup_lists());
  const pages = [
    [10001, "General"],
    [10002, "Device Settings"],
  ];
  if (cat.flag(setup.model, "TranSend")) {
    pages.push([10003, "TranSend SE"]);
  }
  pages.push([10004, "Advanced"]);
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
  tablist.addEventListener("keydown", (e) => {
    const n = tabs.length;
    const to = { ArrowRight: current + 1, ArrowLeft: current - 1, Home: 0, End: n - 1 }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      showPage((to + n) % n);
    }
  });
  dialog.addEventListener("keydown", (e) => {
    if (e.ctrlKey && (e.key === "PageDown" || e.key === "PageUp")) {
      e.preventDefault();
      showPage((current + (e.key === "PageDown" ? 1 : tabs.length - 1)) % tabs.length);
    }
  });
  const sheetControls = new Map();
  const general = built[0].d;
  const device = built[1].d;
  const advanced = built.at(-1).d;
  const fill = (s, models) => fillSheet(general, device, advanced, s, cat, lists, models);
  // The reference places Configuration location (8, 9) and Default Form (10, 11) at the same spot and shows one:
  // Default Form (INFERENCE, as in the desktop program).
  general.hide(8);
  general.hide(9);
  fill(setup, true);
  general.get(5).addEventListener("change", () => {
    const model = general.selected(5) ?? "";
    if (!model.startsWith("(")) {
      fill(JSON.parse(cat.new_setup(model, european())), false);
    }
  });
  let answer = null;
  addButtons(10000, dialog, form, sheetControls, async () => {
    const model = general.selected(5) ?? "";
    if (model.startsWith("(")) {
      await message("Please select an embosser model.");
      return false;
    }
    const s = model === setup.model ? structuredClone(setup) : JSON.parse(cat.new_setup(model, european()));
    s.name = general.text(7).trim();
    if (setupNames().some((n) => n === shownName(s) && n !== shownName(setup))) {
      await message(`You already have a configuration named "${shownName(s)}".  Please give a different name.`);
      return false;
    }
    s.all_users = false;
    const forms = JSON.parse(cat.forms(model));
    const form = forms.find((f) => f.name === general.selected(11));
    if (form) s.form = form.id;
    const out = general.radio([13, 14, 15]);
    s.output =
      out === 0
        ? { Printer: (general.selected(16) ?? "").startsWith("(") ? "" : general.selected(16) ?? "" }
        : out === 2
          ? { File: general.text(18).trim() }
          : { Port: general.selected(17) ?? "lpt1" };
    s.serial_init = general.checked(21);
    s.baud = Number(general.selected(23)) || s.baud;
    s.parity = general.chosenIn(25, lists.parities) ?? s.parity;
    s.data_bits = Number(general.selected(27)) || s.data_bits;
    s.stop_bits = Number(general.selected(29)) || s.stop_bits;
    s.handshaking = general.chosenIn(31, lists.handshaking) ?? s.handshaking;
    s.version = device.chosenIn(5, lists.versions) ?? 0;
    s.escape_codes = device.checked(6);
    s.seek_top_of_form = device.checked(9);
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

// Shows setup `s` on the sheet's pages; the model list is filled only when `models` is set.
function fillSheet(general, device, advanced, s, cat, lists, models) {
  if (models) {
    const names = ["(Please select a model)", ...cat.names()];
    general.setItems(5, names, Math.max(0, names.indexOf(s.model)));
    general.setText(7, s.name);
  }
  const forms = JSON.parse(cat.forms(s.model));
  general.setItems(11, forms.map((f) => f.name), forms.findIndex((f) => f.id === s.form));
  general.enable(11, forms.length > 0);
  general.setItems(16, ["(default printer)"], 0);
  general.setItems(17, lists.ports, 0);
  const [kind, value] = Object.entries(s.output)[0];
  if (kind === "Printer") {
    general.setRadio([13, 14, 15], 0);
    if (value) general.setItems(16, [value], 0);
  } else if (kind === "Port") {
    general.setRadio([13, 14, 15], 1);
    const i = lists.ports.indexOf(value);
    if (i >= 0) general.setItems(17, lists.ports, i);
  } else {
    general.setRadio([13, 14, 15], 2);
    general.setText(18, value);
  }
  general.setChecked(21, s.serial_init);
  general.setItems(23, lists.bauds, lists.bauds.indexOf(String(s.baud)));
  general.setItems(25, lists.parities, Math.min(s.parity, 3));
  general.setItems(27, ["7", "8"], s.data_bits === 8 ? 1 : 0);
  general.setItems(29, ["1", "2"], s.stop_bits === 2 ? 1 : 0);
  general.setItems(31, lists.handshaking, Math.min(s.handshaking, 2));
  const has = (k) => cat.flag(s.model, k);
  const index = cat.driver(s.model).includes("Index");
  device.setItems(5, lists.versions, Math.min(s.version, lists.versions.length - 1));
  device.setChecked(6, s.escape_codes);
  device.setChecked(9, s.seek_top_of_form);
  device.setChecked(34, s.interpoint_enabled);
  let encodings = cat.encodings(s.model);
  if (!encodings.length) encodings = ["North American English"];
  device.setItems(37, encodings, Math.max(0, encodings.indexOf(s.encoding)));
  device.setItems(39, lists.cases, s.high_punctuation ? 1 : 0);
  device.setItems(41, lists.cases, s.upper_case ? 1 : 0);
  device.setChecked(44, s.reverse_order);
  device.setChecked(52, s.z_fold);
  for (const [cid, v] of s.device_other) device.setText(cid, v);
  // INFERENCE (as in the desktop program): the device fields shown for a model follow its properties.
  for (const [cid, on] of [
    [4, index],
    [5, index],
    [6, has("ETCAllowEscapeCodes") || has("UsingEscapeCodes")],
    [7, has("ETSpeaks")],
    [8, has("ETSpeaks")],
    [9, has("AllowSeekTOF")],
    [34, has("CanDoInterpoint")],
    [52, has("IndexZfold")],
  ]) {
    device.enable(cid, on);
  }
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
  const { result } = await showDialog(1032, {
    title: global ? "Global: Embosser Setup" : undefined,
    setup(d) {
      fillSetupList(d, setupName);
      fillSetupForms(d, cat, layout.form);
      d.setText(15, String(cells));
      d.setText(17, String(lines));
      d.setText(19, String(layout.top_margin));
      d.setText(21, String(layout.binding));
      d.setChecked(22, layout.interpoint);
      d.setChecked(23, layout.banner);
      d.setChecked(24, layout.interline !== 0);
      d.get(7).addEventListener("change", () => fillSetupForms(d, cat, loadSetup(d.selected(7))?.form ?? ""));
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
      const form = setupForms(cat, name).find((f) => f.name === d.selected(13));
      const field = (cid, current) => {
        const n = d.number(cid);
        return n !== null && n >= 0 ? n : current;
      };
      const [c, l, top, binding] = [field(15, cells), field(17, lines), field(19, layout.top_margin), field(21, layout.binding)];
      const interline = d.checked(24);
      // The page the form allows; with no embosser, 10 to 160 cells and 4 to 40 lines. Cells and binding margin
      // share the line, lines and top margin the page (spec 25 2c). INFERENCE: the least page with a form is one
      // cell and one line. The form's interline page is not given to the web build; its most is used.
      const [maxC, maxL, minC, minL] = form ? [form.cells, form.lines, 1, 1] : [160, 40, 10, 4];
      const binding2 = Math.min(binding, Math.max(0, maxC - minC));
      const c2 = Math.min(Math.max(c, minC), Math.max(maxC - binding2, minC));
      const top2 = Math.min(top, Math.max(0, maxL - minL));
      const l2 = Math.min(Math.max(l, minL), Math.max(maxL - top2, minL));
      if (c2 !== c || l2 !== l || top2 !== top || binding2 !== binding) {
        d.setText(15, String(c2));
        d.setText(17, String(l2));
        d.setText(19, String(top2));
        d.setText(21, String(binding2));
        await message(
          `BrailleMaker has updated desired document settings to fit the selected page size.\n\nCharacters per line: ${c2}\nLines per page: ${l2}\nTop margin in lines: ${top2}\nBinding margin in characters: ${binding2}`,
        );
        return false;
      }
      chosen = {
        name,
        layout: { form: form?.id ?? "", top_margin: top2, binding: binding2, interpoint: d.checked(22), banner: d.checked(23), interline: interline ? 1 : 0 },
        cells: c2,
        lines: l2,
      };
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
  // The two fields sit beside their radio buttons in the reference and have no label of their own: each is named
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

// The pages the page range group asks for (indexes, -1 for a blank page), or null when a message was shown and the
// dialog stays open; the caret and the selection are the editor's as the dialog opened.
async function chosenPages(d, radios, fields, braille, caret, from, to) {
  const count = core.page_count(editor.value, doc.cells, doc.lines, braille);
  const pageOf = (pos) => core.page_of(editor.value, doc.cells, doc.lines, braille, Math.max(0, pos));
  let range;
  switch (d.radio(radios) ?? 0) {
    case 0:
      range = { kind: "pages", pages: [...Array(count).keys()] };
      break;
    case 1:
      range = { kind: "pages", pages: [pageOf(caret) - 1] };
      break;
    case 2:
      // INFERENCE (as in the desktop program): an empty selection gives the no-pages message.
      if (from >= to) {
        range = { kind: "none" };
      } else {
        const first = pageOf(from);
        const last = pageOf(to - 1);
        range = { kind: "pages", pages: [...Array(last - first + 1).keys()].map((k) => first - 1 + k) };
      }
      break;
    case 3:
      range = JSON.parse(core.parse_range(d.text(fields[0]), count));
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
  return range.pages;
}

// ------------------------------------------------------------------ File Print (dialog 1040)

// The document's chosen pages printed through the browser's print dialog, `copies` times, a page per sheet; a
// braille document in the braille font, with each line's print under it when interline print is chosen.
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
      // On a braille document the page-number choice reads "Braille page number(s)": the reference copies control
      // 13's text into control 12 (spec 24, 2a); control 13 itself is not used.
      if (braille) d.get(12).closest("label").lastChild.textContent = d.get(13).closest("label").textContent;
      d.hide(13);
      d.setChecked(17, braille && store.get("print.interline") === "1");
      d.enable(17, braille);
    },
    async onOk(d) {
      const copies = await copiesField(d, 7);
      if (copies === null) return false;
      const pages = await chosenPages(d, PRINT_RANGE, [15, 16], braille, caret, from, to);
      if (pages === null) return false;
      job = { copies, pages, interline: braille && d.checked(17) };
      return true;
    },
  });
  if (result !== "ok") return;
  if (braille) store.set("print.interline", job.interline ? "1" : "0");
  const pages = JSON.parse(core.page_lines(editor.value, doc.cells, doc.lines, braille, Int32Array.from(job.pages)));
  const back = job.interline ? await engine("en-ueb-g2-back") : null;
  const area = $("print-area");
  area.replaceChildren();
  // The pages print in the document's view font, at the Global, Fonts printed size unless that is set to be found.
  area.style.fontFamily = faceOf(doc.view.font)[0];
  area.style.fontSize = settings.fonts.print_auto ? "" : `${doc.view.font === "print" ? settings.fonts.print_output_size : settings.fonts.braille_output_size}pt`;
  for (let copy = 0; copy < job.copies; copy++) {
    for (const lines of pages) {
      const page = document.createElement("section");
      page.className = "printed-page";
      for (const line of lines ?? []) {
        const row = document.createElement("div");
        row.className = "printed-line";
        row.textContent = line || " ";
        page.append(row);
        if (back && line.trim()) {
          const ink = document.createElement("div");
          ink.className = "printed-ink";
          ink.style.fontFamily = faceOf("print")[0];
          try {
            ink.textContent = back.translate(toNA(line.trimEnd()));
          } catch {
            ink.textContent = "";
          }
          page.append(ink);
        }
      }
      area.append(page);
    }
  }
  window.print();
  area.replaceChildren();
}

// ------------------------------------------------------------------ File Emboss (dialog 1039)

// Sends a job through Web Serial, the browser's serial port access, with the setup's serial settings.
// INFERENCE (web build): XON/XOFF handshaking has no Web Serial setting, so the port runs without handshaking then.
async function sendSerial(setup, bytes) {
  const port = await navigator.serial.requestPort();
  const options = { baudRate: setup.baud, dataBits: setup.data_bits, stopBits: setup.stop_bits };
  if (setup.serial_init) {
    options.parity = ["none", "even", "odd", "none"][setup.parity] ?? "none";
    options.flowControl = setup.handshaking === 0 ? "hardware" : "none";
  }
  await port.open(options);
  try {
    const writer = port.writable.getWriter();
    try {
      await writer.write(bytes);
    } finally {
      writer.releaseLock();
    }
  } finally {
    await port.close();
  }
}

// File, Emboss (dialog 1039, spec 25 2a): the checks before a job in the reference's order (spec 25 9.1), then the
// document's laid-out pages in the chosen range, `copies` times, through its setup's byte pipeline. A serial port
// goes through Web Serial where the browser has it; the browser reaches no printer queue, parallel port or USB
// port, so those jobs, and Write to File, are saved as a file.
async function emboss() {
  if (!core) {
    await message("Embossing is not available in this copy of the web build.");
    return;
  }
  // INFERENCE: a setup name that is no longer kept here is treated as no setup chosen.
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
      d.setText(23, shownName(setup));
      d.setText(25, core.formName(doc.layout.form));
    },
    async onOk(d) {
      const copies = await copiesField(d, 7);
      if (copies === null) return false;
      const pages = await chosenPages(d, EMBOSS_RANGE, [14, 15], braille, caret, from, to);
      if (pages === null) return false;
      job = { copies, pages };
      return true;
    },
  });
  if (result !== "ok") return;
  const bytes = cat.job(setupJson, layoutJson, storedText(), braille, doc.cells, doc.lines, job.copies, Int32Array.from(job.pages), await encodingCodes(setup.encoding));
  const [kind, target] = Object.entries(setup.output)[0];
  if (kind === "Port" && /^com/i.test(target) && "serial" in navigator) {
    try {
      await sendSerial(setup, bytes);
      return;
    } catch (err) {
      if (err?.name === "NotFoundError") return; // no port was chosen
      await message(String(err.message ?? err));
      return;
    }
  }
  const named = kind === "File" && target.trim() ? target.trim().split(/[\\/]/).pop() : `${stem()}.brl`;
  download(named, bytes);
  if (kind !== "File") {
    statusLine.textContent = `This browser cannot send to ${kind === "Printer" ? "a printer" : target}; the job was saved as ${named}.`;
  }
}

// ------------------------------------------------------------------ commands

const fileInput = $("file-input");
fileInput.addEventListener("change", async () => {
  const file = fileInput.files[0];
  fileInput.value = "";
  if (!file) {
    return;
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  const ext = file.name.split(".").pop().toLowerCase();
  embosserDefaults();
  if ((ext === "dxp" || ext === "dxb") && core) {
    try {
      const d = core.read_document(bytes);
      doc.kind = d.braille ? "braille" : "print";
      editor.value = d.braille ? fromNA(d.text) : d.text;
      doc.file = d;
      doc.saved = true;
      // The document's own embosser setup and page layout, as the file keeps them (spec 25 section 10); cells and
      // lines are taken by a braille document.
      if (d.embosser) doc.embosser = d.embosser;
      if (d.embosser_layout) {
        const l = JSON.parse(d.embosser_layout);
        doc.layout = l.layout;
        if (d.braille) {
          doc.cells = l.cells;
          doc.lines = l.lines;
        }
      }
    } catch (err) {
      statusLine.textContent = String(err.message ?? err);
      return;
    }
  } else {
    doc.kind = ext === "brf" || ext === "brl" ? "braille" : "print";
    const read = new TextDecoder("latin1").decode(bytes);
    editor.value = doc.kind === "braille" ? fromNA(read) : read;
    doc.file = null;
    doc.saved = false;
  }
  doc.name = file.name;
  // A document opens in its kind's default view; a document file keeps whether its translated line shows.
  doc.view = newView(doc.kind === "braille");
  if (doc.file) doc.view.translated = doc.file.translated_line;
  showTitle();
  editor.focus();
  applyView();
});

async function translateDocument() {
  const e = await engine(doc.kind === "print" ? "en-ueb-g2-forward" : "en-ueb-g2-back");
  if (!e) {
    statusLine.textContent = "Translation is not available in this copy of the web build.";
    return;
  }
  editor.value = doc.kind === "print" ? fromNA(e.translate(editor.value)) : e.translate(toNA(editor.value));
  doc.kind = doc.kind === "print" ? "braille" : "print";
  doc.name = `${stem()}${doc.kind === "braille" ? ".dxb" : ".dxp"}`;
  doc.file = null;
  doc.saved = false;
  doc.view = newView(doc.kind === "braille");
  showTitle();
  applyView();
}

// ------------------------------------------------------------------ File, Save and Save As

// The document's text as files keep it: braille in North American ASCII braille, whatever encoding it is shown in.
const storedText = () => (doc.kind === "braille" ? toNA(editor.value) : editor.value);

// The Save As file types, in the reference program's order for each kind of document (spec 04d-B); the first is
// chosen when the dialog opens, every time.
const SAVE_TYPES = {
  braille: [
    ["Braille document (*.dxb)", "dxb", "document"],
    ["Braille document, older format (*.dxb)", "dxb", "older"],
    ["Formatted braille (*.brf)", "brf", "formatted"],
    ["Coded braille (*.bru)", "bru", "coded"],
    ["Refreshable braille (*.lbf)", "lbf", "refreshable"],
  ],
  print: [
    ["Print document (*.dxp)", "dxp", "document"],
    ["Print document, older format (*.dxp)", "dxp", "older"],
    ["Formatted text (*.prf)", "prf", "formatted"],
    ["Coded text (*.txt)", "txt", "coded"],
    ["LaTeX (*.tex)", "tex", "latex"],
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

// A choice from a list, the first chosen (the desktop program's single choice dialog): its index, or null.
async function chooseFrom(title, prompt, items) {
  let list;
  const ok = await ownDialog(title, (form) => {
    list = labelledRow(form, "choice-list", prompt, document.createElement("select"));
    list.size = Math.min(10, items.length);
    list.replaceChildren(...items.map((t) => new Option(t, t)));
    list.selectedIndex = 0;
  });
  return ok && list.selectedIndex >= 0 ? list.selectedIndex : null;
}

// The document's file: the one it was read from or last saved as, else a new one from the default template.
async function documentFile() {
  return doc.file ?? core.new_document(doc.kind === "braille", editor.value, await templateText());
}

// Writes the document as a document file (.dxp or .dxb) in format `version` (1, or 0 for the older format), with
// its embosser setup name and layout, then takes that file as its own.
async function writeDocument(name, version) {
  const file = await documentFile();
  const bytes = file.save_version(storedText(), doc.embosser, doc.layout.form ? JSON.stringify(doc.layout) : "", doc.cells, doc.lines, version);
  download(name, bytes);
  doc.file = core.read_document(bytes);
  doc.name = name;
  doc.saved = true;
  showTitle();
}

// Formatted braille (.brf): the encoding of the file chosen from "12.4 Compatible (CAPS)", the national encodings
// and "Unicode Braille Patterns", the first chosen every time (spec 04d-B (a), (f)). Formatted text (.prf): the code
// page chosen (spec 04d-E). Answers the bytes, or null when cancelled.
async function formattedBytes(braille) {
  if (!braille) {
    const pages = core.code_pages();
    const i = await chooseFrom("Formatted text", "Code page of the file:", pages);
    return i === null ? null : core.formatted_text(editor.value, doc.cells, doc.lines, pages[i]);
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
  return core.formatted_braille(editor.value, doc.cells, doc.lines, JSON.stringify(shownCells), Uint32Array.from(codes));
}

// File, Save As: the name and type chosen, then the file written as that type and saved by the browser. A name
// typed with the extension of another type gives the first type with that extension.
async function saveAs() {
  const braille = doc.kind === "braille";
  const types = SAVE_TYPES[braille ? "braille" : "print"];
  let nameField;
  let typeField;
  const ok = await ownDialog("Save As", (form) => {
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
      default:
        bytes = core.latex(
          editor.value,
          new TextDecoder().decode(await fetchBytes("tables/table-txxtxa-binary-forward.bmt")),
          await fetchBytes("tables/rule-scripts/latex.rules"),
        );
    }
    // The document itself keeps its own file and name.
    if (bytes) download(name, bytes);
  } catch (err) {
    await message(String(err.message ?? err));
  }
}

// File, Save: a document read from or saved as a document file (.dxp, .dxb) is saved as that file again; any other
// is asked about (Save As).
async function save(asNew) {
  if (!core) {
    download(`${stem()}.txt`, editor.value, "text/plain");
    return;
  }
  const own = doc.kind === "braille" ? "dxb" : "dxp";
  if (!asNew && doc.saved && doc.name.toLowerCase().endsWith(`.${own}`)) {
    await writeDocument(doc.name, 1);
    return;
  }
  await saveAs();
}

// ------------------------------------------------------------------ View and Global

function setView(change) {
  change(doc.view);
  applyView();
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
      d.setRadio([11, 12], 0);
      d.setChecked(14, settings.uia);
      d.setRadio([15, 16], 0);
    },
  });
  if (result !== "ok") return;
  settings.uia = d.checked(14);
  store.set("uia", settings.uia ? "1" : "0");
  const chosen = all.find((e) => e.name === d.selected(10));
  if (!chosen || chosen.id === current) return;
  if (doc.kind === "braille" && core) {
    editor.value = core.convert_braille(editor.value, JSON.stringify(encodingNow().cells), JSON.stringify(chosen.cells));
  }
  settings.encoding = chosen.id;
  store.set("encoding", chosen.id);
  currentEncoding = chosen;
  applyView();
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
// Unicode value) and the six keys of six-key entry.
async function viewPreferences() {
  const { result, d } = await showDialog(1034, {
    memory: "",
    setup(d) {
      d.setText(7, "0");
      d.setChecked(9, settings.verbose);
      d.setItems(17, ["Hide All", "Hide Idle", "Show All"], 0);
      d.setText(19, settings.sixKeySet);
    },
  });
  if (result !== "ok") return;
  settings.verbose = d.checked(9);
  store.set("verbose", settings.verbose ? "1" : "0");
  showStatus();
  const keys = d.text(19);
  if ([...keys].length !== 6) {
    await message(
      'You must type exactly six keys in as the "Six-Key Key Set."  The keys you type represent, in order, what you will type for Dots 1, 2, 3, 4, 5, and 6 when entering braille by six-key chording.',
    );
    return;
  }
  settings.sixKeySet = keys.toLowerCase();
  store.set("sixkey.set", settings.sixKeySet);
}

// Global, Shortcut Preferences (dialog 1022): the codes the editor's shortcut keys insert and the page keys' moves.
async function shortcutPreferences() {
  const s = settings.shortcuts;
  let chosen = null;
  await showDialog(1022, {
    setup(d) {
      d.setText(7, s.grade0);
      d.setText(9, s.flush_right_leader);
      d.setText(11, s.cbc);
      d.setText(13, s.new_line);
      d.setText(15, s.new_page);
      d.setText(17, s.new_ref_page);
      d.setRadio([19, 20, 21], Math.min(s.page_nav, 2));
      d.setRadio([23, 24, 25], Math.min(s.control_page_nav, 2));
    },
    onOk(d) {
      chosen = {
        grade0: d.text(7),
        flush_right_leader: d.text(9),
        cbc: d.text(11),
        new_line: d.text(13),
        new_page: d.text(15),
        new_ref_page: d.text(17),
        page_nav: d.radio([19, 20, 21]) ?? s.page_nav,
        control_page_nav: d.radio([23, 24, 25]) ?? s.control_page_nav,
      };
      return true;
    },
  });
  if (!chosen) return;
  settings.shortcuts = chosen;
  store.set("shortcuts", JSON.stringify(chosen));
}

const COMMANDS = {
  1101: () => {
    doc.kind = "print";
    doc.name = "Untitled Print Document 1";
    doc.file = null;
    doc.saved = false;
    embosserDefaults();
    doc.view = newView(false);
    editor.value = "";
    showTitle();
    editor.focus();
    applyView();
  },
  1102: () => fileInput.click(),
  1105: () => save(false),
  1106: () => save(true),
  1109: translateDocument,
  1111: print,
  1112: emboss,
  1301: () => setView((v) => (v.full_page = !v.full_page)),
  1303: () => setView((v) => (v.font = "print")),
  1304: () => setView((v) => (v.font = "braille")),
  1305: () => setView((v) => (v.font = "simbraille")),
  1307: async () => {
    if (doc.kind !== "braille" && !doc.view.six_key) {
      await message("Six-key entry is for braille documents.");
      return;
    }
    setView((v) => (v.six_key = !v.six_key));
  },
  1309: () => setView((v) => (v.codes = !v.codes)),
  1310: () => setView((v) => (v.translated = !v.translated)),
  2601: () => embosserSetup(false),
  3101: () => embosserSetup(true),
  3104: () => showDialog(1060, { memory: "", setup: (d) => (d.setChecked(10, true), d.setText(12, ""), d.setText(14, "05:00")) }),
  3105: defaultViews,
  3106: internationalization,
  3107: globalFonts,
  3108: viewPreferences,
  3109: shortcutPreferences,
  3111: () => showDialog(1023),
  3113: () =>
    showDialog(1037, {
      memory: "",
      setup(d) {
        d.setItems(7, ["Text Only", "URLs Only", "Both Text and URLs", "Structured Hyperlinks"], 0);
        d.setItems(23, ["Mark with asterisk (*)", "Ignore", "Output Unicode value"], 0);
        d.setChecked(26, true);
      },
    }),
  3114: () => showDialog(1035, { memory: "", setup: (d) => (d.setChecked(13, true), d.setText(16, "0.25"), d.setChecked(18, true)) }),
  3115: () =>
    showDialog(1036, {
      memory: "",
      setup: (d) => d.setItems(10, ["Ignore", "Mark from Word document", "Treat hard page breaks as print page breaks", "Use publishers' <ipp no=#> reference page tags"], 0),
    }),
  3116: () => message("The PDF importer and its options are built in step 9 of the plan."),
  3117: () =>
    showDialog(1045, {
      memory: "",
      setup(d) {
        d.setRadio([8, 9, 10, 11, 12], 2);
        d.setRadio([14, 15, 16, 17, 18], 4);
        d.setRadio([20, 21, 22, 23, 24], 0);
        d.setRadio([26, 27, 28, 29, 30], 0);
        d.setRadio([32, 33, 34, 35], 0);
        d.setRadio([37, 38, 39, 40], 0);
        d.setText(42, "25");
        d.setText(44, "40");
      },
      // "When this is checked, most of the other controls on the dialog are disabled" (the manual).
      after(d) {
        const apply = () => {
          for (let c = 7; c <= 44; c++) d.enable(c, !d.checked(6));
        };
        d.get(6)?.addEventListener("change", apply);
        apply();
      },
    }),
  3211: () => showDialog(1016),
};

async function run(id, menuLabel, itemLabel) {
  closeMenus();
  if (COMMANDS[id]) {
    await COMMANDS[id]();
  } else {
    const rid = dialogFor(menuLabel, itemLabel);
    if (rid) {
      await showDialog(rid);
    } else {
      statusLine.textContent = `${plain(itemLabel)} is not in the web build yet.`;
    }
  }
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
  const at = label.search(/&[^&]/);
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
      item.setAttribute("aria-keyshortcuts", entry.shortcut.replace(/\s/g, ""));
    }
    item.addEventListener("click", () => run(entry.id, menuLabel, entry.label));
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
  if (!menu) {
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
      else if (entry.shortcut) SHORTCUTS.set(entry.shortcut.toLowerCase(), [entry.id, m.label, entry.label]);
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
  if (hit && !menubar.contains(e.target)) {
    e.preventDefault();
    run(...hit);
  }
});

// ------------------------------------------------------------------ start

showTitle();
showStatus();
applyView();
loadCore().then(async () => {
  await loadEncoding();
  applyView();
  if (core) {
    await embosserSetupAtStart();
  }
});
