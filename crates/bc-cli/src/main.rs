//! braillecraft-cli: BrailleCraft from the command line. Today: `mcp`, the MCP server for AI agents.
//!
//!   braillecraft-cli mcp [--in FILE] [--automation-read-root DIR]... [--automation-write-root DIR]...
//!       the headless server over a session in this process (a new print document, or FILE opened)
//!   braillecraft-cli mcp --connect 127.0.0.1:7991
//!       the server bridged to the running desktop program started with --control 7991
//!
//! Add it to Claude Code with `claude mcp add braillecraft -- braillecraft-cli mcp`.

use std::path::PathBuf;

use bc_mcp::headless::{DataDirs, Headless, Roots};
use bc_mcp::{Backend, Remote, Server};

const USAGE: &str = "usage:
  braillecraft-cli mcp [--in FILE] [--automation-read-root DIR]... [--automation-write-root DIR]...
  braillecraft-cli mcp --connect HOST:PORT
  braillecraft-cli --version";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => match mcp(&args[1..]) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("braillecraft-cli: {e}");
                std::process::exit(1);
            }
        },
        Some("--version") | Some("-V") => println!("braillecraft-cli {}", env!("CARGO_PKG_VERSION")),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn mcp(args: &[String]) -> Result<(), String> {
    let mut open: Option<PathBuf> = None;
    let mut connect: Option<String> = None;
    let mut roots = Roots::default();
    let mut k = 0;
    while k < args.len() {
        let value = |k: &mut usize| -> Result<String, String> {
            *k += 1;
            args.get(*k).cloned().ok_or_else(|| format!("{} needs a value\n{USAGE}", args[*k - 1]))
        };
        match args[k].as_str() {
            "--in" => open = Some(PathBuf::from(value(&mut k)?)),
            "--connect" => {
                let v = value(&mut k)?;
                connect = Some(if v.contains(':') { v } else { format!("127.0.0.1:{v}") });
            }
            "--automation-read-root" => roots.read.push(PathBuf::from(value(&mut k)?)),
            "--automation-write-root" => roots.write.push(PathBuf::from(value(&mut k)?)),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
        k += 1;
    }
    let backend: Box<dyn Backend> = match connect {
        Some(addr) => Box::new(Remote::connect(&addr).map_err(|e| format!("cannot connect to the BrailleCraft program at {addr}: {e}"))?),
        None => {
            let dirs = DataDirs::find()?;
            let mut session = Headless::open(dirs)?.with_roots(roots);
            if let Some(path) = open {
                session.call("document.open", serde_json::json!({"path": path.display().to_string()}))?;
            }
            Box::new(session)
        }
    };
    let mut server = Server::new(backend);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    server.serve(stdin.lock(), stdout.lock()).map_err(|e| e.to_string())
}
