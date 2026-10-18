//! The control channel of the running program (`braillecraft-desktop --control PORT`, or Global, MCP Server): a
//! TCP port on 127.0.0.1 taking newline-delimited JSON requests {id, method, params, key} and answering
//! {id, ok, result} or {id, ok: false, error}. `braillecraft-cli mcp --connect PORT` bridges an AI agent's MCP tools
//! to it, so the agent works on the documents the user has open, presses keys, clicks and takes pictures of the
//! window. The methods are the ones the headless session answers (bc_mcp::headless) plus the ui.* ones; a document
//! method acts on the active document, as a menu command does.
//!
//! Requests arrive on the listener's threads and are carried out on the program's main thread: a timer on the main
//! window takes them from a queue a few times a second, so every method runs between events as a menu command
//! does. While a menu command is running (its dialog is open), only the ui.* methods are served, so an agent can read
//! and answer the dialog; the document methods wait until it closes. A command that opens a dialog is started and
//! answered at once ("started"), and the agent reads the dialog with a picture and answers it with keys and clicks.
//!
//! The key: 32 bytes of randomness written as hex to control-<port>.key in the settings folder (readable by the
//! user only where the system has file modes); a request whose key differs is answered "unauthorized". The port is
//! bound on 127.0.0.1 only, so only programs on this computer reach it.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;

use bc_core::doctext::Shown;
use bc_mcp::headless::{ascii_braille, unicode_braille};
use serde_json::{json, Value};
use wxdragon::prelude::*;
use wxdragon::uiactionsimulator::{KeyModifier, MouseButton, UIActionSimulator};

use crate::app::{self, DocKind};
use crate::settings_store::{get, set};
use crate::{commands, documents, encodings, translation};

/// The Global menu command that opens the MCP Server dialog (BrailleCraft's own item, after the reference's).
pub const COMMAND_ID: i32 = 3220;
/// The port the dialog proposes.
pub const DEFAULT_PORT: u16 = 7991;
const SECTION: &str = "Control";

// ------------------------------------------------------------------ settings

/// Global, MCP Server: the port, and whether the server starts with the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlSettings {
    pub port: u16,
    pub start_with_program: bool,
}

impl Default for ControlSettings {
    fn default() -> Self {
        ControlSettings { port: DEFAULT_PORT, start_with_program: false }
    }
}

impl ControlSettings {
    pub fn load() -> ControlSettings {
        let mut s = ControlSettings::default();
        if let Some(p) = get(SECTION, "Port").and_then(|v| v.trim().parse::<u16>().ok()).filter(|p| *p > 0) {
            s.port = p;
        }
        if let Some(v) = get(SECTION, "StartWithProgram") {
            s.start_with_program = v.trim().eq_ignore_ascii_case("yes");
        }
        s
    }

    pub fn save(&self) {
        set(SECTION, "Port", &self.port.to_string());
        set(SECTION, "StartWithProgram", if self.start_with_program { "yes" } else { "no" });
    }
}

// ------------------------------------------------------------------ the listener

/// A request waiting for the main thread, with the channel its reply line goes back on.
struct Pending {
    id: Value,
    method: String,
    params: Value,
    reply: Sender<String>,
}

static QUEUE: Mutex<VecDeque<Pending>> = Mutex::new(VecDeque::new());
static RUNNING: AtomicBool = AtomicBool::new(false);
static PORT: AtomicU16 = AtomicU16::new(0);
static KEY: Mutex<String> = Mutex::new(String::new());

thread_local! {
    /// How many requests are being carried out on the main thread right now: above zero inside a request's own
    /// dialog, when only ui.* requests are served.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static TIMER: RefCell<Option<wxdragon::timer::Timer<Frame>>> = const { RefCell::new(None) };
    /// The timer that runs the toolkit's deferred-call queue (what requests leave to run after their reply), apart
    /// from the request timer: a dialog opened from that queue runs its own event loop inside this timer's tick,
    /// and the system fires no timer again while its tick is running, so the request timer must be another one
    /// for requests (a picture, a key, a click) to be served while the dialog is up.
    static QUEUE_TIMER: RefCell<Option<wxdragon::timer::Timer<Frame>>> = const { RefCell::new(None) };
    static SHOTS: Cell<u32> = const { Cell::new(0) };
    /// Until when no request is taken, after one that leaves events behind (a command started, a window brought
    /// to the front, a key or a click), so the program has carried them out before the next request looks.
    static SETTLE: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
}

/// How long the program is left to carry out what a request started before the next request is taken.
const SETTLE_TIME: std::time::Duration = std::time::Duration::from_millis(350);

/// Whether a request leaves events behind that the next request should see carried out.
fn leaves_events(method: &str) -> bool {
    matches!(method, "engine.execute" | "ui.activate" | "ui.key" | "ui.click" | "document.open" | "document.new" | "translate" | "back_translate")
}

/// Whether the server is running, and on which port.
pub fn port() -> Option<u16> {
    RUNNING.load(Ordering::SeqCst).then(|| PORT.load(Ordering::SeqCst))
}

/// The key file of a port: control-<port>.key in the settings folder (bc_mcp::backend::key_path).
fn key_file(port: u16) -> Option<PathBuf> {
    bc_core::paths::settings_dir().map(|d| bc_mcp::backend::key_path(&d, port))
}

/// 32 bytes of randomness as 64 hex digits, from the standard library's randomly keyed hasher.
fn new_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for n in 0..4u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(n ^ std::process::id() as u64);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

fn write_key_file(port: u16, key: &str) -> Result<PathBuf, String> {
    let path = key_file(port).ok_or("no settings folder for the key file (HOME or APPDATA is not set)")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, format!("{key}\n")).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(path)
}

/// Starts the server on `port` (0 for one the system picks). Answers the port it listens on.
pub fn start(port: u16) -> Result<u16, String> {
    if let Some(p) = self::port() {
        return Ok(p);
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("port {port} cannot be opened on 127.0.0.1: {e}"))?;
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    let key = new_key();
    write_key_file(port, &key)?;
    *KEY.lock().unwrap_or_else(|p| p.into_inner()) = key;
    PORT.store(port, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);
    std::thread::Builder::new()
        .name("control-listener".into())
        .spawn(move || accept_loop(listener))
        .map_err(|e| format!("the listener thread could not be started: {e}"))?;
    ensure_timer();
    Ok(port)
}

