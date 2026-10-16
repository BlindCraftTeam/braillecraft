// BrailleCraft dialogs. Each entry follows the decoded dialog of the same purpose: the same title, controls, labels, groups, order and buttons. One renderer builds them all as modal <dialog>
// elements with real labels, fieldsets for group boxes and Alt+letter access keys, as in a Windows dialog.
"use strict";

// Control kinds: text, num, codes (a codes field), check, radio (a group box of radio buttons), group (a group box
// holding other controls), list (a list box), combo (a drop-down list), button (a push button inside the dialog),
// static (text). "&" marks the access key.
const OK_CANCEL = [["ok", "OK"], ["cancel", "Cancel"]];
const PLACEMENT = [["none", "None"], ["ul", "Upper Left"], ["ur", "Upper Right"], ["ll", "Lower Left"], ["lr", "Lower Right"]];
const PAGE_RANGE = [["all", "&All"], ["current", "Curr&ent page"], ["selection", "Se&lection"]];
const FORMS = ["11 x 11.5 inches", "11 x 8.5 inches (Letter)", "A4 (297 x 210 mm)", "11.5 x 11 inches (tractor)"];
const STYLE_SHOW = ["Recommended and In-Use Styles", "All Styles", "Styles In-Use", "Only Recommended Styles"];

