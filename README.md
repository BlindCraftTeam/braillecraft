# BrailleCraft

A braille translation and embossing application for Windows, Linux, Mac and the web.
Version 1 scope (owner choice, 2026-10-14): an editor for print and braille documents, translation and
back-translation with liblouis, braille page formatting, import and export of text, BRF, PEF, docx and PDF,
and embossing to Index and Enabling embossers or to a file. Every screen meets WCAG 2.2 AA and is checked
with a screen reader before a stage is called done.

## Layout

| Path | What it is |
|---|---|
| `crates/bm-core` | The core library: document model, translation pipeline, formatting, import, export, embossing. No user interface. |
| `crates/bm-louis` | BrailleCraft's own binding to liblouis (built from source in a later stage). |
| `crates/bm-shell` | The desktop shell (Tauri 2): window, native menu bar, installers. Builds the `braillecraft` program. |
| `ui` | The user interface (HTML, CSS, JavaScript), shared by the desktop shell and, later, the web build. |
| `docs/spec` | The design documents: requirements and gap list (10), architecture and build stages (11), and the behaviour studies they come from. |
| `.github/workflows/build.yml` | Builds the installers for Linux, Windows and macOS on every push to main. |

The web build is added in its own stage.

## Building

Requires Rust 1.80 or later. In the development container (CT 234 on media-01) everything needed is installed.
On Linux the desktop shell also needs the WebKitGTK 4.1, GTK 3, libsoup 3, appindicator and librsvg development
packages (listed in the workflow).

```
cargo build
cargo test
```

Installers (from `crates/bm-shell`): `npx @tauri-apps/cli@2.12.3 build`. They land in `target/release/bundle`.

## Downloading a test build

Every push to main runs the Build workflow. Open the repository on GitHub, then Actions, then the newest run of
Build, then the Artifacts list at the end of the run summary: `braillecraft-windows` (.msi and setup .exe),
`braillecraft-macos` (.dmg, Apple Silicon and Intel), `braillecraft-linux` (.deb, .rpm, .AppImage). The builds are
not signed yet, so Windows SmartScreen asks before the first launch ("More info", then "Run anyway"), and macOS
asks too (System Settings, Privacy and Security, "Open Anyway").

## Rules for this repository

- BrailleCraft code is written new. No code, translation tables, embosser data or scripts from any other
  braille product are copied into it; the documents in `docs/spec` describe behaviour in our own words.
- liblouis is used under its own licence (LGPL) and linked so users can replace it.
- Commits carry the author's name only, with no generated-by or co-author lines.
- The licence of BrailleCraft itself is still to be chosen by the owner before anything is published.