/// At start-up: the server started when the command line gave --control PORT, or when Global, MCP Server says it
/// starts with the program; a failure is reported on the error output and the program goes on without it.
pub fn start_at_launch(command_line: Option<u16>) {
    let settings = ControlSettings::load();
    let port = match command_line {
        Some(p) => p,
        None if settings.start_with_program => settings.port,
        None => return,
    };
    match start(port) {
        Ok(p) => eprintln!("BrailleCraft: control channel on 127.0.0.1:{p}"),
        Err(e) => eprintln!("BrailleCraft: the control channel could not be started: {e}"),
    }
}

/// Stops the server: no more connections are taken, the key file is removed; open connections end at their next
/// request.
pub fn stop() {
    let Some(port) = port() else { return };
    RUNNING.store(false, Ordering::SeqCst);
    // The accept loop wakes up on one more connection and ends.
    let _ = TcpStream::connect(("127.0.0.1", port));
    if let Some(path) = key_file(port) {
        let _ = std::fs::remove_file(path);
    }
    KEY.lock().unwrap_or_else(|p| p.into_inner()).clear();
}

fn accept_loop(listener: TcpListener) {
    for stream in listener.incoming() {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let _ = std::thread::Builder::new().name("control-connection".into()).spawn(move || serve(stream));
    }
}

/// One connection: a request per line, a reply per line, in order.
fn serve(stream: TcpStream) {
    let Ok(mut writer) = stream.try_clone() else { return };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let key = KEY.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let reply = match parse_request(&line, &key, RUNNING.load(Ordering::SeqCst)) {
            Err(early) => early,
            Ok((id, method, params)) => {
                let (tx, rx) = channel();
                QUEUE.lock().unwrap_or_else(|p| p.into_inner()).push_back(Pending { id, method, params, reply: tx });
                match rx.recv() {
                    Ok(r) => r,
                    Err(_) => break,
                }
            }
        };
        if writer.write_all(reply.as_bytes()).and_then(|_| writer.write_all(b"\n")).and_then(|_| writer.flush()).is_err() {
            break;
        }
    }
}

/// A reply line.
fn reply(id: &Value, result: Result<Value, String>) -> String {
    match result {
        Ok(r) => json!({"id": id, "ok": true, "result": r}).to_string(),
        Err(e) => json!({"id": id, "ok": false, "error": e}).to_string(),
    }
}

/// Reads a request line: its id, method and params, or the reply line for one that is refused before it reaches
/// the main thread (malformed, no method, the wrong key, the server stopped).
fn parse_request(line: &str, key: &str, running: bool) -> Result<(Value, String, Value), String> {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Err(reply(&Value::Null, Err(format!("malformed request: {e}")))),
    };
    let id = v.get("id").cloned().unwrap_or(Value::Null);
    if !running {
        return Err(reply(&id, Err("the control channel is stopped".into())));
    }
    if key.is_empty() || v.get("key").and_then(Value::as_str) != Some(key) {
        return Err(reply(&id, Err("unauthorized".into())));
    }
    let Some(method) = v.get("method").and_then(Value::as_str).filter(|m| !m.is_empty()) else {
        return Err(reply(&id, Err("missing method".into())));
    };
    let params = match v.get("params") {
        Some(Value::Object(p)) => Value::Object(p.clone()),
        _ => json!({}),
    };
    Ok((id, method.to_string(), params))
}

// ------------------------------------------------------------------ the main thread

/// The timer that carries requests out on the main thread, made once.
fn ensure_timer() {
    TIMER.with(|t| {
        if t.borrow().is_some() {
            return;
        }
        let timer = wxdragon::timer::Timer::new(&app::frame());
        timer.on_tick(|_| drain());
        timer.start(30, false);
        *t.borrow_mut() = Some(timer);
    });
    QUEUE_TIMER.with(|t| {
        if t.borrow().is_some() {
            return;
        }
        let timer = wxdragon::timer::Timer::new(&app::frame());
        timer.on_tick(|_| {
            wxdragon::app::process_main_thread_queue();
        });
        timer.start(30, false);
        *t.borrow_mut() = Some(timer);
    });
}

/// Whether a request named `method` may run while a menu command is being carried out (its dialog is open): the
/// ui.* methods and ping may; the document methods wait.
fn runs_while_busy(method: &str) -> bool {
    method.starts_with("ui.") || method == "ping"
}

/// Whether a request may run while another request is itself being carried out (a picture being taken lets the
/// program's events run): a key, a click and ping; a second picture waits for the first.
fn runs_inside_request(method: &str) -> bool {
    matches!(method, "ui.key" | "ui.click" | "ping")
}

/// Carries out the next waiting request: one per tick, so the events a request leaves behind (a command started,
/// a key pressed, a window brought to the front) are carried out before the request after it. While a request (or
/// a menu command) is running, only the ui.* ones are taken.
fn drain() {
    // What earlier requests left to run after their reply (a menu command started by engine.execute) runs from
    // QUEUE_TIMER: the toolkit runs that queue only when its main loop is idle, which a steady stream of requests
    // never leaves it; and it runs apart from this timer so a dialog opened from it leaves this one ticking.
    if SETTLE.with(|t| t.get().is_some_and(|until| std::time::Instant::now() < until)) {
        return;
    }
    let next = {
        let mut q = QUEUE.lock().unwrap_or_else(|p| p.into_inner());
        let inside = DEPTH.with(|d| d.get()) > 0;
        let busy = inside || app_busy();
        match q.front() {
            None => None,
            Some(p) if inside && !runs_inside_request(&p.method) => None,
            Some(p) if busy && !runs_while_busy(&p.method) => None,
            Some(_) => q.pop_front(),
        }
    };
    let Some(p) = next else { return };
    DEPTH.with(|d| d.set(d.get() + 1));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(&p.method, &p.params)))
        .unwrap_or_else(|_| Err(format!("the program failed inside `{}`", p.method)));
    DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    if leaves_events(&p.method) {
        SETTLE.with(|t| t.set(Some(std::time::Instant::now() + SETTLE_TIME)));
    }
    let _ = p.reply.send(reply(&p.id, result));
}

