//! JSON-RPC 2.0 framing and the MCP lifecycle, tools and resources methods (newline-delimited JSON on stdio; logs
//! go to stderr, stdout carries only the protocol).

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::backend::Backend;
use crate::tools::{call_tool, tool_definitions};

/// The protocol revision implemented; older clients are answered in their own.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: &[&str] = &[PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
const RESOURCE_NOT_FOUND: i64 = -32002;

const INSTRUCTIONS: &str = "BrailleCraft is a braille translation, formatting and embossing program. Documents are print \
(text with codes in square brackets, such as [es~h1.] for a style start) or braille (cells in North American ASCII \
braille). Typical flow: new_document or open_document (or import_file for Word, PDF, Markdown, BRF and the other \
kinds) → type_text or execute Layout menu codes → translate (print to braille) → format_document to see the pages → \
export_file {path:\"out.brf\"} or emboss_bytes. list_commands names every menu command; execute runs one by id. \
Codes in text are written in square brackets.";

/// Resource URIs.
pub const DOC_URI: &str = "braillecraft://document";
pub const COMMANDS_URI: &str = "braillecraft://commands";
pub const TABLES_URI: &str = "braillecraft://tables";
pub const TEMPLATES_URI: &str = "braillecraft://templates";
pub const EMBOSSERS_URI: &str = "braillecraft://embossers";

/// An MCP server bound to one backend.
pub struct Server {
    backend: Box<dyn Backend>,
    initialized: bool,
}

fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

impl Server {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self { backend, initialized: false }
    }

    pub fn backend(&mut self) -> &mut dyn Backend {
        self.backend.as_mut()
    }

    /// Whether the client has sent notifications/initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Serves newline-delimited JSON-RPC until `input` closes.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if let Some(reply) = self.handle_line(&line) {
                output.write_all(reply.as_bytes())?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// Handles one line; the reply line, or None for notifications and blank lines.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(batch)) => {
                // Batches were dropped from the protocol in 2025-06-18; older clients are still answered.
                if batch.is_empty() {
                    Some(error(Value::Null, INVALID_REQUEST, "empty batch"))
                } else {
                    let replies: Vec<Value> = batch.into_iter().filter_map(|m| self.handle(m)).collect();
                    (!replies.is_empty()).then_some(Value::Array(replies))
                }
            }
            Ok(msg) => self.handle(msg),
            Err(e) => Some(error(Value::Null, PARSE_ERROR, format!("parse error: {e}"))),
        };
        reply.map(|r| r.to_string())
    }

    /// Handles one JSON-RPC message; None for notifications and responses.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let Value::Object(o) = &msg else { return Some(error(Value::Null, INVALID_REQUEST, "message must be an object")) };
        let id = o.get("id").cloned();
        let Some(method) = o.get("method").and_then(Value::as_str) else {
            // A response to a server-to-client request (none are sent): ignored. Anything else is invalid.
            if o.contains_key("result") || o.contains_key("error") {
                return None;
            }
            return Some(error(id.unwrap_or(Value::Null), INVALID_REQUEST, "missing `method`"));
        };
        let params = o.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            self.notification(method);
            return None;
        };
        if !(id.is_string() || id.is_number()) {
            return Some(error(Value::Null, INVALID_REQUEST, "`id` must be a string or number"));
        }
        Some(match self.request(method, &params) {
            Ok(r) => response(id, r),
            Err((code, m)) => error(id, code, m),
        })
    }

    fn notification(&mut self, method: &str) {
        match method {
            "notifications/initialized" => self.initialized = true,
            "notifications/cancelled" | "notifications/progress" | "notifications/roots/list_changed" => {}
            other => eprintln!("braillecraft-cli mcp: ignoring notification {other}"),
        }
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION);
                let version = if SUPPORTED_VERSIONS.contains(&asked) { asked } else { PROTOCOL_VERSION };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}, "resources": {}},
                    "serverInfo": {"name": "braillecraft", "title": "BrailleCraft", "version": env!("CARGO_PKG_VERSION"), "websiteUrl": "https://github.com/BlindCraftTeam/braillecraft"},
                    "instructions": format!("{INSTRUCTIONS} Backend: {}.", self.backend.describe()),
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing tool `name`".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);
                Ok(call_tool(self.backend.as_mut(), name, &args).to_value())
            }
            "resources/list" => Ok(json!({"resources": [
                {"uri": DOC_URI, "name": "document", "title": "Open document", "description": "The document: kind, name, template, tables, size, caret, selection, open styles and status line (document.inspect)", "mimeType": "application/json"},
                {"uri": COMMANDS_URI, "name": "commands", "title": "Commands", "description": "Every menu command: id, label, menu, shortcut, and whether it runs without the desktop program (engine.commands)", "mimeType": "application/json"},
                {"uri": TABLES_URI, "name": "tables", "title": "Translation tables", "description": "The shipped translation tables by language (tables.list)", "mimeType": "application/json"},
                {"uri": TEMPLATES_URI, "name": "templates", "title": "Templates", "description": "The document templates (templates.list)", "mimeType": "application/json"},
                {"uri": EMBOSSERS_URI, "name": "embossers", "title": "Embossers", "description": "The embosser models (embossers.list)", "mimeType": "application/json"},
            ]})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing `uri`".to_string()))?;
                let v = match uri {
                    DOC_URI => self.backend.call("document.inspect", json!({})),
                    COMMANDS_URI => self.backend.call("engine.commands", json!({})),
                    TABLES_URI => self.backend.call("tables.list", json!({})),
                    TEMPLATES_URI => self.backend.call("templates.list", json!({})),
                    EMBOSSERS_URI => self.backend.call("embossers.list", json!({})),
                    _ => return Err((RESOURCE_NOT_FOUND, format!("resource not found: {uri}"))),
                }
                .map_err(|e| (INTERNAL_ERROR, e))?;
                Ok(json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}]}))
            }
            other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
        }
    }
}
