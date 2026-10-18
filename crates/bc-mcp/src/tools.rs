//! The MCP tools: their names, schemas and what backend method each one calls.

use serde_json::{json, Value};

use crate::backend::Backend;

#[derive(Clone, Debug, PartialEq)]
pub struct ToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

impl ToolResult {
    pub fn text(t: impl Into<String>) -> Self {
        Self { content: vec![json!({"type": "text", "text": t.into()})], is_error: false }
    }
    pub fn json(v: &Value) -> Self {
        Self::text(serde_json::to_string_pretty(v).unwrap_or_default())
    }
    pub fn error(t: impl Into<String>) -> Self {
        Self { content: vec![json!({"type": "text", "text": t.into()})], is_error: true }
    }
    pub fn image(png_base64: String, info: &Value) -> Self {
        Self {
            content: vec![json!({"type": "image", "data": png_base64, "mimeType": "image/png"}), json!({"type": "text", "text": info.to_string()})],
            is_error: false,
        }
    }
    pub fn to_value(&self) -> Value {
        json!({"content": self.content, "isError": self.is_error})
    }
}

fn s(desc: &str) -> Value {
    json!({"type": "string", "description": desc})
}
fn n(desc: &str) -> Value {
    json!({"type": "integer", "description": desc})
}
fn b(desc: &str) -> Value {
    json!({"type": "boolean", "description": desc})
}
fn obj(props: Value, req: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": req})
}
fn tool(name: &str, title: &str, desc: &str, schema: Value, read_only: bool) -> Value {
    json!({"name": name, "title": title, "description": desc, "inputSchema": schema, "annotations": {"title": title, "readOnlyHint": read_only, "openWorldHint": false}})
}

pub fn tool_definitions() -> Value {
    json!([
        tool("list_commands", "List commands", "Every BrailleCraft menu command: id, label, menu, shortcut, and whether it runs here without the desktop program. Filter with `query`.", obj(json!({"query": s("Case-insensitive filter on the id, label or menu")}), &[]), true),
        tool("execute", "Run a command", "Run one menu command by id (from list_commands), with its parameters where it takes any: e.g. {command: 1109} translates, {command: 1901} inserts a new-line code, {command: 1101, params: {braille: false, template: \"English (UEB) - basic\"}} starts a document.", obj(json!({"command": n("Command id"), "params": {"type": "object", "description": "Command parameters"}}), &["command"]), false),
        tool("new_document", "New document", "Start a print or braille document from a template (list_templates).", obj(json!({"braille": b("A braille document (default print)"), "template": s("Template name (default the program's default template)")}), &[]), false),
        tool("open_document", "Open", "Open a file as the document: a BrailleCraft document (.dxp, .dxb), or any file BrailleCraft imports (.brf, .bru, .txt, .prf, .docx, .pdf, .md, .pef, .obf ...), its kind found from its name and content, or given.", obj(json!({"path": s("File path"), "kind": s("Import kind to force: CodedText, FormattedText, CodedBraille, FormattedBraille, Orbit, Docx, Pdf, Markdown, Html, Pef"), "code_page": s("Code page of a text file (default Windows-1252)"), "template": s("Template whose styles an import uses")}), &["path"]), false),
        tool("import_file", "Import", "As open_document (kept for the name).", obj(json!({"path": s("File path"), "kind": s("Import kind"), "code_page": s("Code page"), "template": s("Template")}), &["path"]), false),
        tool("save_document", "Save", "Save the document as a BrailleCraft file (.dxp for print, .dxb for braille): in place, or at `path`.", obj(json!({"path": s("File path; omit to save in place")}), &[]), false),
        tool("export_file", "Export", "Write the document in another format, chosen by the extension of `path` or by `type`: brf, pef, bru, txt, prf, lbf, htm, tex, md, dxp, dxb; mathjax for HTML with MathJax; older for the older document format.", obj(json!({"path": s("File path"), "type": s("Type when the extension does not say"), "cells": n("Cells per line for a laid-out type (default the document's)"), "lines": n("Lines per page"), "code_page": s("Code page of a formatted text file")}), &["path"]), false),
        tool("get_text", "Document text", "The document's text with its codes in square brackets.", obj(json!({}), &[]), true),
        tool("inspect_document", "Inspect document", "The document: kind, name, template, tables, size, pages, caret, selection, the styles open at the caret and the status line.", obj(json!({}), &[]), true),
        tool("type_text", "Type text", "Insert text at the caret (replacing the selection). Codes may be written in square brackets, e.g. [p] or [es~h1.]; a line break is a new-line code.", obj(json!({"text": s("Text"), "at": n("Position to insert at instead of the caret")}), &["text"]), false),
        tool("select_text", "Select", "Select a range of the document, or the n-th occurrence of some text; the caret goes to the end of the selection.", obj(json!({"from": n("Start position"), "to": n("End position"), "text": s("Text to find"), "occurrence": n("1-based occurrence")}), &[]), false),
        tool("translate", "Translate", "Translate text to braille with the document's tables (or the document itself when no text is given: a print document becomes a braille document, as File, Translate). Braille is returned as North American ASCII braille, and as Unicode braille patterns too.", obj(json!({"text": s("Print text with codes in brackets; omit to translate the open document"), "language": s("Language name (list_tables) whose tables to use instead of the document's")}), &[]), false),
        tool("back_translate", "Back-translate", "Braille (North American ASCII, or Unicode braille patterns) to print with the document's back-translation tables.", obj(json!({"text": s("Braille text; omit to back-translate the open braille document"), "language": s("Language name whose tables to use")}), &[]), false),
        tool("format_document", "Lay out pages", "The document laid out into pages: each page's lines, as they would be embossed or printed.", obj(json!({"cells": n("Cells per line (default the document's)"), "lines": n("Lines per page"), "first_page": n("First page to return (1)"), "pages": n("How many pages to return (default 5; 0 for only the count)")}), &[]), true),
        tool("emboss_bytes", "Emboss job bytes", "The bytes an embosser would receive for the document (base64), for the Generic driver with the given page size; `port` sends them to a serial port or printer queue only when named.", obj(json!({"cells": n("Cells per line"), "lines": n("Lines per page"), "copies": n("Copies (1)"), "port": s("A port or printer to send the job to; omit to only return the bytes")}), &[]), false),
        tool("list_tables", "Translation tables", "The shipped translation tables by language and region.", obj(json!({"query": s("Filter on the language name")}), &[]), true),
        tool("list_templates", "Templates", "The document templates by name.", obj(json!({"query": s("Filter on the name")}), &[]), true),
        tool("list_embossers", "Embossers", "The embosser models the program knows.", obj(json!({"query": s("Filter on the model name")}), &[]), true),
        tool("undo", "Undo", "Undo the last change.", obj(json!({"steps": n("Steps (1)")}), &[]), false),
        tool("redo", "Redo", "Redo the last undone change.", obj(json!({"steps": n("Steps (1)")}), &[]), false),
        tool("screenshot", "Screenshot", "A picture of the program's window (desktop program only).", obj(json!({}), &[]), true),
        tool("key", "Press key", "Press a key with modifiers in the program's editing window (desktop program only), e.g. {key: \"Down\"} or {key: \"F3\", alt: true}.", obj(json!({"key": s("Key name"), "shift": b("Shift"), "alt": b("Alt"), "ctrl": b("Ctrl")}), &["key"]), false),
        tool("click", "Click", "Click at window coordinates (desktop program only).", obj(json!({"x": n("x"), "y": n("y")}), &["x", "y"]), false),
        tool("list_documents", "Open documents", "The documents open in the program and which is active (desktop program only).", obj(json!({}), &[]), true),
        tool("activate_document", "Activate document", "Make one of the open documents the active one, by its number from list_documents (desktop program only).", obj(json!({"index": n("Document number")}), &["index"]), false),
    ])
}

