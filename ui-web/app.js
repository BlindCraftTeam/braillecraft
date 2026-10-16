// BrailleMaker web build: the desktop program's menus and dialogs (ui-web/tables.js, generated from the desktop's own
// tables), a document editor with the translated line and status line, and the core (bm-web, WebAssembly) for
// documents, translation and embossing.

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

// ------------------------------------------------------------------ the document

const doc = { kind: "print", name: "Untitled Print Document 1", file: null, translatedLine: true };

function kindName() {
  return doc.kind === "braille" ? "Braille Document" : "Print Document";
}

function showTitle() {
  const title = `BrailleMaker - [${doc.name} -- ${kindName()}]`;
  $("doc-title").textContent = title;
  document.title = title;
  editor.classList.toggle("braille", doc.kind === "braille");
  translated.classList.toggle("braille", doc.kind === "print");
}

function caretLine() {
  const before = editor.value.slice(0, editor.selectionStart);
  const line = before.split("\n").length;
  const column = before.length - before.lastIndexOf("\n");
  return { line, column, text: editor.value.split("\n")[line - 1] ?? "" };
}

function showStatus() {
  const { line, column } = caretLine();
  statusLine.textContent = `Current Style: None  Line:${line}  Column:${column}`;
}

let lastLine = null;
async function showTranslatedLine() {
  if (!doc.translatedLine) {
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
    translated.textContent = e.translate(text);
  } catch (err) {
    translated.textContent = String(err.message ?? err);
  }
}

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

// Builds dialog `rid` as a modal dialog: its controls in the reference program's order (OK and Cancel last), each
// group box a fieldset around the controls inside it, each label tied to the control after it, radio buttons in
// their groups. Returns the dialog and a promise of how it was closed.
function buildDialog(rid, title) {
  const spec = DIALOGS[rid];
  const dialog = document.createElement("dialog");
  const heading = document.createElement("h2");
  heading.id = `dlg${rid}-title`;
  heading.textContent = title ?? spec.title;
  dialog.setAttribute("aria-labelledby", heading.id);
  dialog.append(heading);
  const form = document.createElement("form");
  form.method = "dialog";
  dialog.append(form);
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
      if (pendingLabel) {
        const label = document.createElement("label");
        label.htmlFor = id;
        label.textContent = plain(pendingLabel.label);
        row.append(label);
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
  const buttons = document.createElement("div");
  buttons.className = "buttons";
  for (const c of spec.controls.filter((c) => c.id === 1 || c.id === 2)) {
    const button = document.createElement("button");
    button.textContent = plain(c.label);
    if (c.id === 1) {
      button.type = "submit";
      button.value = "ok";
      button.className = "default";
    } else {
      button.type = "button";
      button.addEventListener("click", () => dialog.close("cancel"));
    }
    buttons.append(button);
    controls.set(c.id, button);
  }
  form.append(buttons);
  // Alt with a label's letter moves to the control after the label, as in the desktop program.
  for (const fs of fieldsets.values()) {
    const first = fs.querySelector("input, select, button");
    if (fs.dataset.mnemonic && first) {
      keys.push([fs.dataset.mnemonic, first]);
    }
  }
  dialog.addEventListener("keydown", (e) => {
    if (e.altKey && !e.ctrlKey && e.key.length === 1) {
      const hit = keys.find(([k]) => k === e.key.toUpperCase());
      if (hit) {
        e.preventDefault();
        hit[1].focus();
      }
    }
  });
  $("dialogs").append(dialog);
  const closed = new Promise((resolve) => {
    dialog.addEventListener("close", () => {
      resolve(dialog.returnValue === "ok" ? "ok" : "cancel");
      dialog.remove();
    });
  });
  return { dialog, controls, closed };
}

async function showDialog(rid, title, setup) {
  const d = buildDialog(rid, title);
  setup?.(d);
  d.dialog.showModal();
  const first = d.dialog.querySelector("input:not([type=hidden]):not(:disabled), select:not(:disabled), textarea");
  first?.focus();
  return { result: await d.closed, d };
}

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
  if ((ext === "dxp" || ext === "dxb") && core) {
    try {
      const d = core.read_document(bytes);
      doc.kind = d.braille ? "braille" : "print";
      editor.value = d.text;
      doc.file = d;
    } catch (err) {
      statusLine.textContent = String(err.message ?? err);
      return;
    }
  } else {
    doc.kind = ext === "brf" || ext === "brl" ? "braille" : "print";
    editor.value = new TextDecoder("latin1").decode(bytes);
    doc.file = null;
  }
  doc.name = file.name;
  showTitle();
  lastLine = null;
  editor.focus();
  showStatus();
  showTranslatedLine();
});

async function translateDocument() {
  const e = await engine(doc.kind === "print" ? "en-ueb-g2-forward" : "en-ueb-g2-back");
  if (!e) {
    statusLine.textContent = "Translation is not available in this copy of the web build.";
    return;
  }
  editor.value = e.translate(editor.value);
  doc.kind = doc.kind === "print" ? "braille" : "print";
  doc.name = `${stem()}${doc.kind === "braille" ? ".dxb" : ".dxp"}`;
  doc.file = null;
  showTitle();
  lastLine = null;
  showTranslatedLine();
}

function save(asBrf) {
  if (asBrf && core) {
    download(`${stem()}.brf`, core.layout_braille(editor.value, 40, 25));
  } else if (doc.file) {
    download(doc.name, doc.file.save(editor.value));
  } else {
    download(`${stem()}.txt`, editor.value, "text/plain");
  }
}

async function emboss() {
  const { result, d } = await showDialog(1039, null, (d) => {
    d.controls.get(7).value = "1";
    d.controls.get(9).checked = true;
  });
  if (result !== "ok") {
    return;
  }
  const copies = Number(d.controls.get(7).value);
  if (!Number.isInteger(copies) || copies < 1) {
    statusLine.textContent = "You must print or emboss at least 1 copy, or press Cancel to avoid printing any.";
    return;
  }
  if (!core) {
    statusLine.textContent = "Embossing is not available in this copy of the web build.";
    return;
  }
  download(`${stem()}.brl`, core.emboss_job(editor.value, 40, 25, copies));
}

const COMMANDS = {
  1101: () => {
    doc.kind = "print";
    doc.name = "Untitled Print Document 1";
    doc.file = null;
    editor.value = "";
    showTitle();
    editor.focus();
  },
  1102: () => fileInput.click(),
  1105: () => save(false),
  1106: () => save(doc.kind === "braille"),
  1109: translateDocument,
  1111: () => window.print(),
  1112: emboss,
  1310: () => {
    doc.translatedLine = !doc.translatedLine;
    $("translated-section").hidden = !doc.translatedLine;
    lastLine = null;
    showTranslatedLine();
  },
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
  const kind = entry.kind === "check" ? "menuitemcheckbox" : entry.kind === "radio" ? "menuitemradio" : "menuitem";
  item.setAttribute("role", kind);
  if (kind !== "menuitem") {
    item.setAttribute("aria-checked", entry.id === 1310 ? "true" : "false");
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
loadCore().then(() => {
  lastLine = null;
  showTranslatedLine();
});
