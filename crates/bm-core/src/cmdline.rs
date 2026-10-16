//! The command line, as the reference program reads it (re/spec/26-installer.md section 26.5): the arguments in order;
//! an option sets the mode that applies to every file after it, until another option changes it; a file argument may
//! hold wildcards (* and ?), one action per file found; settings options (copies, page size, template, prompt) apply
//! to the whole run. When at least one file comes with a mode and none without, the run is a batch: the program does
//! the work and ends without showing the editor.

use std::path::{Path, PathBuf};

/// What is done with a file given on the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// No option: the file opens in the editor.
    Open,
    /// /print: printed with File Print's dialog skipped (a braille document is translated first).
    Print,
    /// /braille[:setup]: embossed with File Emboss's dialog skipped (a print document is translated first), with the
    /// embosser setup named when one is given.
    Braille(Option<String>),
    /// /simulated (also /simulate, as the reference installer writes it): printed; a print document is translated
    /// first, so its braille prints.
    Simulated,
    /// /prf:<file>: saved as formatted text (a braille document is translated first).
    Prf(PathBuf),
    /// /brf:<file>: saved as formatted braille (a print document is translated first).
    Brf(PathBuf),
    /// /lbf:<file>: saved as refreshable braille.
    Lbf(PathBuf),
    /// An option the reference program has whose effect BrailleMaker does not carry out (its name, for the log):
    /// /RemoveEmbosserInfo, /RemovePrinterInfo, /RemovePageInfo, /ebrl, /dumpstyles, /dumptemplate.
    NotAvailable(String),
}

/// The /prompt setting: the answer given to "translation failed" while the run goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Prompt {
    /// Ask (the reference's preset reply 0).
    #[default]
    Ask,
    /// Go on with the next file (reply 1).
    Continue,
    /// Stop the run (reply 2).
    Stop,
}

/// One file and what is done with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub file: PathBuf,
    pub mode: Mode,
}

/// The command line read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandLine {
    pub actions: Vec<Action>,
    /// /copies:<n>.
    pub copies: Option<u32>,
    /// /pagesize:<rows>x<cols>: lines per page and cells (characters) per line.
    pub page_size: Option<(u32, u32)>,
    /// /template:<name>: the default template for the run.
    pub template: Option<String>,
    pub prompt: Prompt,
    /// /resetfilesavefolder and /resetfileopenfolder.
    pub reset_save_folder: bool,
    pub reset_open_folder: bool,
    /// Arguments that are neither an option nor a file found, and files that matched nothing, with the reference's
    /// log lines ("Ignoring command line parameter: %s", "Cannot open file %s").
    pub log: Vec<String>,
}

impl CommandLine {
    /// Whether the program does the work and ends without the editor: at least one file with a mode and none
    /// without.
    pub fn is_batch(&self) -> bool {
        self.actions.iter().any(|a| a.mode != Mode::Open) && !self.actions.iter().any(|a| a.mode == Mode::Open)
    }
}

/// An option name and its value: "/name", "-name", "/name:value" or "/name=value" (`equals` allows "="). None for
/// an argument that is not an option.
fn option(arg: &str, equals: bool) -> Option<(String, Option<String>)> {
    let rest = arg.strip_prefix('/').or_else(|| arg.strip_prefix('-'))?;
    let cut = rest.find(|c| c == ':' || (equals && c == '='));
    let (name, value) = match cut {
        Some(i) => (&rest[..i], Some(rest[i + 1..].to_string())),
        None => (rest, None),
    };
    (!name.is_empty()).then(|| (name.to_ascii_lowercase(), value))
}

/// Reads the arguments (without the program's own name). `expand` lists the files a file argument names (its
/// wildcards expanded); an empty list means nothing was found.
pub fn parse(args: &[String], expand: impl Fn(&str) -> Vec<PathBuf>) -> CommandLine {
    let mut cl = CommandLine::default();
    let mut mode = Mode::Open;
    for arg in args {
        if let Some((name, value)) = option(arg, true) {
            let path = || value.clone().filter(|v| !v.is_empty()).map(PathBuf::from);
            match name.as_str() {
                "print" => mode = Mode::Print,
                // /braille takes its embosser setup after ":" only (the reference's kind 3: no "=").
                "braille" => mode = Mode::Braille(option(arg, false).and_then(|(_, v)| v).filter(|v| !v.is_empty())),
                "simulated" | "simulate" => mode = Mode::Simulated,
                "removeembosserinfo" | "removeprinterinfo" | "removepageinfo" | "ebrl" | "dumpstyles" | "dumpstyles_xml" | "dumptemplate" => {
                    mode = Mode::NotAvailable(arg.clone())
                }
                "prf" | "brf" | "lbf" => match path() {
                    Some(p) => {
                        mode = match name.as_str() {
                            "prf" => Mode::Prf(p),
                            "brf" => Mode::Brf(p),
                            _ => Mode::Lbf(p),
                        }
                    }
                    None => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                },
                "copies" => match value.as_deref().and_then(|v| v.trim().parse::<i64>().ok()) {
                    Some(n) if n >= 1 => cl.copies = Some(n.min(u32::MAX as i64) as u32),
                    _ => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                },
                "pagesize" => {
                    let size = value.as_deref().and_then(|v| {
                        let (r, c) = v.split_once(|ch: char| !ch.is_ascii_digit())?;
                        Some((r.parse::<u32>().ok()?, c.parse::<u32>().ok()?))
                    });
                    match size {
                        Some((r, c)) if r > 0 && c > 0 => cl.page_size = Some((r, c)),
                        _ => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                    }
                }
                "template" => match value.filter(|v| !v.is_empty()) {
                    Some(t) => cl.template = Some(t),
                    None => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                },
                "prompt" => match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
                    Some("stop") => cl.prompt = Prompt::Stop,
                    Some("continue") => cl.prompt = Prompt::Continue,
                    Some("prompt") => cl.prompt = Prompt::Ask,
                    _ => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                },
                "resetfilesavefolder" => cl.reset_save_folder = true,
                "resetfileopenfolder" => cl.reset_open_folder = true,
                // Options of the reference program BrailleMaker has nothing for (its Word importer's style map, its
                // debugging and automation hooks, its window title): kept out of the run, as noted.
                "wordstylemap" | "debug" | "api" | "title" => cl.log.push(format!("Not used by BrailleMaker: {arg}")),
                // Not an option name: a file when it names one (on Linux and macOS a path starts with "/"); an unknown
                // name with no path in it is an option the program does not know.
                _ => {
                    if !expand(arg).is_empty() || name.contains(['/', '\\', '.']) {
                        expand_file(&mut cl, arg, &mode, &expand);
                    } else {
                        cl.log.push(format!("Ignoring command line parameter: {arg}"));
                    }
                }
            }
        } else {
            expand_file(&mut cl, arg, &mode, &expand);
        }
    }
    cl
}

