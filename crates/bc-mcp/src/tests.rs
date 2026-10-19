//! The server over a headless session on the repository's data: the lifecycle, the tool list, and the tools an
//! agent would call in order.

use serde_json::{json, Value};

use crate::headless::{DataDirs, Headless};
use crate::server::Server;

fn server() -> Server {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    Server::new(Box::new(Headless::open(DataDirs::under(&root)).unwrap()))
}

fn call(s: &mut Server, id: u64, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
    serde_json::from_str(&s.handle_line(&line).expect("a reply")).unwrap()
}

fn tool(s: &mut Server, name: &str, args: Value) -> (bool, String) {
    let r = call(s, 9, "tools/call", json!({"name": name, "arguments": args}));
    let result = &r["result"];
    (result["isError"].as_bool().unwrap_or(true), result["content"][0]["text"].as_str().unwrap_or("").to_string())
}

#[test]
fn lifecycle_and_tool_list() {
    let mut s = server();
    let r = call(&mut s, 1, "initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}));
    assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(r["result"]["serverInfo"]["name"], "braillecraft");
    assert!(s.handle_line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string()).is_none());
    assert!(s.is_initialized());
    let r = call(&mut s, 2, "tools/list", json!({}));
    let names: Vec<&str> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["list_commands", "execute", "translate", "back_translate", "format_document", "export_file", "import_file", "emboss_bytes", "list_tables", "list_templates", "list_embossers", "inspect_document", "type_text", "select_text", "undo", "redo", "screenshot", "key"] {
        assert!(names.contains(&n), "{n} missing from {names:?}");
    }
    let r = call(&mut s, 3, "resources/list", json!({}));
    assert_eq!(r["result"]["resources"].as_array().unwrap().len(), 5);
    let r = call(&mut s, 4, "nothing/here", json!({}));
    assert_eq!(r["error"]["code"], -32601);
    assert_eq!(serde_json::from_str::<Value>(&s.handle_line("{not json").unwrap()).unwrap()["error"]["code"], -32700);
}

#[test]
fn commands_text_translation_and_pages() {
    let mut s = server();
    let (err, text) = tool(&mut s, "list_commands", json!({"query": "translate"}));
    assert!(!err, "{text}");
    let list: Vec<Value> = serde_json::from_str(&text).unwrap();
    assert!(list.iter().any(|c| c["id"] == 1109 && c["headless"] == true), "{text}");
    let (err, text) = tool(&mut s, "type_text", json!({"text": "[es~h1.]Title[ee~h1.][es~para.]The cat sat.[ee~para.]"}));
    assert!(!err, "{text}");
    let (err, text) = tool(&mut s, "inspect_document", json!({}));
    assert!(!err, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["kind"], "print");
    assert_eq!(v["template"], "English (UEB) - basic");
    assert!(v["characters"].as_u64().unwrap() > 20, "{text}");
    // Translation of given text, then of the document (which becomes braille).
    let (err, text) = tool(&mut s, "translate", json!({"text": "The cat sat."}));
    assert!(!err, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["braille"], ",! cat sat4", "{text}");
    assert_eq!(v["unicode"].as_str().unwrap().chars().next().map(|c| c as u32), Some(0x2820));
    let (err, text) = tool(&mut s, "execute", json!({"command": 1109}));
    assert!(!err, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["kind"], "braille");
    let (err, text) = tool(&mut s, "format_document", json!({"cells": 40, "lines": 25}));
    assert!(!err, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["pages"], 1, "{text}");
    let lines = v["shown"][0]["lines"].as_array().unwrap();
    assert!(lines.iter().any(|l| l.as_str().unwrap().contains(",! cat sat4")), "{text}");
    let (err, text) = tool(&mut s, "back_translate", json!({"text": ",! cat sat4"}));
    assert!(!err, "{text}");
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap()["print"], "The cat sat.");
    // Emboss bytes for the braille document.
    let (err, text) = tool(&mut s, "emboss_bytes", json!({"cells": 40, "lines": 25}));
    assert!(!err, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(v["bytes"].as_u64().unwrap() > 10, "{text}");
    assert_eq!(crate::unbase64(v["base64"].as_str().unwrap()).unwrap().len() as u64, v["bytes"].as_u64().unwrap());
    // Undo brings the print document back.
    let (err, text) = tool(&mut s, "undo", json!({}));
    assert!(!err, "{text}");
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap()["kind"], "print");
    // A command that needs the program says so.
    let (err, text) = tool(&mut s, "execute", json!({"command": 3202}));
    assert!(err && text.contains("desktop program"), "{text}");
    let (err, text) = tool(&mut s, "screenshot", json!({}));
    assert!(err, "{text}");
}