/// Whether a menu command is running (its dialog is open).
fn app_busy() -> bool {
    let mut busy = false;
    app::try_with(|a| busy = a.busy);
    busy
}

fn active() -> Result<usize, String> {
    app::with(|a| a.active).ok_or_else(|| "no document is open".to_string())
}

fn index_of(key: u64) -> Result<usize, String> {
    app::with(|a| a.index_of(key)).ok_or_else(|| "the document was closed".to_string())
}

fn na() -> &'static encodings::Encoding {
    encodings::find(encodings::DEFAULT)
}

/// A braille document's text in North American ASCII braille, whatever encoding it is shown in; a print
/// document's as it is.
fn text_of(a: &app::App, i: usize) -> Shown {
    let shown = a.docs[i].shown();
    if a.docs[i].kind == DocKind::Braille {
        translation::convert_braille(&shown, a.encoding(), na())
    } else {
        shown
    }
}

/// Text an agent gives for a braille document (North American ASCII, or Unicode braille patterns), in the
/// document's own encoding.
fn braille_in(a: &app::App, text: &str) -> Shown {
    translation::convert_braille(&Shown::marked(&ascii_braille(text)), na(), a.encoding())
}

/// What inspect_document answers for document `i`.
fn inspect_doc(i: usize) -> Value {
    app::with(|a| {
        let shown = a.docs[i].shown();
        let pages = a.laid_out(i, &shown);
        let text = text_of(a, i);
        let d = &a.docs[i];
        let pos = d.caret();
        let (from, to) = d.selection();
        let (row, col) = d.ed.caret_place();
        let location = match d.ed.row(row) {
            Some(bc_core::editor::Row::Line { page, line }) if !d.ed.coded => Some((page + 1, line + 1, col + 1)),
            _ => None,
        };
        json!({
            "kind": if d.kind == DocKind::Braille { "braille" } else { "print" },
            "index": i,
            "active": a.active == Some(i),
            "name": d.name,
            "window": d.full_name(),
            "path": d.path.as_ref().map(|p| p.display().to_string()),
            "template": d.template,
            "tables": d.tables,
            "characters": text.text.chars().count(),
            "codes": text.codes.len(),
            "cells": d.cells, "lines": d.lines, "pages": pages.len(),
            "caret": pos, "selection": [from, to],
            "page": location.map(|l| l.0), "line": location.map(|l| l.1), "column": location.map(|l| l.2),
            "view": if d.ed.coded { "coded" } else { "formatted" },
            "open_styles": bc_core::status::open_styles(&shown, pos as i64),
            "status": d.status.get_label(),
            "styles": d.styles.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            "embosser_setup": d.embosser,
            "dirty": d.dirty,
            "saved": d.saved,
        })
    })
}

/// The open documents, as ui.documents lists them.
fn documents_list() -> Value {
    app::with(|a| {
        Value::Array(
            a.docs
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    json!({
                        "index": i,
                        "name": d.name,
                        "window": d.full_name(),
                        "kind": if d.kind == DocKind::Braille { "braille" } else { "print" },
                        "path": d.path.as_ref().map(|p| p.display().to_string()),
                        "dirty": d.dirty,
                        "active": a.active == Some(i),
                        // The document's tab and its close button, each as a point in the main window's
                        // coordinates (for the click tool), where the system reports them (not on macOS).
                        "tab": a.tabs.tab_rect(i).map(|r| {
                            let at = a.frame.screen_to_client(Point::new(r.x + r.width / 2, r.y + r.height / 2));
                            json!([at.x, at.y])
                        }),
                        "close_button": a.tabs.close_button_rect(i).map(|r| {
                            let at = a.frame.screen_to_client(Point::new(r.x + r.width / 2, r.y + r.height / 2));
                            json!([at.x, at.y])
                        }),
                    })
                })
                .collect(),
        )
    })
}

/// The four table codes of a language by its name, or the document's own.
fn tables_for(i: usize, language: Option<&str>) -> Result<[String; 4], String> {
    let Some(name) = language else { return Ok(app::with(|a| a.docs[i].tables.clone())) };
    let want = name.to_lowercase();
    translation::with_list(|l| {
        l.entries
            .iter()
            .find(|e| e.name.to_lowercase() == want)
            .or_else(|| l.entries.iter().find(|e| e.name.to_lowercase().contains(&want)))
            .map(|e| e.tables.clone())
    })
    .ok_or_else(|| format!("no language named {name} (see list_tables)"))
}

/// Print text with codes to braille (North American ASCII) with document `i`'s tables, or a language's.
fn forward(i: usize, text: &str, language: Option<&str>) -> Result<Shown, String> {
    let tables = tables_for(i, language)?;
    app::with(|a| {
        let d = &a.docs[i];
        let st = translation::style_table(&d.styles);
        let out = translation::forward(&tables[0], &Shown::marked(text), &d.initial_codes, a.encoding(), &st).map_err(|e| format!("Translation failed: {e}"))?;
        Ok(translation::convert_braille(&out, a.encoding(), na()))
    })
}

fn backward(i: usize, text: &str, language: Option<&str>) -> Result<Shown, String> {
    let tables = tables_for(i, language)?;
    app::with(|a| {
        let d = &a.docs[i];
        let st = translation::style_table(&d.styles);
        let input = braille_in(a, text);
        translation::back(&tables[2], &input, &d.initial_codes, a.encoding(), &st).map_err(|e| format!("Translation failed: {e}"))
    })
}