fn expand_file(cl: &mut CommandLine, arg: &str, mode: &Mode, expand: &impl Fn(&str) -> Vec<PathBuf>) {
    let found = expand(arg);
    if found.is_empty() {
        cl.log.push(format!("Cannot open file {arg}"));
    }
    for file in found {
        cl.actions.push(Action { file, mode: mode.clone() });
    }
}

/// The files a file argument names, with * and ? in its last part matched in its folder (sorted by name); the
/// argument itself when it names a file.
pub fn expand_wildcards(arg: &str) -> Vec<PathBuf> {
    let path = Path::new(arg);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !name.contains(['*', '?']) {
        return if path.is_file() { vec![path.to_path_buf()] } else { Vec::new() };
    }
    let folder = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let pattern: Vec<char> = name.to_lowercase().chars().collect();
    let mut out: Vec<PathBuf> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_file())
                .filter(|e| e.file_name().to_str().is_some_and(|n| matches(&pattern, &n.to_lowercase().chars().collect::<Vec<_>>())))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Whether `name` matches `pattern` (* any run, ? one character).
fn matches(pattern: &[char], name: &[char]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some('*') => (0..=name.len()).any(|i| matches(&pattern[1..], &name[i..])),
        Some('?') => !name.is_empty() && matches(&pattern[1..], &name[1..]),
        Some(c) => name.first() == Some(c) && matches(&pattern[1..], &name[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| a.to_string()).collect()
    }

    // Files "exist" when they have an extension and are not named missing.
    fn found(arg: &str) -> Vec<PathBuf> {
        if arg.starts_with("missing") || !arg.contains('.') {
            Vec::new()
        } else {
            vec![PathBuf::from(arg)]
        }
    }

    #[test]
    fn a_mode_applies_to_the_files_after_it() {
        let cl = parse(&args(&["a.dxp", "/print", "b.dxp", "c.dxb", "/braille:Index Everest", "d.dxb", "/simulate", "e.dxp"]), found);
        let modes: Vec<(String, Mode)> = cl.actions.iter().map(|a| (a.file.display().to_string(), a.mode.clone())).collect();
        assert_eq!(
            modes,
            vec![
                ("a.dxp".into(), Mode::Open),
                ("b.dxp".into(), Mode::Print),
                ("c.dxb".into(), Mode::Print),
                ("d.dxb".into(), Mode::Braille(Some("Index Everest".into()))),
                ("e.dxp".into(), Mode::Simulated),
            ]
        );
        assert!(!cl.is_batch());
        assert!(parse(&args(&["/braille", "x.dxb"]), found).is_batch());
        assert_eq!(parse(&args(&["/braille", "x.dxb"]), found).actions[0].mode, Mode::Braille(None));
    }

    #[test]
    fn settings_values_and_the_log() {
        let cl = parse(&args(&["/copies:3", "/pagesize=25x40", "/template:English (UEB) - basic", "/prompt:continue", "-brf:out.brf", "x.dxp", "/frob", "missing.dxp", "/debug:1"]), found);
        assert_eq!((cl.copies, cl.page_size, cl.template.as_deref(), cl.prompt), (Some(3), Some((25, 40)), Some("English (UEB) - basic"), Prompt::Continue));
        assert_eq!(cl.actions[0].mode, Mode::Brf(PathBuf::from("out.brf")));
        assert_eq!(cl.log, vec!["Ignoring command line parameter: /frob".to_string(), "Cannot open file missing.dxp".into(), "Not used by BrailleMaker: /debug:1".into()]);
        // A path is a file, also when it starts with "/".
        assert_eq!(parse(&args(&["/print", "/home/x/a.dxp"]), found).actions[0].file, PathBuf::from("/home/x/a.dxp"));
        assert_eq!(parse(&args(&["/copies:0"]), found).log, vec!["Ignoring command line parameter: /copies:0".to_string()]);
        assert!(matches!(parse(&args(&["/RemovePageInfo", "x.dxp"]), found).actions[0].mode, Mode::NotAvailable(_)));
    }

    #[test]
    fn wildcards_match_names() {
        let p: Vec<char> = "*.dx?".chars().collect();
        assert!(matches(&p, &"report.dxb".chars().collect::<Vec<_>>()));
        assert!(!matches(&p, &"report.txt".chars().collect::<Vec<_>>()));
        assert!(expand_wildcards("no-such-folder/*.dxp").is_empty());
    }
}