const DIALOGS = {
  fileNew: { title: "File: New...", width: 380, controls: [
    { k: "radio", id: "kind", label: "Document Type:", options: [["print", "&Print"], ["braille", "&Braille"]], value: "print" },
    { k: "list", id: "template", label: "Document Settings Template:", items: [], size: 8 },
    { k: "button", id: "markDefault", label: "&Mark template as default" },
    { k: "button", id: "hide", label: "&Hide Template" },
    { k: "button", id: "showAlways", label: "&Show Template Always" },
    { k: "button", id: "region", label: "Select Region..." },
    { k: "check", id: "showHidden", label: "Show Hidden Templates" },
  ] },
  selectRegion: { title: "Select Language Region", width: 380, controls: [
    { k: "list", id: "region", label: "Display Templates from which Region:", items: ["Western Hemisphere", "Europe", "Asia", "Africa", "Pacific", "All Languages/All Regions"], size: 6 },
  ] },
  goto: { title: "Edit: Goto...", width: 240, controls: [
    { k: "num", id: "page", label: "Page Number:", min: 1 },
    { k: "check", id: "reference", label: "Use reference page number" },
    { k: "num", id: "line", label: "Line Number:", min: 1 },
    { k: "num", id: "column", label: "Column Number:", min: 1 },
  ] },
  find: { title: "Edit: Find...", width: 470, controls: [
    { k: "codes", id: "find", label: "Text to find:" },
    { k: "check", id: "matchCase", label: "Case sensitive" },
    { k: "check", id: "backward", label: "Backward search" },
    { k: "check", id: "wholeWords", label: "Find whole words only" },
    { k: "radio", id: "mode", label: "Advanced options", options: [["literal", "Find literal"], ["style", "Find stylename"], ["wildcard", "Allow wildcard (*) at end of codes"]], value: "literal" },
  ] },
  replace: { title: "Edit: Replace...", width: 470, controls: [
    { k: "codes", id: "find", label: "Text to find:" },
    { k: "codes", id: "replaceWith", label: "Replace with:" },
    { k: "check", id: "matchCase", label: "Case sensitive" },
    { k: "check", id: "backward", label: "Backward search" },
    { k: "check", id: "wholeWords", label: "Find whole words only" },
    { k: "radio", id: "mode", label: "Advanced options", options: [["literal", "Replace literal"], ["style", "Replace stylename"], ["wildcard", "Allow wildcard (*) at end of codes"]], value: "literal" },
  ] },
  confirmReplace: { title: "Confirm Replacement", width: 280, controls: [
    { k: "static", text: "Replace?" },
  ], buttons: [["yes", "&Yes"], ["no", "&No"], ["all", "&All"], ["cancel", "&Stop"]] },
  shortcutPreferences: { title: "Shortcut Preferences", width: 340, controls: [
    { k: "codes", id: "grade0", label: "Insert for Grade &0 (Alt-0):", value: "[cz]" },
    { k: "codes", id: "flushRightLeader", label: "Insert for Flush &Right with Leader (Alt-F7):", value: "[fr;p~\"]" },
    { k: "codes", id: "cbc", label: "Insert for &Computer Braille (Alt-3)", value: "[cb]" },
    { k: "codes", id: "newLine", label: "Insert for new &line (Enter)", value: "[<]" },
    { k: "codes", id: "newPage", label: "Insert for new &page (CTRL-Enter)", value: "[pg]" },
    { k: "codes", id: "newRefPage", label: "Insert for &new reference page (Alt-Enter)", value: "[lea]" },
    { k: "radio", id: "pageNav", label: "Page Up and Page Down Navigation", options: [["screen", "Move by One Screen"], ["page", "Move by One Page"], ["pageTop", "Move by One Page (to Page Top)"]], value: "screen" },
    { k: "radio", id: "pageNavCtrl", label: "Page Up and Page Down Navigation (with Control)", options: [["screen", "Move by One Screen"], ["page", "Move by One Page"], ["pageTop", "Move by One Page (to Page Top)"]], value: "page" },
  ] },
  notificationSettings: { title: "Notification Settings", width: 360, controls: [
    { k: "static", text: "After you click OK below, any warnings or other notifications previously hidden with a \"Don't show me this again\" or similar control will be shown once more. Use Cancel to leave these notifications hidden." },
  ] },
  addStyle: { title: "Document: Add Style...", width: 460, controls: [
    { k: "text", id: "name", label: "New Style Name:", value: "New Style Name" },
    { k: "check", id: "nested", label: "Style can be nested within another style" },
    { k: "list", id: "basis", label: "Choose Style for basis:", items: [], size: 6 },
    { k: "combo", id: "show", label: "Show:", items: STYLE_SHOW },
    { k: "codes", id: "begin", label: "Beginning Codes:" },
    { k: "codes", id: "end", label: "Ending Codes:" },
  ] },
  modifyStyle: { title: "Document: Modify Style...", width: 460, controls: [
    { k: "button", id: "rename", label: "Rename style..." },
    { k: "list", id: "style", label: "Style to Modify:", items: [], size: 6 },
    { k: "combo", id: "show", label: "Show:", items: STYLE_SHOW },
    { k: "codes", id: "begin", label: "Beginning Codes:" },
    { k: "codes", id: "end", label: "Ending Codes:" },
  ] },
  renameStyle: { title: "Rename Style...", width: 350, controls: [
    { k: "text", id: "name", label: "Enter a new name for the style:" },
    { k: "check", id: "replaceInstances", label: "Replace existing style instances", checked: true },
  ] },
  applyStyle: { title: "Apply Style...", width: 380, controls: [
    { k: "list", id: "style", label: "Choose a style to apply:", items: [], size: 8 },
    { k: "combo", id: "show", label: "Show:", items: STYLE_SHOW },
  ] },
  deleteStyle: { title: "Delete Style...", width: 380, controls: [
    { k: "list", id: "style", label: "Choose a style to delete:", items: [], size: 8 },
    { k: "combo", id: "show", label: "Show:", items: STYLE_SHOW },
  ] },
  createTemplate: { title: "Document: Create Template...", width: 370, controls: [
    { k: "static", text: "Create a template with current document settings" },
    { k: "text", id: "name", label: "Create New Template:" },
    { k: "list", id: "replace", label: "Replace Existing Template:", items: [], size: 5 },
    { k: "check", id: "printer", label: "Include printer setup" },
    { k: "check", id: "embosser", label: "Include embosser setup" },
    { k: "check", id: "text", label: "Include text and codes" },
    { k: "check", id: "default", label: "Set template as default" },
    { k: "check", id: "wordMap", label: "Use customized Word style map" },
    { k: "list", id: "wordStyleMap", label: "Word style map:", items: [], size: 3 },
    { k: "button", id: "toc", label: "Table of Contents Options" },
    { k: "radio", id: "share", label: "Template use", options: [["me", "Save for Me Only"], ["shared", "Share with Others"]], value: "me" },
  ] },
  deleteTemplate: { title: "Delete Template...", width: 380, controls: [
    { k: "list", id: "template", label: "Choose a template to delete:", items: [], size: 8 },
  ] },
  header: { title: "Layout: Header...", width: 320, controls: [
    { k: "codes", id: "text", label: "Header text:" },
    { k: "radio", id: "placement", label: "Header Placement:", options: [["all", "All Pages"], ["odd", "Odd Pages"], ["even", "Even Pages"], ["none", "No Header"]], value: "all" },
  ] },
  footer: { title: "Layout: Footer...", width: 320, controls: [
    { k: "codes", id: "text", label: "Footer text:" },
    { k: "radio", id: "placement", label: "Footer Placement:", options: [["all", "All Pages"], ["odd", "Odd Pages"], ["even", "Even Pages"], ["none", "No Footer"]], value: "all" },
  ] },
  pageNumbering: { title: "Document: Page Numbering...", width: 490, columns: 2, controls: [
    { k: "radio", id: "even", label: "Page Numbering for Even Pages:", options: PLACEMENT, value: "ur" },
    { k: "radio", id: "odd", label: "Page Numbering for Odd Pages:", options: PLACEMENT, value: "ur" },
    { k: "radio", id: "refEven", label: "Reference Page Numbering for Even Pages:", options: PLACEMENT, value: "none" },
    { k: "radio", id: "refOdd", label: "Reference Page Numbering for Odd Pages:", options: PLACEMENT, value: "none" },
    { k: "num", id: "first", label: "First page number to display:", value: 1, min: 1 },
  ] },
  embosserSetup: { title: "Document: Embosser Setup", width: 400, controls: [
    { k: "group", label: "Brailler &Device:", controls: [
      { k: "list", id: "device", label: "Brailler devices", items: ["(No Target Embosser)"], size: 5 },
      { k: "button", id: "new", label: "&New" },
      { k: "button", id: "modify", label: "Modify &Embosser" },
      { k: "button", id: "remove", label: "&Remove" },
    ] },
    { k: "group", label: "Desired Braille Document Formatting", controls: [
      { k: "combo", id: "form", label: "&Form:", items: FORMS },
      { k: "num", id: "cells", label: "&Characters per line:", value: 40, min: 10, max: 100 },
      { k: "num", id: "lines", label: "&Lines per page:", value: 25, min: 5, max: 100 },
      { k: "num", id: "top", label: "&Top margin in lines:", value: 0, min: 0, max: 20 },
      { k: "num", id: "binding", label: "&Binding margin in characters:", value: 0, min: 0, max: 20 },
    ] },
    { k: "check", id: "interpoint", label: "Emboss in &Interpoint" },
    { k: "check", id: "banner", label: "Emboss Banner Page" },
    { k: "check", id: "interline", label: "Interline &Print" },
  ] },
  printerSetup: { title: "Document: Printer Setup...", width: 380, controls: [
    { k: "group", label: "Paper", controls: [
      { k: "combo", id: "size", label: "Si&ze", items: ["Letter (8.5 x 11 inches)", "Legal (8.5 x 14 inches)", "A4 (210 x 297 mm)"] },
      { k: "combo", id: "source", label: "&Source", items: ["Automatically Select"] },
    ] },
    { k: "radio", id: "orientation", label: "Orientation", options: [["portrait", "P&ortrait"], ["landscape", "L&andscape"]], value: "portrait" },
    { k: "group", label: "Margins", controls: [
      { k: "text", id: "left", label: "&Left:", value: "1\"" },
      { k: "text", id: "right", label: "&Right:", value: "1\"" },
      { k: "text", id: "top", label: "&Top", value: "1\"" },
      { k: "text", id: "bottom", label: "&Bottom", value: "1\"" },
    ] },
    { k: "group", label: "Formatted Page Size", controls: [
      { k: "num", id: "lines", label: "L&ines/page", value: 50, min: 5, max: 160 },
      { k: "num", id: "chars", label: "&Characters/line", value: 80, min: 10, max: 100 },
    ] },
    { k: "button", id: "printer", label: "&Printer..." },
  ] },
  defaultViews: { title: "Global: Default Views...", width: 300, controls: [
    { k: "radio", id: "translatedFont", label: "Font for translated braille line:", options: [["print", "Print"], ["braille", "Braille"], ["simbraille", "SimBraille"]], value: "braille" },
    { k: "check", id: "printFullPage", label: "Show full page in print documents" },
    { k: "check", id: "printCodes", label: "Show codes in print documents" },
    { k: "check", id: "printTranslated", label: "Show translated line in print documents" },
    { k: "radio", id: "brailleFont", label: "Default font for braille documents:", options: [["print", "Print"], ["braille", "Braille"], ["simbraille", "SimBraille"]], value: "print" },
    { k: "check", id: "brailleFullPage", label: "Show full page in braille documents" },
    { k: "check", id: "brailleCodes", label: "Show codes in braille documents" },
    { k: "check", id: "brailleTranslated", label: "Show translated line in braille documents" },
    { k: "check", id: "sixKey", label: "Six-key entry in braille documents" },
  ] },
  viewPreferences: { title: "Global, View Preferences...", width: 360, controls: [
    { k: "num", id: "bell", label: "Margin bell position:", value: 0, min: 0, max: 100 },
    { k: "static", text: "(Set a margin bell position of 0 to disable the margin bell.)" },
    { k: "check", id: "verbose", label: "Use verbose labels for non-ANSI characters in coded view" },
    { k: "check", id: "lineNumbers", label: "Show line &numbers in formatted braille documents" },
    { k: "group", label: "Show Cursor Locator", controls: [
      { k: "check", id: "locOpenTranslated", label: "When document &opened or translated" },
      { k: "check", id: "locOpened", label: "When document &opened" },
      { k: "check", id: "locViews", label: "When switching &views" },
      { k: "check", id: "locShift", label: "When &shift is pushed" },
    ] },
    { k: "combo", id: "translatedCodes", label: "Formatted View Translated Line Codes:", items: ["Hide All", "Hide Idle", "Show All"] },
    { k: "text", id: "sixKeySet", label: "Six-Key &Key Set:", value: "fdsjkl" },
  ] },
  wordPerfectImporter: { title: "Global: WordPerfect Importer...", width: 300, controls: [
    { k: "check", id: "courier", label: "Transcribe Courier to CBC" },
    { k: "check", id: "hardBreaks", label: "Preserve hard page breaks" },
    { k: "check", id: "compress", label: "Compress Skipped Lines" },
    { k: "check", id: "markBreaks", label: "Mark print page breaks" },
    { k: "radio", id: "paragraph", label: "Paragraph Style:", options: [["none", "None"], ["skip", "Skipped Line"], ["newlineTab", "Newline Tab"], ["skipTab", "Skipped Line with Tab"]], value: "newlineTab" },
    { k: "text", id: "indent", label: "Indent one braille cell per (inches/cm):", value: "0.25" },
    { k: "radio", id: "units", label: "Units", options: [["in", "Inches"], ["cm", "Centimeters"]], value: "in" },
  ] },
  wordImporter: { title: "Global: Word Importer...", width: 420, controls: [
    { k: "check", id: "courier", label: "Transcribe Courier to CBC" },
    { k: "check", id: "hardBreaks", label: "Preserve hard page breaks" },
    { k: "check", id: "skipped", label: "Preserve Skipped Lines" },
    { k: "combo", id: "pageBreaks", label: "Print Page Breaks:", items: ["Ignore", "Mark from Word document", "Treat hard page breaks as print page breaks", "Use publishers' <ipp no=#> reference page tags"] },
    { k: "check", id: "brailleFonts", label: "Treat braille and simbraille fonts as braille" },
    { k: "check", id: "embedded", label: "Allow embedded codes, eg. [[*pg*]]" },
    { k: "group", label: "Ignore:", controls: [
      { k: "check", id: "ignoreBold", label: "bold" },
      { k: "check", id: "ignoreItalics", label: "italics" },
      { k: "check", id: "ignoreUnderline", label: "underline" },
      { k: "check", id: "ignoreLanguage", label: "language switches" },
    ] },
  ] },
  importOptions: { title: "Global: Import Options...", width: 500, controls: [
    { k: "combo", id: "hyperlinks", label: "Import from Hyperlinks:", items: ["Text Only", "URLs Only", "Both Text and URLs", "Structured Hyperlinks"] },
    { k: "check", id: "dedupe", label: "Remove duplicates" },
    { k: "check", id: "autoTag", label: "Auto-&Tag URLs as Computer Notation" },
    { k: "check", id: "mathSpaces", label: "Add spaces around math signs of comparison" },
    { k: "check", id: "dashSpaces", label: "Remove spaces surrounding dashes" },
    { k: "combo", id: "unknown", label: "Unknown characters:", items: ["Mark with asterisk (*)", "Ignore", "Output Unicode value"] },
    { k: "check", id: "hanTemplate", label: "Choose Han (Chinese) script based on template selected for import" },
    { k: "radio", id: "han", label: "Default language for Han (Chinese) script:", options: [["pinyin", "Pinyin (Mandarin)"], ["cantonese", "Cantonese (Yue)"], ["japanese", "Japanese"], ["korean", "Korean"], ["zhuyin", "Zhuyin (Mandarin)"]], value: "pinyin" },
  ] },
  formattedBrailleImporter: { title: "Global: Formatted Braille Importer...", width: 490, columns: 2, controls: [
    { k: "check", id: "raw", label: "Read formatted braille without interpretation" },
    { k: "radio", id: "bEven", label: "Braille Page Numbering for Even Pages of Input File:", options: PLACEMENT, value: "ur" },
    { k: "radio", id: "bOdd", label: "Braille Page Numbering for Odd Pages of Input File:", options: PLACEMENT, value: "lr" },
    { k: "radio", id: "pEven", label: "Print Page Numbering for Even Pages of Input File:", options: PLACEMENT, value: "none" },
    { k: "radio", id: "pOdd", label: "Print Page Numbering for Odd Pages of Input File:", options: PLACEMENT, value: "none" },
    { k: "radio", id: "header", label: "Header Placement:", options: [["none", "No Header"], ["odd", "Odd Pages"], ["even", "Even Pages"], ["all", "All Pages"]], value: "none" },
    { k: "radio", id: "footer", label: "Footer Placement:", options: [["none", "No Footer"], ["odd", "Odd Pages"], ["even", "Even Pages"], ["all", "All Pages"]], value: "none" },
    { k: "num", id: "lines", label: "Maximum lines per input page:", value: 25, min: 1, max: 100 },
    { k: "num", id: "cells", label: "Cells per line on input page:", value: 40, min: 1, max: 100 },
  ] },
  emboss: { title: "File: Emboss...", width: 360, controls: [
    { k: "num", id: "copies", label: "&Copies:", value: 1, min: 1, max: 999 },
    { k: "radio", id: "range", label: "Page range", options: [...PAGE_RANGE, ["braille", "&Braille page number(s)"], ["sequence", "Page sequence &number(s)"]], value: "all" },
    { k: "text", id: "pages", label: "Pages:" },
    { k: "static", id: "device", text: "Brailler Device: (No Target Embosser)" },
    { k: "static", id: "form", text: "Form: 11 x 11.5 inches" },
  ] },
  print: { title: "File: Print...", width: 360, controls: [
    { k: "num", id: "copies", label: "&Copies:", value: 1, min: 1, max: 999 },
    { k: "radio", id: "range", label: "Page range", options: [...PAGE_RANGE, ["print", "&Print page number(s)"], ["braille", "&Braille page number(s)"], ["sequence", "Page sequence &number(s)"]], value: "all" },
    { k: "text", id: "pages", label: "Pages:" },
    { k: "check", id: "interline", label: "Interline Print" },
  ] },
  codeList: { title: "Layout: Code List...", width: 500, controls: [
    { k: "group", label: "Code categories", controls: [
      { k: "check", id: "all", label: "All", checked: true },
      { k: "check", id: "page", label: "Page" },
      { k: "check", id: "line", label: "Line" },
      { k: "check", id: "tab", label: "Tab" },
      { k: "check", id: "european", label: "European" },
      { k: "check", id: "greek", label: "Greek Letters" },
      { k: "check", id: "cbc", label: "CBC" },
      { k: "check", id: "math", label: "Math" },
      { k: "check", id: "misc", label: "Miscellaneous" },
    ] },
    { k: "list", id: "code", label: "Codes:", items: [], size: 10 },
    { k: "static", id: "chosen", text: "Chosen Code: (none)" },
    { k: "text", id: "parameters", label: "Code Parameters:" },
  ], buttons: [["ok", "OK"], ["cancel", "Cancel"], ["help", "Help"]] },
  setPageNumber: { title: "Layout: Set Page Number...", width: 260, controls: [
    { k: "num", id: "page", label: "Page number for next page:", value: 1, min: 1 },
    { k: "radio", id: "style", label: "Page Numbering Style:", options: [["arabic", "Arabic"], ["roman", "Roman"]], value: "arabic" },
  ] },
  importFile: { title: "Import File", width: 400, controls: [
    { k: "list", id: "template", label: "Template", items: [], size: 6 },
    { k: "button", id: "setDefault", label: "&Set template as default" },
    { k: "button", id: "hide", label: "&Hide Template" },
    { k: "button", id: "showAlways", label: "&Show Template Always" },
    { k: "button", id: "region", label: "Select Region..." },
    { k: "check", id: "showHidden", label: "Show Hidden Templates" },
    { k: "list", id: "filter", label: "Import Filter", items: ["Formatted braille", "Formatted text", "OpenXML / Word 2007-2019, 365", "Portable Document Format (PDF)", "SGML/ICADD/HTML"], size: 5 },
    { k: "combo", id: "codePage", label: "Code Page (Character Set):", items: ["Unicode (UTF-8)", "Windows ANSI", "IBM ASCII"] },
  ] },
  editStyleCodes: { title: "Edit Style Codes", width: 450, controls: [
    { k: "static", id: "name", text: "Name" },
    { k: "codes", id: "begin", label: "Beginning Codes:" },
    { k: "codes", id: "end", label: "Ending Codes:" },
    { k: "static", text: "Press F5 for a list of codes." },
    { k: "static", text: "Press Ctrl-[ to insert a code manually." },
  ] },
  fonts: { title: "Global: Fonts...", width: 360, controls: [
    { k: "list", id: "printFont", label: "Print font:", items: ["Courier New", "Consolas", "Arial", "Times New Roman", "Verdana"], size: 5 },
    { k: "num", id: "printSize", label: "Print font point size for screen display:", value: 12, min: 6, max: 72 },
    { k: "num", id: "brailleSize", label: "Braille/SimBraille font point size for screen display:", value: 18, min: 6, max: 72 },
    { k: "check", id: "autoSize", label: "Auto-determine point size for printed output", checked: true },
    { k: "num", id: "printOutSize", label: "Print font point size for printed output:", value: 12, min: 6, max: 72 },
    { k: "num", id: "brailleOutSize", label: "Braille/SimBraille font point size for printed output:", value: 18, min: 6, max: 72 },
    { k: "list", id: "statusFont", label: "Status Line font:", items: ["Segoe UI", "Arial", "Courier New", "Verdana"], size: 4 },
    { k: "num", id: "statusSize", label: "Status Line font size:", value: 9, min: 6, max: 36 },
  ] },
  spellCheck: { title: "Spell Check...", width: 480, controls: [
    { k: "static", id: "checking", text: "Checking:" },
    { k: "static", id: "word", text: "Unknown word:" },
    { k: "radio", id: "action", label: "Options:", options: [["ignore", "&Ignore once"], ["ignoreAll", "Ignore &all"], ["add", "Add to &dictionary"], ["replace", "&Replace"]], value: "ignore" },
    { k: "text", id: "replaceWith", label: "Replace with:" },
    { k: "list", id: "suggestions", label: "Suggested replacements:", items: [], size: 5 },
  ] },
  internationalization: { title: "Internationalization", width: 320, controls: [
    { k: "list", id: "uiLanguage", label: "Language for menus and dialogs", items: ["English"], size: 4 },
    { k: "group", label: "Braille Encoding for Input and Display", controls: [
      { k: "list", id: "encoding", label: "Local/National Encoding:", items: ["North American ASCII Braille"], size: 3 },
      { k: "radio", id: "display", label: "Braille display", options: [["local", "Use Local/National Encoding for all Braille Display"], ["unicode", "Prefer Unicode Braille Patterns for Braille Display"]], value: "local" },
      { k: "static", text: "(Local/National Encoding will be used for braille displayed in Print Font)" },
    ] },
    { k: "check", id: "uia", label: "Support UI Automation", checked: true },
    { k: "radio", id: "screenReaders", label: "Braille reported to screen readers", options: [["local", "Report Braille to Screen Readers in Local/National Encoding"], ["unicode", "Report Braille to Screen Readers as Unicode Braille Patterns"]], value: "local" },
  ] },
  tableOfContents: { title: "Table of Contents", width: 500, controls: [
    { k: "combo", id: "style", label: "Style to Use:", items: ["Contents", "Chapter", "Page", "Custom"] },
    { k: "check", id: "suspend", label: "Suspend Reference Numbering for Contents Pages" },
    { k: "check", id: "removeBreaks", label: "Remove Internal Line Breaks within Entries" },
    { k: "num", id: "levels", label: "Include Levels One Through:", value: 3, min: 1, max: 9 },
    { k: "check", id: "centerTop", label: "Center Top Level" },
    { k: "check", id: "numberTop", label: "Number Top Level" },
    { k: "group", label: "Single Level", controls: [
      { k: "num", id: "sLeft", label: "Left Margin (Single Level):", value: 1, min: 1 },
      { k: "num", id: "sRunover", label: "Runover Indent (Single Level):", value: 3, min: 1 },
      { k: "num", id: "sRight", label: "Right Margin (Single Level):", value: 0, min: 0 },
    ] },
    { k: "group", label: "Multi-Level", controls: [
      { k: "num", id: "mLeft", label: "Left Margin for Level One:", value: 1, min: 1 },
      { k: "num", id: "mIncrement", label: "Left Margin Increment:", value: 2, min: 0 },
      { k: "num", id: "mRunover", label: "Runover Indent:", value: 2, min: 0 },
      { k: "check", id: "maxRunover", label: "Always Use Maximum Runover" },
      { k: "num", id: "mRight", label: "Right Margin:", value: 0, min: 0 },
    ] },
    { k: "codes", id: "centered", label: "Centered Heading:", value: "Contents" },
    { k: "codes", id: "leftHead", label: "Left Column Heading:" },
    { k: "codes", id: "rightHead", label: "Right Column Heading:", value: "Page" },
    { k: "combo", id: "guideDots", label: "Character for Guide Dots:", items: ["Dot 5", "Dot 3", "Dot 6", "Dots 3-6"] },
    { k: "combo", id: "placeNumbers", label: "Place Numbers:", items: ["Right-Aligned", "In Last %d Cells"] },
    { k: "combo", id: "use", label: "Use:", items: ["Braille Page Numbers", "Print Page Numbers"] },
    { k: "codes", id: "endCodes", label: "End Codes:" },
  ] },
  fileLocations: { title: "File Locations", width: 500, controls: [
    { k: "text", id: "open", label: "File open location:" },
    { k: "text", id: "save", label: "File save location:" },
    { k: "check", id: "autosave", label: "Enable autosave", checked: true },
    { k: "text", id: "autosaveFolder", label: "Autosaved file location:" },
    { k: "text", id: "autosaveEvery", label: "Autosave every (mm:ss):", value: "05:00" },
  ] },
  tableProperties: { title: "Table Properties", width: 340, controls: [
    { k: "list", id: "type", label: "Table type:", items: ["Table: [Automatic]", "Table: Columnar", "Table: Stairstep", "Table: Listed", "Table: Linear", "Table: Two-Page"], size: 6 },
    { k: "combo", id: "swap", label: "X/Y (Row/Column) Swap:", items: ["Do Not Swap", "Force Swap", "Swap if More Compact"] },
    { k: "num", id: "rows", label: "Total Rows:", value: 2, min: 1 },
    { k: "num", id: "headerRows", label: "Header rows:", value: 1, min: 0 },
    { k: "num", id: "columns", label: "Total Columns:", value: 2, min: 1 },
    { k: "num", id: "headerColumns", label: "Header columns:", value: 0, min: 0 },
  ] },
  transcribersNote: { title: "Insert Transcriber's Note", width: 340, controls: [
    { k: "text", id: "note", label: "Note text:" },
  ] },
  message: { title: "BrailleCraft", width: 340, controls: [
    { k: "static", id: "text", text: "" },
  ], buttons: [["ok", "OK"]] },
  yesNo: { title: "BrailleCraft", width: 340, controls: [
    { k: "static", id: "text", text: "" },
  ], buttons: [["yes", "&Yes"], ["no", "&No"]] },
  saveChanges: { title: "BrailleCraft", width: 340, controls: [
    { k: "static", id: "text", text: "" },
  ], buttons: [["yes", "&Yes"], ["no", "&No"], ["cancel", "Cancel"]] },
  about: { title: "About BrailleCraft", width: 380, controls: [
    { k: "static", text: "BrailleCraft for Windows, Linux, Mac and the web" },
    { k: "static", text: "Version 0.1.0" },
    { k: "static", text: "Copyright (c) 2026 Jeremiah Beasley." },
    { k: "static", text: "Braille translation by liblouis (LGPL), to be added in build stage 2." },
  ], buttons: [["ok", "OK"]] },
};