/// Document `i` laid out into pages: its own page size, or `cells` and `lines` given.
fn pages_of(i: usize, cells: Option<u32>, lines: Option<u32>) -> std::rc::Rc<Vec<bc_core::format::Page>> {
    app::with(|a| {
        let shown = a.docs[i].shown();
        if cells.is_none() && lines.is_none() {
            return a.laid_out(i, &shown);
        }
        let (_, mut layout, styles) = a.page_layout(i);
        if let Some(c) = cells {
            layout.cells = c.max(1) as usize;
        }
        if let Some(l) = lines {
            layout.lines = l.max(1) as usize;
        }
        std::rc::Rc::new(bc_core::format::format(&shown, &layout, Some(&styles)))
    })
}

/// A page line's text, in North American ASCII for a braille document.
fn line_text(i: usize, text: &str) -> String {
    app::with(|a| if a.docs[i].kind == DocKind::Braille { translation::convert_braille(&Shown::plain(text), a.encoding(), na()).text } else { text.to_string() })
}

fn u32_param(p: &Value, name: &str) -> Option<u32> {
    p.get(name).and_then(Value::as_u64).map(|v| v.min(u32::MAX as u64) as u32)
}

/// The commands the headless session carries out by itself (the rest need the program, which this channel is).
fn headless_command(id: i32) -> bool {
    matches!(id, 1101 | 1102 | 1105 | 1106 | 1109 | 1201 | 1202 | 1206 | 1207 | 1213 | 1701..=2404)
}

/// The key code (wxWidgets key codes, wx/defs.h) a key name stands for: a letter, digit or sign; Enter, Tab,
/// Escape, Backspace, Delete, Space, Insert, Home, End, PageUp, PageDown, Up, Down, Left, Right, F1 to F24.
pub fn key_code(name: &str) -> Option<i32> {
    let n = name.trim();
    let lower = n.to_ascii_lowercase();
    let code = match lower.as_str() {
        "enter" | "return" => 13,
        "tab" => 9,
        "escape" | "esc" => 27,
        "backspace" | "back" => 8,
        "delete" | "del" => 127,
        "space" => 32,
        "insert" | "ins" => 322,
        "end" => 312,
        "home" => 313,
        "left" => 314,
        "up" => 315,
        "right" => 316,
        "down" => 317,
        "pageup" | "page_up" | "pgup" | "prior" => 366,
        "pagedown" | "page_down" | "pgdn" | "next" => 367,
        s if s.len() > 1 && s.starts_with('f') && s[1..].chars().all(|c| c.is_ascii_digit()) => {
            let f: i32 = s[1..].parse().ok()?;
            if !(1..=24).contains(&f) {
                return None;
            }
            339 + f
        }
        _ => {
            let mut cs = n.chars();
            let c = cs.next()?;
            if cs.next().is_some() || (c as u32) > 126 {
                return None;
            }
            c.to_ascii_uppercase() as i32
        }
    };
    Some(code)
}

/// A key request: "Ctrl+Shift+T" or {key: "T", ctrl: true, shift: true}; the key code and the modifier flags
/// (wxMOD_ALT 1, wxMOD_CONTROL 2, wxMOD_SHIFT 4, wxMOD_META 8).
pub fn key_request(params: &Value) -> Result<(i32, i32), String> {
    let spec = params.get("key").and_then(Value::as_str).ok_or("missing `key`")?;
    let mut mods = 0;
    let mut name = spec.trim();
    // "Ctrl+Shift+T": every part but the last names a modifier ("+" alone is the key).
    while let Some((m, rest)) = name.split_once('+').filter(|(_, r)| !r.is_empty()) {
        match m.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= 2,
            "shift" => mods |= 4,
            "alt" | "option" => mods |= 1,
            "meta" | "cmd" | "command" | "win" => mods |= 8,
            _ => break,
        }
        name = rest;
    }
    let flag = |k: &str| params.get(k).and_then(Value::as_bool).unwrap_or(false);
    if flag("ctrl") || flag("control") {
        mods |= 2;
    }
    if flag("shift") {
        mods |= 4;
    }
    if flag("alt") {
        mods |= 1;
    }
    if flag("meta") || flag("cmd") {
        mods |= 8;
    }
    let code = key_code(name).ok_or_else(|| format!("unknown key `{name}`: a letter, digit or sign, or Enter, Tab, Escape, Backspace, Delete, Space, Insert, Home, End, PageUp, PageDown, Up, Down, Left, Right, F1 to F24"))?;
    Ok((code, mods))
}

/// How long a picture lets the program carry out its pending events first (a dialog opening, a window coming to
/// the front), so the picture shows the state the last request led to.
const PICTURE_SETTLE_MS: i32 = 500;

/// A picture of the program's windows as a PNG file at `path` (every shown top-level window of the program, as
/// one picture): its width and height.
fn screenshot(path: &Path) -> Result<(i32, i32), String> {
    let c = std::ffi::CString::new(path.to_string_lossy().into_owned()).map_err(|_| "bad picture path")?;
    let (mut w, mut h) = (0i32, 0i32);
    // SAFETY: the path is a live zero-ended string and the two outputs are live integers; the call runs on the
    // main thread, where the windows are.
    let ok = unsafe { wxdragon_sys::wxd_App_SaveScreenshot(c.as_ptr(), PICTURE_SETTLE_MS, &mut w, &mut h) };
    if !ok {
        return Err("the picture could not be taken (no window is shown, or the screen cannot be read)".into());
    }
    Ok((w, h))
}