fn wrap(r: Result<Value, String>) -> ToolResult {
    match r {
        Ok(v) => ToolResult::json(&v),
        Err(e) => ToolResult::error(e),
    }
}

/// Runs a tool: each one is a backend method call.
pub fn call_tool(b: &mut dyn Backend, name: &str, a: &Value) -> ToolResult {
    let args = if a.is_null() { json!({}) } else { a.clone() };
    match name {
        "list_commands" => {
            let q = args.get("query").and_then(Value::as_str).unwrap_or("").to_lowercase();
            match b.call("engine.commands", json!({})) {
                Ok(Value::Array(v)) => {
                    let f: Vec<Value> = v
                        .into_iter()
                        .filter(|c| q.is_empty() || ["id", "label", "menu"].iter().any(|k| c.get(*k).map(|x| x.to_string().to_lowercase()).is_some_and(|x| x.contains(&q))))
                        .collect();
                    ToolResult::json(&Value::Array(f))
                }
                Ok(v) => ToolResult::json(&v),
                Err(e) => ToolResult::error(e),
            }
        }
        "execute" => match args.get("command").and_then(Value::as_i64) {
            Some(c) => wrap(b.call("engine.execute", json!({"command": c, "params": args.get("params").cloned().unwrap_or(json!({}))}))),
            None => ToolResult::error("missing `command` (a command id)"),
        },
        "new_document" => wrap(b.call("document.new", args)),
        "open_document" | "import_file" => wrap(b.call("document.open", args)),
        "save_document" => wrap(b.call("document.save", args)),
        "export_file" => wrap(b.call("document.export", args)),
        "get_text" => wrap(b.call("document.text", json!({}))),
        "inspect_document" => wrap(b.call("document.inspect", json!({}))),
        "type_text" => wrap(b.call("text.insert", args)),
        "select_text" => wrap(b.call("select", args)),
        "translate" => wrap(b.call("translate", args)),
        "back_translate" => wrap(b.call("back_translate", args)),
        "format_document" => wrap(b.call("document.format", args)),
        "emboss_bytes" => wrap(b.call("emboss.bytes", args)),
        "list_tables" => wrap(b.call("tables.list", args)),
        "list_templates" => wrap(b.call("templates.list", args)),
        "list_embossers" => wrap(b.call("embossers.list", args)),
        "undo" => wrap(b.call("undo", args)),
        "redo" => wrap(b.call("redo", args)),
        "screenshot" => match b.call("ui.screenshot", json!({})) {
            Ok(v) => match v.get("png").and_then(Value::as_str) {
                Some(png) if !png.is_empty() => ToolResult::image(png.to_string(), &json!({"width": v["width"], "height": v["height"]})),
                _ => ToolResult::error("the reply carried no picture"),
            },
            Err(e) => ToolResult::error(e),
        },
        "key" => wrap(b.call("ui.key", args)),
        "click" => wrap(b.call("ui.click", args)),
        "list_documents" => wrap(b.call("ui.documents", json!({}))),
        "activate_document" => wrap(b.call("ui.activate", args)),
        other => ToolResult::error(format!("unknown tool `{other}`")),
    }
}
