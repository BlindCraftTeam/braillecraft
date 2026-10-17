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
  files (.txt, .brf, .brl) and PEF (.pef) files.
- **Formatted and coded views.** A document opens in the formatted view: its laid-out page lines with the codes
  hidden and an empty row after each page, which is what a screen reader reads. You type, delete, cut, paste and undo
  there as anywhere; a code next to the caret is removed whole. View, Codes (Alt+F3) switches to the coded view,
  the running text with every code in brackets in rows of 42 characters (65 in a print document), and back, with the caret kept in place. Control+Page Up and
  Control+Page Down go to the same line on the previous or next page (Global, Shortcut Preferences sets what Page Up
  and Page Down do).
- **Translation.** Print to braille and braille to print with 657 translation tables for 188 languages and braille
  codes, chosen by region and language (Document, Translation Tables) or by learning course (Learning Tables).
  Language codes in the text switch tables in the middle of a document. Every character the built-in character
  table names translates (accented letters, £, €, œ, dashes, curly quotes and thousands more); text files are read
  as UTF-8, or as Windows code page 1252 when they are not UTF-8.
- **Translated line.** Under the document, the braille (or the print) of the line you are on, updated as you write.
- **View preferences.** A margin bell a chosen number of cells before the end of a braille line, which codes the
  translated line shows, line numbers beside each line of a braille document in the formatted view when
  that setting is on, and the Cursor Locator: a rectangle that closes in on the caret when a document is opened or
  translated, when the view changes, or when Shift is pressed alone.
- **Braille pages.** Lines per page, cells per line, margins, centred headings and page numbering, shown in the
  status line as page, line and column. Each document's styles (headings, lists, paragraphs) are laid out from its
  own style list. Running heads and feet are kept out of the body text; showing them at the top and bottom of later
  pages is still being built.
- **Save As.** Formatted braille (BRF in a choice of braille encodings), PEF (Portable Embosser Format), formatted
  text in a choice of code pages, coded braille, refreshable braille (.lbf), LaTeX, and the older document format.
- **Printing.** Print documents and braille (with interline print) through the system printer.
- **Embossing.** Embosser setups for 139 embosser models, including Index and Enabling Technologies, with page ranges and
  copies, to a printer queue, a serial port or a file. Embossers on an FTDI USB connection (the EBrailler Cosmo) are
  listed and embossed to through FTDI's own driver library where it is installed. TranSend SE printers get their ink
  pass, and manual interpoint embossers a second pass for the backs. The web build embosses through Web Serial where
  the browser has it, or to a downloaded file.
- **Screen readers.** The document area is the system's own text control on every platform. On Windows a set of JAWS
  scripts comes with the program, and BrailleCraft offers to install them when it starts.
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
| `vendor/wxdragon-sys` | wxDragon's wxWidgets binding with what BrailleCraft needs added: printing, the Cursor Locator, the page break row and the line number column of the formatted view, and About box texts a screen reader reaches by Tab. The list is in `CHANGES-BrailleCraft.txt`. |
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