/// Carries out one request on the main thread.
fn dispatch(method: &str, p: &Value) -> Result<Value, String> {
    let q = p.get("query").and_then(Value::as_str).map(|q| q.to_lowercase());
    let matches = |s: &str| q.as_ref().is_none_or(|q| s.to_lowercase().contains(q.as_str()));
    match method {
        "ping" => Ok(json!("pong")),
        "engine.commands" => Ok(Value::Array(
            bc_core::command_table::COMMANDS
                .iter()
                .map(|c| json!({"id": c.id, "label": c.label, "menu": c.menu, "shortcut": c.shortcut, "headless": headless_command(c.id), "available": true}))
                .collect(),
        )),
        "engine.execute" => execute(p),
        "document.new" => new_document(p),
        "document.open" => open_document(p),
        "document.insert_file" => Err("Insert File runs through its dialog: execute command 1213, then answer the dialog with key and click".into()),
        "document.save" => save_document(p),
        "document.export" => export_document(p),
        "document.text" => {
            let i = active()?;
            let text = app::with(|a| text_of(a, i));
            let braille = app::with(|a| a.docs[i].kind == DocKind::Braille);
            Ok(json!({"text": text.text, "kind": if braille { "braille" } else { "print" }, "unicode": braille.then(|| unicode_braille(&text.text))}))
        }
        "document.inspect" => Ok(inspect_doc(active()?)),
        "text.insert" => {
            let i = active()?;
            let text = p.get("text").and_then(Value::as_str).ok_or("missing `text`")?;
            app::with(|a| {
                if let Some(at) = u32_param(p, "at") {
                    let len = a.docs[i].shown().text.chars().count();
                    let at = (at as usize).min(len);
                    a.docs[i].select_doc(at, at);
                }
                let put = if a.docs[i].kind == DocKind::Braille { braille_in(a, text) } else { Shown::marked(text) };
                documents::type_in(a, i, &put);
            });
            Ok(inspect_doc(i))
        }
        "select" => {
            let i = active()?;
            app::with(|a| {
                let shown = a.docs[i].shown();
                let len = shown.text.chars().count();
                let (from, to) = if let Some(text) = p.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                    let n = p.get("occurrence").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
                    let hay: Vec<char> = shown.text.chars().collect();
                    let needle: Vec<char> = text.chars().collect();
                    let mut seen = 0;
                    let mut found = None;
                    for start in 0..hay.len().saturating_sub(needle.len() - 1) {
                        if hay[start..start + needle.len()] == needle[..] {
                            seen += 1;
                            if seen == n {
                                found = Some(start);
                                break;
                            }
                        }
                    }
                    let start = found.ok_or_else(|| format!("occurrence {n} of {text:?} not found"))?;
                    (start, start + needle.len())
                } else {
                    let (anchor, caret) = (a.docs[i].selection().0, a.docs[i].caret());
                    let from = u32_param(p, "from").map_or(anchor.min(caret), |v| v as usize).min(len);
                    let to = u32_param(p, "to").map_or(from, |v| v as usize).min(len);
                    (from, to)
                };
                a.docs[i].select_doc(from, to);
                a.show_area(i, None);
                a.update_status();
                let (s, e) = (from.min(to), from.max(to));
                Ok(json!({"selection": [s, e], "text": shown.slice(s, e).text}))
            })
        }
        "translate" => {
            let i = active()?;
            let language = p.get("language").and_then(Value::as_str);
            match p.get("text").and_then(Value::as_str) {
                Some(text) => {
                    let out = forward(i, text, language)?;
                    Ok(json!({"braille": out.text, "unicode": unicode_braille(&out.text)}))
                }
                None => {
                    if app::with(|a| a.docs[i].kind == DocKind::Braille) {
                        return Err("the active document is braille already; give `text`, or use back_translate".into());
                    }
                    let key = commands::translate_active()?;
                    let j = index_of(key)?;
                    let text = app::with(|a| text_of(a, j));
                    Ok(json!({"document": inspect_doc(j), "braille": text.text, "unicode": unicode_braille(&text.text)}))
                }
            }
        }
        "back_translate" => {
            let i = active()?;
            let language = p.get("language").and_then(Value::as_str);
            match p.get("text").and_then(Value::as_str) {
                Some(text) => Ok(json!({"print": backward(i, text, language)?.text})),
                None => {
                    if app::with(|a| a.docs[i].kind == DocKind::Print) {
                        return Err("the active document is print; give `text`, or use translate".into());
                    }
                    let key = commands::translate_active()?;
                    let j = index_of(key)?;
                    Ok(json!({"document": inspect_doc(j), "print": app::with(|a| a.docs[j].shown().text)}))
                }
            }
        }
        "document.format" => {
            let i = active()?;
            let (cells, lines) = (u32_param(p, "cells"), u32_param(p, "lines"));
            let pages = pages_of(i, cells, lines);
            let first = p.get("first_page").and_then(Value::as_u64).unwrap_or(1).max(1) as usize - 1;
            let count = p.get("pages").and_then(Value::as_u64).unwrap_or(5) as usize;
            let shown: Vec<Value> = pages
                .iter()
                .enumerate()
                .skip(first)
                .take(count)
                .map(|(k, page)| json!({"page": k + 1, "lines": page.lines.iter().map(|l| line_text(i, &l.text)).collect::<Vec<_>>()}))
                .collect();
            let (c, l) = app::with(|a| (a.docs[i].cells, a.docs[i].lines));
            Ok(json!({"pages": pages.len(), "cells": cells.unwrap_or(c), "lines": lines.unwrap_or(l), "shown": shown}))
        }
        "emboss.bytes" => {
            let i = active()?;
            if app::with(|a| a.docs[i].kind != DocKind::Braille) {
                return Err("an emboss job is made from a braille document: translate first".into());
            }
            let (c, l) = app::with(|a| (a.docs[i].cells, a.docs[i].lines));
            let cells = u32_param(p, "cells").unwrap_or(c);
            let lines = u32_param(p, "lines").unwrap_or(l);
            let copies = u32_param(p, "copies").unwrap_or(1).max(1);
            let pages = pages_of(i, Some(cells), Some(lines));
            let sides: Vec<Option<Vec<String>>> = pages.iter().map(|page| Some(page.lines.iter().map(|ln| line_text(i, &ln.text)).collect())).collect();
            let options = bc_core::emboss::Options { cells, lines, ..bc_core::emboss::Options::default() };
            let bytes = bc_core::emboss::job(&options, &sides, copies);
            let mut result = json!({"bytes": bytes.len(), "pages": pages.len(), "base64": bc_mcp::base64(&bytes)});
            if let Some(port) = p.get("port").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                let looks_like_file = port.contains(['/', '\\']) || Path::new(port).extension().is_some();
                let target = if looks_like_file { bc_core::emboss_out::Target::File(PathBuf::from(port)) } else { bc_core::emboss_out::Target::Port { name: port.to_string(), serial: None } };
                let name = app::with(|a| a.docs[i].name.clone());
                bc_core::emboss_out::send(&target, &name, &bytes).map_err(|e| e.message())?;
                result["sent_to"] = json!(port);
            }
            Ok(result)
        }
        "tables.list" => Ok(translation::with_list(|l| Value::Array(l.entries.iter().filter(|e| matches(&e.name)).map(|e| json!({"language": e.name, "tables": e.tables})).collect()))),
        "templates.list" => Ok(Value::Array(translation::with_templates(|t| t.map(|t| t.names_sorted()).unwrap_or_default()).into_iter().filter(|n| matches(n)).map(Value::String).collect())),
        "embossers.list" => crate::embosser::with_catalog(|c| c.map(|c| Value::Array(c.names().into_iter().filter(|n| matches(n)).map(Value::String).collect())).map_err(|e| e.to_string())),
        "undo" | "redo" => {
            let i = active()?;
            let steps = p.get("steps").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
            let redo = method == "redo";
            app::with(|a| {
                for _ in 0..steps {
                    if !a.undo(i, redo) {
                        break;
                    }
                }
            });
            Ok(inspect_doc(i))
        }
        "ui.documents" => Ok(json!({"documents": documents_list(), "busy": app_busy()})),
        "ui.activate" => {
            let index = p.get("index").and_then(Value::as_u64).ok_or("missing `index`")? as usize;
            let count = app::with(|a| a.docs.len());
            if index >= count {
                return Err(format!("no document {index}: {count} open (see list_documents)"));
            }
            documents::bring_to_front(index);
            Ok(inspect_doc(index))
        }
        "ui.key" => {
            let sim = UIActionSimulator::new();
            if !sim.is_ok() {
                return Err("keys cannot be simulated on this system".into());
            }
            if let Some(text) = p.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                if !text.is_ascii() {
                    return Err("`text` for key takes ASCII only; use type_text for other characters".into());
                }
                if !sim.text(text) {
                    return Err("the text could not be typed".into());
                }
                return Ok(json!({"typed": text}));
            }
            let (code, mods) = key_request(p)?;
            if !sim.char_with_modifiers(code, KeyModifier::from_raw(mods)) {
                return Err("the key could not be pressed".into());
            }
            Ok(json!({"key": p.get("key"), "code": code, "modifiers": mods}))
        }
        "ui.click" => {
            let x = p.get("x").and_then(Value::as_i64).ok_or("missing `x`")? as i32;
            let y = p.get("y").and_then(Value::as_i64).ok_or("missing `y`")? as i32;
            let sim = UIActionSimulator::new();
            if !sim.is_ok() {
                return Err("the mouse cannot be simulated on this system".into());
            }
            let at = app::frame().client_to_screen(Point::new(x, y));
            let button = match p.get("button").and_then(Value::as_str).unwrap_or("left") {
                "right" => MouseButton::Right,
                "middle" => MouseButton::Middle,
                _ => MouseButton::Left,
            };
            // With `to_x` and `to_y`: a drag from the point to there (a tab dragged along the tab bar).
            if let (Some(tx), Some(ty)) = (p.get("to_x").and_then(Value::as_i64), p.get("to_y").and_then(Value::as_i64)) {
                let to = app::frame().client_to_screen(Point::new(tx as i32, ty as i32));
                if !sim.mouse_drag_drop(at.x, at.y, to.x, to.y, button) {
                    return Err("the drag could not be made".into());
                }
                return Ok(json!({"x": x, "y": y, "to_x": tx, "to_y": ty, "screen": [at.x, at.y, to.x, to.y]}));
            }
            sim.mouse_move(at.x, at.y);
            let ok = if p.get("double").and_then(Value::as_bool).unwrap_or(false) { sim.mouse_dbl_click(button) } else { sim.mouse_click(button) };
            if !ok {
                return Err("the click could not be made".into());
            }
            Ok(json!({"x": x, "y": y, "screen": [at.x, at.y]}))
        }
        "ui.screenshot" => {
            let n = SHOTS.with(|s| {
                s.set(s.get() + 1);
                s.get()
            });
            let path = std::env::temp_dir().join(format!("braillecraft-picture-{}-{n}.png", std::process::id()));
            let (w, h) = screenshot(&path)?;
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let _ = std::fs::remove_file(&path);
            Ok(json!({"png": bc_mcp::base64(&bytes), "width": w, "height": h}))
        }
        other => Err(format!("unknown method `{other}`")),
    }
}