// Builds the label text: removes "&" and marks the access key letter. Returns the key, lower case, or "".
function labelInto(node, text) {
  const i = text.indexOf("&");
  if (i < 0 || i === text.length - 1) { node.textContent = text; return ""; }
  node.append(text.slice(0, i));
  const key = document.createElement("span");
  key.className = "mnemonic";
  key.textContent = text[i + 1];
  node.append(key, text.slice(i + 2));
  return text[i + 1].toLowerCase();
}

function plain(text) { return text.replace("&", ""); }

let uid = 0;

function buildControl(c, prefix, parts, keys) {
  const id = `${prefix}-${c.id || ++uid}`;
  const row = document.createElement("div");
  row.className = `field field-${c.k}`;
  const addKey = (key, target) => { if (key) keys[key] = target; };

  if (c.k === "static") {
    const p = document.createElement("p");
    p.className = "static";
    p.id = id;
    p.textContent = c.text;
    parts[c.id || id] = p;
    return p;
  }
  if (c.k === "group" || c.k === "radio") {
    const set = document.createElement("fieldset");
    const legend = document.createElement("legend");
    const key = labelInto(legend, c.label);
    set.append(legend);
    if (c.k === "group") {
      let first = null;
      for (const child of c.controls) {
        const el = buildControl(child, prefix, parts, keys);
        set.append(el);
        first = first || el.querySelector("input, select, button");
      }
      addKey(key, first);
    } else {
      let first = null;
      for (const [value, text] of c.options) {
        const optId = `${id}-${value}`;
        const lab = document.createElement("label");
        lab.className = "choice";
        lab.htmlFor = optId;
        const input = document.createElement("input");
        input.type = "radio";
        input.id = optId;
        input.name = id;
        input.value = value;
        input.checked = value === c.value;
        const span = document.createElement("span");
        addKey(labelInto(span, text), input);
        lab.append(input, span);
        set.append(lab);
        first = first || input;
      }
      addKey(key, first);
      parts[c.id] = { radios: set };
    }
    return set;
  }
  if (c.k === "check") {
    const lab = document.createElement("label");
    lab.className = "choice";
    lab.htmlFor = id;
    const input = document.createElement("input");
    input.type = "checkbox";
    input.id = id;
    input.checked = !!c.checked;
    const span = document.createElement("span");
    addKey(labelInto(span, c.label), input);
    lab.append(input, span);
    parts[c.id] = input;
    return lab;
  }
  if (c.k === "button") {
    const b = document.createElement("button");
    b.type = "button";
    b.id = id;
    b.className = "inline-button";
    addKey(labelInto(b, c.label), b);
    parts[c.id] = b;
    return b;
  }
  // Labelled fields: text, num, codes, list, combo.
  const lab = document.createElement("label");
  lab.htmlFor = id;
  const key = labelInto(lab, c.label);
  let field;
  if (c.k === "list" || c.k === "combo") {
    field = document.createElement("select");
    if (c.k === "list") field.size = c.size || 5;
    for (const item of c.items) {
      const o = document.createElement("option");
      o.textContent = item;
      field.append(o);
    }
    if (c.items.length) field.selectedIndex = 0;
  } else {
    field = document.createElement("input");
    field.type = c.k === "num" ? "number" : "text";
    if (c.k === "codes") { field.className = "codes"; field.spellcheck = false; }
    if (c.min !== undefined) field.min = c.min;
    if (c.max !== undefined) field.max = c.max;
    if (c.value !== undefined) field.value = c.value;
  }
  field.id = id;
  addKey(key, field);
  parts[c.id] = field;
  row.append(lab, field);
  return row;
}