#[test]
fn files_in_and_out() {
    let mut s = server();
    let dir = std::env::temp_dir().join(format!("braillecraft-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let md = dir.join("notes.md");
    std::fs::write(&md, "# Heading\n\nA paragraph with *emphasis*.\n\n- one\n- two\n").unwrap();
    let (err, text) = tool(&mut s, "open_document", json!({"path": md.display().to_string()}));
    assert!(!err, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["kind"], "print");
    assert_eq!(v["name"], "notes.md");
    let (_, text) = tool(&mut s, "get_text", json!({}));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(v["text"].as_str().unwrap().contains("[es~h1.]Heading[ee~h1.]"), "{text}");
    for (name, kind) in [("out.dxp", "dxp"), ("out.htm", "htm"), ("out.md", "md"), ("out.tex", "tex"), ("out.txt", "txt"), ("out.prf", "prf"), ("out.lbf", "lbf"), ("out.epub", "epub")] {
        let p = dir.join(name);
        let (err, text) = tool(&mut s, "export_file", json!({"path": p.display().to_string()}));
        assert!(!err, "{kind}: {text}");
        assert!(std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false), "{kind} not written");
    }
    let saved = dir.join("saved.dxp");
    let (err, text) = tool(&mut s, "save_document", json!({"path": saved.display().to_string()}));
    assert!(!err, "{text}");
    let (err, text) = tool(&mut s, "open_document", json!({"path": saved.display().to_string()}));
    assert!(!err, "{text}");
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap()["name"], "saved.dxp");
    // Translated and written as BRF and PEF.
    let (err, text) = tool(&mut s, "translate", json!({}));
    assert!(!err, "{text}");
    for name in ["out.brf", "out.pef", "out.bru", "braille.md", "out.ebrl"] {
        let p = dir.join(name);
        let (err, text) = tool(&mut s, "export_file", json!({"path": p.display().to_string()}));
        assert!(!err, "{name}: {text}");
    }
    let brf = std::fs::read_to_string(dir.join("out.brf")).unwrap();
    // The heading and the paragraph both contracted: the h1 style holds no grade code, and the template's
    // initial codes are in force from the start.
    assert!(brf.contains(",h1d+") && brf.contains(",a p>agraph") && brf.contains('\u{c}'), "{brf:?}");
    // The EPUB 3 and eBraille packages: zips whose package.opf names its identifier's id.
    for name in ["out.epub", "out.ebrl"] {
        let parts = bc_core::docx::parts(&std::fs::read(dir.join(name)).unwrap()).unwrap();
        let opf = String::from_utf8(bc_core::docx::part(&parts, "package.opf").unwrap().to_vec()).unwrap();
        assert!(opf.contains("unique-identifier=\"pub-id\"") && opf.contains("<dc:identifier id=\"pub-id\">"), "{name}: {opf}");
    }
    let pef = std::fs::read_to_string(dir.join("out.pef")).unwrap();
    assert!(pef.contains("<row>") && pef.contains('\u{2800}'), "{pef}");
    // Lists.
    let (err, text) = tool(&mut s, "list_tables", json!({"query": "english"}));
    assert!(!err && text.contains("English") && text.contains("UEB"), "{text}");
    let (err, text) = tool(&mut s, "list_templates", json!({"query": "espanol"}));
    assert!(!err && text.contains("Espanol"), "{text}");
    let (err, text) = tool(&mut s, "list_embossers", json!({"query": "index"}));
    assert!(!err && text.contains("Index"), "{text}");
    // A read root keeps files outside it out.
    let mut confined = Server::new(Box::new(Headless::open(DataDirs::under(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))).unwrap().with_roots(crate::headless::Roots { read: vec![dir.join("nowhere")], write: vec![] })));
    let (err, text) = tool(&mut confined, "open_document", json!({"path": md.display().to_string()}));
    assert!(err && text.contains("outside"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