/// engine.execute: a menu command by id. The ones the headless session carries out run here at once with their
/// parameters; any other runs as its menu item would, after the reply, so a dialog it opens is answered with the
/// ui.* methods.
fn execute(p: &Value) -> Result<Value, String> {
    let id = p.get("command").and_then(Value::as_i64).ok_or("missing `command` (a command id)")? as i32;
    let params = p.get("params").cloned().unwrap_or(json!({}));
    let command = bc_core::command_table::by_id(id).ok_or_else(|| format!("no command with id {id}"))?;
    let has_path = params.get("path").and_then(Value::as_str).is_some();
    match id {
        1101 => return new_document(&params),
        1102 if has_path => return open_document(&params),
        1105 if has_path || app::with(|a| a.doc().is_some_and(|d| d.saved && d.path.is_some())) => return save_document(&params),
        1106 if has_path => return export_document(&params),
        1109 => {
            let key = commands::translate_active()?;
            return Ok(inspect_doc(index_of(key)?));
        }
        1201 | 1202 => return dispatch(if id == 1201 { "undo" } else { "redo" }, &json!({})),
        1203..=1207 => {
            let i = active()?;
            let done = app::with(|a| documents::edit_command(a, i, id));
            if !done {
                return Err(format!("command {id} could not be carried out"));
            }
            return Ok(inspect_doc(i));
        }
        1701..=2404 => {
            use bc_core::menu_codes::{Dynamic, Insert};
            if let Some(what) = bc_core::menu_codes::insert(id) {
                let i = active()?;
                app::with(|a| {
                    let braille = a.docs[i].kind == DocKind::Braille;
                    let put = match what {
                        Insert::Text(t) => Shown::marked(t),
                        Insert::Pair(x, y) => {
                            let (from, to) = a.docs[i].selection();
                            let middle = a.docs[i].shown().slice(from, to);
                            let mut s = Shown::marked(x);
                            s.push(&middle);
                            s.push(&Shown::marked(y));
                            s
                        }
                        Insert::Setting(d) => match d {
                            Dynamic::Grade0 => Shown::marked("[cz]"),
                            Dynamic::Cbc => Shown::marked("[cb]"),
                            Dynamic::FlushRightLeader => Shown::marked("[fr;p~\"]"),
                            Dynamic::NewPage => Shown::marked("[pg]"),
                            Dynamic::NewReferencePage => Shown::marked("[lea]"),
                            Dynamic::TerminationSign => Shown::plain(if braille { ",'" } else { "&@" }),
                            Dynamic::HardSpace => Shown::plain("\u{a0}"),
                        },
                    };
                    documents::type_in(a, i, &put);
                });
                return Ok(inspect_doc(i));
            }
        }
        _ => {}
    }
    let checked = params.get("checked").and_then(Value::as_bool).unwrap_or(true);
    wxdragon::call_after(Box::new(move || commands::run(id, checked)));
    Ok(json!({
        "command": id, "label": command.label, "started": true,
        "note": "the command runs in the program now; a dialog it opens is read with screenshot and answered with key and click; inspect_document afterwards",
    }))
}