function valueOf(part) {
  if (!part) return undefined;
  if (part.radios) {
    const on = part.radios.querySelector("input:checked");
    return on ? on.value : null;
  }
  if (part.type === "checkbox") return part.checked;
  if (part.tagName === "SELECT") return part.value;
  if (part.tagName === "INPUT") return part.type === "number" ? Number(part.value) : part.value;
  return undefined;
}

// Opens a dialog and resolves with { button, values } when it closes. "setup" receives the controls by id before
// the dialog is shown (to fill lists, set values, wire in-dialog buttons). Escape resolves as "cancel".
function showDialog(name, setup) {
  const spec = DIALOGS[name];
  const prefix = `dlg-${name}`;
  const dialog = document.createElement("dialog");
  dialog.className = "win-dialog";
  dialog.style.width = `min(${spec.width}px, calc(100vw - 32px))`;
  dialog.setAttribute("aria-labelledby", `${prefix}-title`);

  const caption = document.createElement("h2");
  caption.className = "caption";
  caption.id = `${prefix}-title`;
  caption.textContent = spec.title;

  const form = document.createElement("form");
  form.method = "dialog";
  const body = document.createElement("div");
  body.className = spec.columns === 2 ? "body two-columns" : "body";
  const parts = {};
  const keys = {};
  for (const c of spec.controls) body.append(buildControl(c, prefix, parts, keys));

  const row = document.createElement("div");
  row.className = "buttons";
  for (const [value, text] of spec.buttons || OK_CANCEL) {
    const b = document.createElement("button");
    b.type = "submit";
    b.value = value;
    if (value === "cancel" || value === "no" || value === "help") b.formNoValidate = true;
    if (value === "ok" || value === "yes") b.className = "default-button";
    keys[labelInto(b, text) || "\0"] = b;
    row.append(b);
  }
  form.append(body, row);
  dialog.append(caption, form);
  document.body.append(dialog);

  // Alt plus the underlined letter moves to that control, or presses that button, as in Windows dialogs.
  dialog.addEventListener("keydown", (event) => {
    if (!event.altKey || event.ctrlKey || event.metaKey) return;
    const target = keys[event.key.toLowerCase()];
    if (!target) return;
    event.preventDefault();
    target.focus();
    if (target.type === "radio" || target.type === "checkbox" || target.tagName === "BUTTON") target.click();
  });

  const describe = spec.controls.find((c) => c.k === "static" && c.id === "text");
  if (describe) dialog.setAttribute("aria-describedby", `${prefix}-text`);

  if (setup) setup(parts, dialog);

  // Focus goes back where it was when the dialog closes, and a text field gets its caret, selection and scroll
  // back: closing a modal dialog otherwise leaves the caret at the start of the text in Chromium.
  const returnTo = document.activeElement;
  const textField = returnTo && (returnTo.tagName === "TEXTAREA" || returnTo.tagName === "INPUT") ? returnTo : null;
  const saved = textField ? [textField.selectionStart, textField.selectionEnd, textField.selectionDirection, textField.scrollTop, textField.value] : null;

  return new Promise((resolve) => {
    dialog.addEventListener("close", () => {
      const values = {};
      for (const [id, part] of Object.entries(parts)) {
        const v = valueOf(part);
        if (v !== undefined) values[id] = v;
      }
      const button = dialog.returnValue || "cancel";
      dialog.remove();
      if (returnTo && returnTo.isConnected && !returnTo.disabled) {
        returnTo.focus();
        if (saved && textField.value === saved[4]) {
          textField.setSelectionRange(saved[0], saved[1], saved[2]);
          textField.scrollTop = saved[3];
        }
      }
      resolve({ button, values });
    }, { once: true });
    dialog.returnValue = "";
    dialog.showModal();
    const first = body.querySelector("input:not([type=radio]):not([disabled]), select, input[type=radio]:checked, textarea")
      || row.querySelector("button");
    if (first) first.focus();
  });
}

// A message box, for notes and warnings.
async function messageBox(text, title = "BrailleCraft") {
  await showDialog("message", (parts, dialog) => {
    parts.text.textContent = text;
    dialog.querySelector(".caption").textContent = title;
  });
}
