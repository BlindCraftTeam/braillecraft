# BrailleCraft

BrailleCraft is made by the BlindCraft Team. It is a braille translation, formatting and embossing program for Windows, Linux and macOS, with the same
program in a web browser. You write or open a print document, translate it to braille (or a braille document back to
print), lay it out in braille pages, and print or emboss it. Every screen is built to work with a screen reader and
the keyboard alone, and to meet WCAG 2.2 AA.

## Screenshots

![The BrailleCraft window with a print document holding two lines of text. Below the text, the translated line
shows the first line in contracted braille, written in braille ASCII. The status line under it reads Current Style:
None, Page 1, Line 1, Column 51.](docs/screenshots/desktop-print-document.png)

*A print document. The translated line under the text shows the braille for the line the cursor is on.*

![The same text after File, Translate: a braille document shown in the braille font as six-dot braille cells, with
the print text of the current line in the translated line below it.](docs/screenshots/desktop-braille-document.png)

*The braille document made by File, Translate, shown in the braille font.*

![The File New dialog: Document Type with Print selected and Braille, a list of document settings templates
starting with English (UEB) basic, the buttons Mark template as default, Hide Template and Select Region, a Show
Hidden Templates check box, and OK and Cancel.](docs/screenshots/desktop-file-new.png)

*File, New: each new document starts from a template that sets its language and braille code.*

![The Embosser Setup dialog: a Brailler Device list with No Target Embosser, the buttons New, Modify Embosser and
Remove, and Desired Braille Document Formatting with Form, 40 characters per line, 25 lines per page, top and
binding margins, and check boxes for Emboss in Interpoint, Emboss Banner Page and Interline
Print.](docs/screenshots/desktop-embosser-setup.png)

*Embosser Setup opens at the first start, when no embosser is set up yet.*

![BrailleCraft in a web browser: the same menu bar (File, Edit, View, Layout, Table, Document, Global, Help), a print
document with two lines of text, its translated line in braille ASCII, and the status
line.](docs/screenshots/web-editor.png)

*The web build: the same program in a browser.*

![The web build with the File menu open: New, Open, Close, Save, Save as, Save eBraille, Translate, Print, Emboss and
Exit, each with its keyboard shortcut.](docs/screenshots/web-file-menu.png)

*The web build's File menu, with the same items and shortcuts as the desktop program.*

## What it does

- **Documents.** Print documents and braille documents, each in its own window, with formatting codes and styles
  kept apart from the text. Opens and saves .dxp (print) and .dxb (braille) documents, and opens text and braille
  files (.txt, .brf, .brl), PEF (.pef) files and PDF files. A PDF becomes a print document: a tagged PDF through
  its structure (headings, paragraphs, lists, tables as styles and codes), any other PDF from its page layout
  (paragraphs, line breaks, and ledger tables of dates, text and amounts).
- **Formatted and coded views.** The editing area is drawn by BrailleCraft itself, as in the original program, and
  its caret is a place in the document, not in the text on the screen. A document opens in the formatted view: its
  laid-out page lines with the codes hidden and a line across after each page. The caret can stand on a space the
  line does not show (at the end of a line) or on an empty last line, and what you type goes in there. You type,
  delete, cut, paste and undo (100 steps) there as anywhere; a code next to the caret is removed whole, and the caret
  goes over a code at once. View, Codes (Alt+F3) switches to the coded view,
  the running text with every code in rows of 42 characters (65 in a print document), and back, with the caret kept
  in place. In the coded view each code is drawn in a smaller print font inside a rounded box, whatever the document's
  font, as in the original: a style begin as `<name>`, a style end as `</name>`, a hard space as `[']`, a marked
  hyphen as `[-]`, any other code in its brackets. Control+Page Up and
  Control+Page Down go to the same line on the previous or next page (Global, Shortcut Preferences sets what Page Up
  and Page Down do).