fn new_document(p: &Value) -> Result<Value, String> {
    let braille = p.get("braille").and_then(Value::as_bool).unwrap_or(false);
    let key = documents::new_document(if braille { DocKind::Braille } else { DocKind::Print }, None, &Shown::default());
    app::with(|a| {
        if let Some(i) = a.index_of(key) {
            a.docs[i].dirty = false;
        }
    });
    if let Some(t) = p.get("template").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        if !translation::with_templates(|ts| ts.is_some_and(|ts| ts.by_name(t).is_some())) {
            return Err(format!("no template named {t} (see list_templates)"));
        }
        commands::apply_template(key, t);
    }
    Ok(inspect_doc(index_of(key)?))
}

fn open_document(p: &Value) -> Result<Value, String> {
    let path = PathBuf::from(p.get("path").and_then(Value::as_str).ok_or("missing `path`")?);
    if !path.is_file() {
        return Err(format!("{}: no such file", path.display()));
    }
    let key = commands::open_path(&path).ok_or_else(|| format!("{} could not be opened (the program showed why)", path.display()))?;
    if let Some(t) = p.get("template").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        commands::apply_template(key, t);
    }
    Ok(inspect_doc(index_of(key)?))
}

fn save_document(p: &Value) -> Result<Value, String> {
    let i = active()?;
    let (key, braille, own) = app::with(|a| {
        let d = &a.docs[i];
        (d.key, d.kind == DocKind::Braille, d.path.clone().filter(|_| d.saved))
    });
    let path = match p.get("path").and_then(Value::as_str) {
        Some(s) => PathBuf::from(s),
        None => own.ok_or("the document has no file of its own yet: give `path`")?,
    };
    let path = if path.extension().is_none() { path.with_extension(if braille { "dxb" } else { "dxp" }) } else { path };
    if !commands::write_document(key, &path, 1) {
        return Err(format!("{} was not written (the program showed why)", path.display()));
    }
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(json!({"path": path.display().to_string(), "bytes": bytes}))
}

fn export_document(p: &Value) -> Result<Value, String> {
    let i = active()?;
    let (key, braille) = app::with(|a| (a.docs[i].key, a.docs[i].kind == DocKind::Braille));
    let path = PathBuf::from(p.get("path").and_then(Value::as_str).ok_or("missing `path`")?);
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let kind = p.get("type").and_then(Value::as_str).map(|t| t.to_lowercase()).unwrap_or(ext);
    let code_page = p.get("code_page").and_then(Value::as_str);
    let written = match kind.as_str() {
        "dxp" | "dxb" | "document" => commands::write_document(key, &path, 1),
        "older" => commands::write_document(key, &path, 0),
        "brf" | "brl" => {
            if !braille {
                return Err("formatted braille is written from a braille document: translate first".into());
            }
            commands::write_formatted(key, &path, true, 0)
        }
        "prf" | "formatted" => {
            let choice = code_page.and_then(|cp| bc_core::codepages::NAMES.iter().position(|n| n.eq_ignore_ascii_case(cp))).unwrap_or(0);
            commands::write_formatted(key, &path, false, choice)
        }
        "pef" => {
            if !braille {
                return Err("PEF is written from a braille document: translate first".into());
            }
            commands::write_pef(key, &path)
        }
        "txt" | "bru" | "coded" => commands::write_coded(key, &path),
        "lbf" | "refreshable" => commands::write_refreshable(key, &path),
        "htm" | "html" => commands::write_text_export(key, &path, translation::html),
        "mathjax" => commands::write_text_export(key, &path, translation::mathjax),
        "tex" | "latex" => commands::write_text_export(key, &path, translation::latex),
        "md" | "markdown" if braille => commands::write_markdown_braille(key, &path),
        "md" | "markdown" => commands::write_text_export(key, &path, translation::markdown),
        "epub" | "ebrl" | "ebraille" => crate::ebraille::save_as(key, &path),
        other => return Err(format!("unknown export type {other}; types: dxp, dxb, older, brf, pef, prf, txt, bru, lbf, htm, mathjax, tex, md, epub, ebrl")),
    };
    if !written {
        return Err(format!("{} was not written (the program showed why)", path.display()));
    }
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(json!({"path": path.display().to_string(), "type": kind, "bytes": bytes}))
}

// ------------------------------------------------------------------ Global, MCP Server

fn named(d: &crate::dlg::Dlg, name: &str) -> Option<Window> {
    d.dialog.find_window_by_name(name)
}

