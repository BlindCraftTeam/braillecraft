//! BrailleCraft's MCP server (Model Context Protocol, JSON-RPC 2.0 over stdio), in the shape the ArtCraft apps
//! give theirs: a `Server` speaks the protocol to a `Backend`; the `Headless` backend runs a session over the core
//! with no window, and the `Remote` backend talks to a running desktop program through its control port
//! (`braillecraft-desktop --control PORT`), so an agent can also work on the documents the user has open.
//!
//! Tools: list_commands and execute (every menu command by id), translate, back_translate, format_document,
//! import_file, export_file, emboss_bytes, inspect_document, get_text, type_text, select_text, new_document,
//! open_document, save_document, undo, redo, list_tables, list_templates, list_embossers, and, through the
//! running program, screenshot, key and click.

pub mod backend;
pub mod headless;
pub mod server;
pub mod tools;

pub use backend::{Backend, Remote};
pub use headless::Headless;
pub use server::Server;

/// Methods that need the desktop program.
pub const NEEDS_APP: &str = "needs the desktop program: start `braillecraft-desktop --control 7991` and run `braillecraft-cli mcp --connect 127.0.0.1:7991`";

/// Base64 of bytes (standard alphabet, padded), for the job bytes and pictures a tool returns.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, &b)| acc | (u32::from(b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Bytes from base64 (standard alphabet, padding optional); None for other characters.
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