- **Translation.** Print to braille and braille to print with 657 translation tables for 188 languages and braille
  codes, chosen by region and language (Document, Translation Tables) or by learning course (Learning Tables).
  Language codes in the text switch tables in the middle of a document. Every character the built-in character
  table names translates (accented letters, £, €, œ, dashes, curly quotes and thousands more); File, Open (its file types
  as the original program lists them, All Files chosen, starting in the folder last opened from) finds a
  file's kind from its name and content (formatted text, formatted braille, coded text, coded braille, PDF, PEF, a
  document file) and, for any file that is not a document file, asks in the import dialog for its template, its
  kind, and the code page or braille encoding to read it in; Insert File does the same and refuses braille in a print
  document and print in a braille one. A document keeps the translation tables its file names. Formatted braille, coded braille, Orbit Reader braille (.obf), coded text and
  formatted text are imported the way the original program imports them (formatted text is read for its page
  layout: centred and left headings, paragraphs, poems and outlines, tables, letters' salutations and closes, running
  heads and page numbers become styles and codes, and its characters go through the character map tables) (an Orbit file's line ends become line codes): formatted braille is read page by page, with the page numbers in its corners taken
  as page numbering, running heads and feet, print page changes, indented, centred and as-is lines turned back into
  codes; then format words ($p, |IND5 and the like), line ends, hyphens at line ends and spaces are read as the import
  scan tables say, and the import rule scripts finish the codes, tables and Nemeth passages. Word documents (.docx)
  are imported the way the original program imports them: the document's paragraphs, runs, styles, lists, tables,
  footnotes, endnotes, hyperlinks, page breaks and fields go through the Word style map the template names
  (default, Australian, BANA, French, Russian or UK basic, shipped as data) and the Word import rule scripts, with
  the Global, Word Importer settings (Courier as computer braille, hard page breaks, skipped lines, print page
  breaks, braille fonts, embedded codes, bold, italics, underline and language switches ignored) and the Global,
  Import Options settings (hyperlinks as text, URLs, both or hyperlink codes, duplicates removed, URLs tagged as
  computer notation, spaces around signs of comparison, spaces around dashes, unknown characters). The web build opens files
  the same way, through the same import dialog and import steps, with its settings kept in the browser.
- **Translated line.** Under the document, the braille (or the print) of the line you are on, updated as you write.
- **View preferences.** A margin bell a chosen number of cells before the end of a braille line, which codes the
  translated line shows, line numbers beside each line of a braille document in the formatted view when
  that setting is on, and the Cursor Locator: a rectangle that closes in on the caret when a document is opened or
  translated, when the view changes, or when Shift is pressed alone.
- **Braille pages.** Lines per page, cells per line, margins, centred headings and page numbering, shown in the
  status line as page, line and column. A page number shares the first or last line with text, which keeps three
  cells clear of it; print documents get page numbers too, and [pg] codes can renumber pages. Each document's styles (headings, lists, paragraphs) are laid out from its
  own style list. Running heads go on the first lines of the pages they are set for and running feet on the last
  line, centred or at a side, beside the page numbers. Blocks kept together (a heading with the lines after it, a list
  item) move to the next page whole rather than break across one. Print page numbers marked in the text show in their corner,
  with a continuation letter on later braille pages, and a print page change line where a print page begins in the
  middle of a braille page.
- **Layout commands as the original.** Header and Footer place their text on all, odd or even pages (or end it), Set
  Page Number and Document, Page Numbering check the number in the dialog, and Apply Last Style applies the paragraph
  style last ended before the caret. A value a dialog refuses gets its message there, with what was typed kept.
  Delete Style and changing a style's codes ask first, and the last style cannot be deleted; Clear (Del) with nothing
  selected deletes the character at the caret, and Cut ends Highlight. Shortcut Preferences checks its codes in the
  dialog, Notification Settings brings back every message hidden with "do not show again", and a new hyphen table
  lays the document out again. Words are hyphenated at line ends as the original does, by the document's Hyphen
  Table (None, Dashes, Dashes and Hyphens, French, French 113; the tables ship as data): at a break marked in the
  text, after or before a dash, after an internal hyphen, or (French, in braille documents) at a syllable break the
  table's automaton finds, with a hyphen inserted; a word longer than a line with no break point is cut at the line
  end; a computer braille word ([wb-cb]) breaks after punctuation and carries the continuation sign. Change Case works on the word at the caret when nothing is selected, and steps
  lower case to a capital on each word to upper case and back. Six-key entry (F2) works in print documents too. Goto checks its numbers in the dialog,
  offers the closest location when a place has no text, and goes to a print page with Use reference page number. Add Style
  checks the name in the dialog, then opens Edit Style Codes for the codes, and a style that cannot be nested gets
  its period. Rename Style checks the new name in the dialog. Code List (F5) lists all 300 codes by category,
  remembers the categories chosen, and takes a double-click as OK. A paragraph style applied takes whole paragraphs, Join Lines and
  Join Styles work on the selection, and New Paragraph inside a style ends it and starts it again. Style Level +
  and - (Alt+Right, Alt+Left) raise or lower the hierarchy level code [hlN] of the paragraph style at the caret, up
  to the most levels the [hi] code allows, and renumber every run of level codes in a selection; inside a table they
  move to the next or previous cell of the row; a failure beeps.
- **Formatter.** The braille layout follows the original's codes: tab stops ([stb], [#], [>], [ctb]), [tas] and [taa],
  [lps] and [lpr], the decimal character, the left-margin adjustment [svlma], the relative form of [hi] and its
  paragraph values, [skn], page parity [sd], magazine paragraphs [pm], anticipatory heads and print numbers [svant],
  the word-break modes [wb-no] and [wb-cb], and a print page beginning on the last lines of a page.
- **Web build.** File, New offers the template list with Mark as Default, Hide, Show Template Always, Region and Show
  Hidden Templates, kept in the browser as the desktop program keeps them; the import dialog has the same list; a
  PEF file opens straight into a braille document.
- **Find and Replace as the original.** Each document keeps its own Find and Replace. A search looks in the
  selection first, then offers the rest of the document and the other end of it; it can search for codes, for a
  style by its name (Replace renames a style's codes, or takes them out), or with a `*` at the end of a code standing
  for the rest of its name. Find leaves the caret at what it finds. Replace asks Yes, No, All or Stop for each match,
  each replacement is one undo step and All is one together, the caret and selection go back where they were, and
  the number of replacements is shown (with "do not show again").
- **Tables as the original.** Table, Create puts a new table at the caret: its type (Unrelated Columns, Matrix, or
  one of the six table formats), swap and header rows and columns in the table's start code, and its rows of empty
  cells, with the caret in the first cell. With the caret in a table, Create and Table Properties change that table's
  type, swap and headers. A selection holding a table's start or end code is refused. The formatter lays tables out
  as the original does: the cells from the start code to the end code are measured and laid in columns (header rows
  underlined, guide dots to the next column, the table repeated with its header rows on every page it runs on, or
  split across two pages when asked), as a stairstep, as a list (each cell under its column heading) or as a linear
  paragraph (cells separated by colons and semicolons), chosen by the start code or automatically (columnar, then
  listed, then linear), in the BANA or the RNIB style, with the swapped orientation when allowed; a table the
  layout cannot fit is shown as its text with the reason in the status line (Failsafe), and inside a table the
  status line names the table's layout, row and column.
- **Global, Fonts and File, New.** Fonts lists the fonts installed on the computer (typewriter fonts first for the
  print font) and checks each point size, 5 to 180, in the dialog; the printed-output sizes are greyed while their
  size is set automatically. File, New remembers Show Hidden Templates. Global, Internationalization keeps its two
  Unicode choices: braille shown in a braille font can be written as Unicode braille patterns, and on Windows a
  braille document's text can be given to screen readers (UI Automation) as Unicode braille patterns. The web
  build shows the patterns too (in the editor and the translated line, the document keeping its own characters);
  there the text screen readers read is the text shown.
- **Save As.** Formatted braille (BRF in a choice of braille encodings), PEF (Portable Embosser Format), formatted
  text in a choice of code pages (the encoding or code page chosen in the Save As dialog's own Encoding box; each
  Windows code page written and read by Windows' own table, on every system), coded braille, refreshable braille (.lbf), eBraille (.ebrl, braille documents) and EPUB 3 (.epub, print documents), HTML, LaTeX, HTML with MathJax (both HTML forms written as the original writes them, with no head, title or language on the plain HTML, and the original's fixed head on the MathJax page), and the older document format. File, Save eBraille saves the same package; Document, eBraille Metadata holds the package's metadata (required, recommended and additional properties, with the language, braille system and attribute dialogs, and an XML preview that can be edited); the quality notice shows before each save until its box is checked. Where the original's behaviour could not be read (the element of a paragraph style, which styles are headings and lists, the metadata property names of the recommended page, the list contents of the dialogs), the choice made is marked in the code.
- **Printing.** Print documents and braille (with interline print) through the system printer. In File, Print and
  File, Emboss, Selection lays the selected text out as a document of its own and prints or embosses all of its pages;
  a page-number range (p5, iv, 3-, -5) is matched against the pages' own numbers, nearest first with a question when
  a number had to be adjusted, and is not offered when two pages carry the same number. Bold, italic and underlined
  passages ([fts~b], [fts~i], [fts~u]) print in those styles; Interline Print starts from the document's embosser
  layout; a document whose printer the computer no longer has prints with the global printer after saying so.
  Printer Setup corrects a page size outside 7 to 160 characters and 1 to 100 lines and asks before taking it (Cancel
  returns to the field); Global, Printer Setup and Global, Embosser Setup first say, once unless asked to repeat, that
  they do not change the open document. The embosser setup sheet's General page offers the configuration location
  (current user or all users; kept with the setup) and its Device Settings page lists only the model's own firmware
  versions and shows the Braillo rows by the model's sub-model, as the original does. A Tiger model is sent to its
  printer only. File, Emboss asks for the output file first (an Embossed Braille File, .ebf, asked again while it
  cannot be made), shows the TranSend SE dialog only when the setup names an ink printer, shows a progress window
  with Cancel while the job goes out, and ends a failed job with "Printing failed." after its own message. A document
  opened with an embosser setup that is no longer kept says so and takes none.
- **Embossing.** Embosser setups for 139 embosser models, including Index and Enabling Technologies, with page ranges and
  copies, to a printer queue, a serial port or a file. Embossers on an FTDI USB connection (the EBrailler Cosmo) are
  listed and embossed to through FTDI's own driver library where it is installed. TranSend SE printers get their ink
  pass, and manual interpoint embossers a second pass for the backs. Each embosser gets the job start, margins and
  line padding its driver gives it in the original: the Index parameter string for its firmware version, the Enabling
  codes when escape codes are allowed, and the embosser's own left, right and top margin settings. The web build
  embosses through Web Serial where the browser has it, or to a downloaded file. Embosser Setup fits the page to the
  embosser as the original does: lines and cells within range and lowered before the margins, a table of the old and
  new values when OK had to change them, and a question before a document is reformatted to new cells or lines.
- **Screen readers.** The editing area gives screen readers its text (the formatted lines or the coded rows, one
  line each), the caret, the selection and every change: through UI Automation on Windows while Global,
  Internationalization, Support UI Automation is on, as the original program does; through the macOS accessibility
  interface for VoiceOver; through ATK and AT-SPI for Orca on Linux. On Windows a set of JAWS scripts comes with the
  program, and BrailleCraft offers to install them when it starts.
- **Formatted braille import settings.** Global, Formatted Braille Importer: where the braille and print page
  numbers stand on even and odd pages of the file, which pages have a running head or foot, the lines and cells of
  the file's pages, and Read formatted braille without interpretation, which keeps the file's lines and pages as they
  are. Until that is on, importing formatted braille first says that it may be reformatted.
- **Python Options.** Global, Python Options shows the original's dialog (its custom library path, Include
  Built-In Library, logging level and log file, kept under the original's keys); BrailleCraft reads PDFs with its
  own reader and runs no Python, so these have no effect on an import. The web build has the dialog too, and its
  PDF Importer dialog, whose settings its PDF import takes.
- **PDF import settings.** Global, PDF Importer: whether the dialog shows for each import, PDF/A markup, ledger
  tables, leader dots, a custom script, header and footer heights, font hints and the layout analysis values.
- **Command line.** `braillecraft-desktop file.dxp` opens a file. Options before a file act on the files after
  them: `/print`, `/braille[:setup]`, `/simulate`, `/brf:<file>`, `/prf:<file>`, `/lbf:<file>`, with the settings
  `/copies:<n>`, `/pagesize:<lines>x<cells>` and `/template:<name>`. When every file has an option, BrailleCraft does
  the work and ends without opening the editor.

## Downloads

The repository is [github.com/BlindCraftTeam/braillecraft](https://github.com/BlindCraftTeam/braillecraft). Every push
to main runs the Build workflow. Open the repository on GitHub, then **Actions**, then the newest run of
**Build**, then the **Artifacts** list at the end of the run summary:

| Artifact | What it holds |
|---|---|
| `braillecraft-windows-installer` | The Windows installer (.msi): the program, the braille fonts for every program, the JAWS scripts, a desktop shortcut, and the .dxp and .dxb file types with Open, Print and Emboss. |
| `braillecraft-macos-installer` | A disk image (.dmg) with BrailleCraft.app, for Apple Silicon Macs. Drag it to Applications. |
| `braillecraft-linux-packages` | A .deb (Debian, Ubuntu), an .rpm (Fedora and others) and an AppImage that runs without installing. |
| `braillecraft-desktop-<system>` | The program as a plain folder, for trying it without an installer. |
| `braillecraft-web` | The web build, to serve from any web server. |

The builds are not signed yet. Windows SmartScreen asks before the first launch ("More info", then "Run anyway"), and
macOS asks too (System Settings, Privacy and Security, "Open Anyway").

## Building

Requires Rust 1.80 or later, CMake and a C++ compiler; wxWidgets is built from source as part of the build. On Linux
the GTK 3, OpenGL and fontconfig development packages are needed as well (the workflow lists the packages it
installs). Tests that check translation against the full table set read the tables from the folder named by
`BC_TABLES`.

```
cargo build --release -p bc-desktop     # the desktop program: target/release/braillecraft-desktop
cargo test --workspace
```

The web build:

```
cargo build --release -p bc-web --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir site/pkg target/wasm32-unknown-unknown/release/bc_web.wasm
```

then copy `ui-web`, the fonts, the tables, the templates and the embosser models into `site` as the workflow's web
step does. The installers are made by the scripts in `packaging` (WiX 5 on Windows, `build-app.sh` on macOS,
`build-packages.sh` and `build-appimage.sh` on Linux).

## Layout

| Path | What it is |
|---|---|
| `crates/bc-core` | The core library: documents, the translation engine and its table compiler, page layout, file formats, printing and embossing data. No user interface. |
| `crates/bc-desktop` | The desktop program (wxWidgets through wxDragon): main window, menus, dialogs, translated line, status line. Builds `braillecraft-desktop`. |
| `crates/bc-web` | The core for the browser (WebAssembly). |
| `ui-web` | The web interface: menus and dialogs generated from the desktop program's tables. |
| `ui-native` | The desktop dialogs as XRC resources. |
| `tables` | The translation tables in BrailleCraft's own source format, with `index.tsv` and `languages.tsv`. |
| `templates` | The document templates. |
| `embossers` | The embosser models. |
| `ui/fonts` | The BrailleCraft Braille and SimBraille screen fonts, one pair per braille encoding. |
| `scripts/jaws` | The JAWS scripts. |
| `packaging` | Icons and the installers for Windows, macOS and Linux. |
| `tools` | The generators for menus, dialogs, fonts, icons, templates, language lists and the web tables. |
| `vendor/wxdragon-sys` | wxDragon's wxWidgets binding with what BrailleCraft needs added: printing, the drawn editing window with its text for screen readers on each system (UI Automation, the macOS accessibility interface, ATK), the Cursor Locator, and About box texts a screen reader reaches by Tab. The list is in `CHANGES-BrailleCraft.txt`. |
| `vendor/pdf-extract` | The PDF text reader, with marked content, font descent and figures reported for PDF import. The list is in `CHANGES-BrailleCraft.txt`. |
| `crates/bc-shell`, `crates/bc-louis`, `ui` | An earlier desktop shell (Tauri) and its liblouis binding; not part of the downloads. |
| `.github/workflows/build.yml` | Builds, tests and packages everything on every push to main. |

## Notes

- Commits carry the author's name only, with no generated-by or co-author lines.

## License and credits

BrailleCraft is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Copyright (c) 2026 BlindCraft Team and the BrailleCraft contributors. Required notices are in [NOTICE](NOTICE).

The BlindCraft name is a trademark of the BlindCraft Team and is not covered by the license above; its terms are in
[docs/brand/LICENSE-brand.txt](docs/brand/LICENSE-brand.txt). A modified version may say, in plain text, that it is
based on BrailleCraft by the BlindCraft Team.

Third-party parts (wxWidgets, wxdragon and the libraries they carry) keep their own licenses, listed in
[NOTICE](NOTICE). The braille fonts and the translation tables are part of BrailleCraft under the license above.