/// What the dialog says about the server.
fn state_text() -> String {
    match port() {
        Some(p) => format!("Running on 127.0.0.1:{p}. Connect an agent with: braillecraft-cli mcp --connect {p}"),
        None => "Not running.".to_string(),
    }
}

/// Global, MCP Server: the server turned on or off now, its port, and whether it starts with the program.
pub fn dialog() {
    let s = ControlSettings::load();
    let d = crate::dlg::Dlg::open_xrc(&app::frame(), COMMAND_ID);
    if let Some(c) = named(&d, "CONTROL_RUNNING").and_then(|w| w.as_widget::<CheckBox>()) {
        c.set_value(port().is_some());
        c.set_focus();
    }
    if let Some(t) = named(&d, "CONTROL_PORT").and_then(|w| w.as_widget::<TextCtrl>()) {
        t.set_value(&port().unwrap_or(s.port).to_string());
        crate::speech::set_name(t.get_handle(), "Port");
    }
    if let Some(c) = named(&d, "CONTROL_AUTOSTART").and_then(|w| w.as_widget::<CheckBox>()) {
        c.set_value(s.start_with_program);
    }
    if let Some(t) = named(&d, "CONTROL_STATE").and_then(|w| w.as_widget::<StaticText>()) {
        t.set_label(&state_text());
    }
    if d.show() != 1 {
        return;
    }
    let run = named(&d, "CONTROL_RUNNING").and_then(|w| w.as_widget::<CheckBox>()).is_some_and(|c| c.is_checked());
    let auto = named(&d, "CONTROL_AUTOSTART").and_then(|w| w.as_widget::<CheckBox>()).is_some_and(|c| c.is_checked());
    let typed = named(&d, "CONTROL_PORT").and_then(|w| w.as_widget::<TextCtrl>()).map(|t| t.get_value()).unwrap_or_default();
    let Some(new_port) = typed.trim().parse::<u16>().ok().filter(|p| *p > 0) else {
        commands::message("The port must be a number from 1 to 65535.");
        return;
    };
    ControlSettings { port: new_port, start_with_program: auto }.save();
    match (run, port()) {
        (true, Some(p)) if p == new_port => {}
        (true, _) => {
            stop();
            match start(new_port) {
                Ok(p) => commands::message(&format!("The MCP server is running on 127.0.0.1:{p}.\nConnect an agent with: braillecraft-cli mcp --connect {p}")),
                Err(e) => commands::message(&format!("The MCP server could not be started: {e}")),
            }
        }
        (false, Some(_)) => stop(),
        (false, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_checked_before_they_reach_the_main_thread() {
        let key = "abc";
        let line = json!({"id": 7, "method": "ping", "params": {"a": 1}, "key": "abc"}).to_string();
        let (id, method, params) = parse_request(&line, key, true).unwrap();
        assert_eq!((id, method.as_str(), params["a"].as_i64()), (json!(7), "ping", Some(1)));
        // The wrong key, no key, and an empty expected key (no server) are all unauthorized.
        for bad in [json!({"id": 1, "method": "ping", "key": "x"}), json!({"id": 1, "method": "ping"})] {
            let r = parse_request(&bad.to_string(), key, true).unwrap_err();
            assert!(r.contains("unauthorized") && r.contains("\"id\":1"), "{r}");
        }
        assert!(parse_request(&line, "", true).unwrap_err().contains("unauthorized"));
        // No method, a stopped server, and a line that is not JSON.
        assert!(parse_request(&json!({"id": 2, "key": "abc"}).to_string(), key, true).unwrap_err().contains("missing method"));
        assert!(parse_request(&line, key, false).unwrap_err().contains("stopped"));
        let r = parse_request("{not json", key, true).unwrap_err();
        assert!(r.contains("malformed") && r.contains("\"id\":null"), "{r}");
        // Params that are not an object are taken as none.
        let (_, _, params) = parse_request(&json!({"id": 3, "method": "ping", "params": 5, "key": "abc"}).to_string(), key, true).unwrap();
        assert_eq!(params, json!({}));
    }

    #[test]
    fn key_names_give_wx_key_codes() {
        assert_eq!(key_code("Down"), Some(317));
        assert_eq!(key_code("F3"), Some(342));
        assert_eq!(key_code("f12"), Some(351));
        assert_eq!(key_code("F25"), None);
        assert_eq!(key_code("Enter"), Some(13));
        assert_eq!(key_code("Insert"), Some(322));
        assert_eq!(key_code("PageDown"), Some(367));
        assert_eq!(key_code("a"), Some(65));
        assert_eq!(key_code("7"), Some(55));
        assert_eq!(key_code("é"), None);
        assert_eq!(key_code("nothing"), None);
        assert_eq!(key_request(&json!({"key": "Ctrl+Shift+T"})).unwrap(), (84, 6));
        assert_eq!(key_request(&json!({"key": "F3", "alt": true})).unwrap(), (342, 1));
        assert_eq!(key_request(&json!({"key": "+"})).unwrap(), (43, 0));
        assert!(key_request(&json!({"key": "Ctrl+"})).is_err());
        assert!(key_request(&json!({})).is_err());
    }

    #[test]
    fn only_ui_methods_run_inside_a_dialog() {
        assert!(runs_while_busy("ui.key") && runs_while_busy("ui.screenshot") && runs_while_busy("ping"));
        assert!(!runs_while_busy("text.insert") && !runs_while_busy("engine.execute"));
        assert!(leaves_events("engine.execute") && leaves_events("ui.key") && !leaves_events("document.text"));
        assert!(runs_inside_request("ui.key") && !runs_inside_request("ui.screenshot"));
    }

    #[test]
    fn reply_lines_carry_the_id() {
        assert_eq!(reply(&json!(4), Ok(json!("pong"))), r#"{"id":4,"ok":true,"result":"pong"}"#);
        assert_eq!(reply(&json!("x"), Err("no".into())), r#"{"error":"no","id":"x","ok":false}"#);
    }

    #[test]
    fn keys_are_long_and_differ() {
        let (a, b) = (new_key(), new_key());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
