//! The command line, as it is read (re/spec/26-installer.md section 26.5): the arguments in order;
//! an option sets the mode that applies to every file after it, until another option changes it; a file argument may
//! hold wildcards (* and ?), one action per file found; settings options (copies, page size, template, prompt) apply
//! to the whole run. When at least one file comes with a mode and none without, the run is a batch: the program does
//! the work and ends without showing the editor.

use std::path::{Path, PathBuf};

pub use crate::dxfile::Remove;

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
    /// /simulated (also /simulate): printed; a print document is translated
    /// first, so its braille prints.
    Simulated,
    /// /prf:<file>: saved as formatted text (a braille document is translated first).
    Prf(PathBuf),
    /// /brf:<file>: saved as formatted braille (a print document is translated first).
    Brf(PathBuf),
    /// /lbf:<file>: saved as refreshable braille.
    Lbf(PathBuf),
    /// /ebrl=<file>: saved as eBraille (a print document is translated first) with the document's metadata, no
    /// quality notice and no metadata dialog.
    Ebrl(PathBuf),
    /// /RemoveEmbosserInfo, /RemovePrinterInfo, /RemovePageInfo: the document file is written again in its own
    /// place without the embosser part, the printer part, or both (dxfile::remove_info).
    RemoveInfo(Remove),
    /// /dumpstyles:<file>: the document's styles written to the file as plain text (templates::style_dump).
    DumpStyles(PathBuf),
    /// /dumptemplate:<file>: the document's styles written to the file as the XML style dump. As built, this option
    /// writes the XML dump, not a text template, and /dumpstyles_xml is not an option at all: its argument is taken
    /// as a file ("Cannot open file /dumpstyles_xml:...").
    DumpStylesXml(PathBuf),
}

/// The /prompt setting. It is not an answer to a question: it says where the run's messages go and what a failed
/// translation does with the document that was opened (after_failed_translation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Prompt {
    /// /prompt:prompt (0): messages are shown as message boxes.
    #[default]
    Ask,
    /// /prompt:continue (1): messages go to the error output; a failed translation closes the opened document.
    Continue,
    /// /prompt:stop (2): messages go to the error output. The run does not stop: the next file is handled as with
    /// the other two.
    Stop,
}

impl Prompt {
    /// Whether the run's messages are message boxes ("Error", warning icon; the usage "Note", information icon)
    /// rather than lines on the error output.
    pub fn shows_boxes(self) -> bool {
        self == Prompt::Ask
    }
}

/// After a translation a file's mode needed has failed: whether the document opened for the file is closed, and
/// whether the run goes on with the next file. The run always goes on; only /prompt:continue closes the opened
/// document (with /prompt:prompt and /prompt:stop it is left open, in the editor when the run is not a batch).
pub fn after_failed_translation(prompt: Prompt) -> (bool, bool) {
    (prompt == Prompt::Continue, true)
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
    /// --control PORT (also /control:PORT): BrailleCraft's own option, the control channel for an AI agent
    /// (braillecraft-cli mcp --connect PORT) opened on 127.0.0.1 at that port; 0 lets the system pick one.
    pub control: Option<u16>,
    /// The first argument asks for the usage (is_help): it is shown and the program ends, with nothing else done.
    pub help: bool,
    /// File arguments given after an option and with no option, each counted once for every file it names, or once
    /// when it names none: the batch decision (is_batch) counts the files that could not be found too.
    pub with_mode: usize,
    pub without_mode: usize,
    /// Arguments that are not taken and files that matched nothing, with their log lines ("Ignoring command line
    /// parameter: %s", "Cannot open file %s").
    pub log: Vec<String>,
}

impl CommandLine {
    /// Whether the program does the work and ends without the editor: at least one file argument with a mode and
    /// none without, a file that could not be found counted as one.
    pub fn is_batch(&self) -> bool {
        self.with_mode > 0 && self.without_mode == 0
    }
}

/// Whether the first argument asks for the usage: "/help" in any case, "?", or anything that starts with "/?".
/// "-help" and "--help", the form of Linux and macOS, are BrailleCraft's own. Only the first argument is looked at.
pub fn is_help(first: &str) -> bool {
    first.eq_ignore_ascii_case("/help")
        || first == "?"
        || first.starts_with("/?")
        || first.eq_ignore_ascii_case("-help")
        || first.eq_ignore_ascii_case("--help")
}

/// The usage text shown for is_help, with the program's name as it was given.
pub fn usage(program: &str) -> String {
    format!(
        "Usage: {program} [/print | /braille[:embosser-name] | /simulate | /brf:<filename> | /prf:<filename>] [/pagesize:<rows>x<cols>] [/copies:nn] [/prompt:prompt|continue|stop] [/template:<name>] [/wordstylemap:<name>] [<files>]"
    )
}

/// The number at the start of `v`, read as C's "%d" reads it: spaces skipped, a sign, then digits. The rest after
/// it, or None when there is no digit.
fn leading_number(v: &str) -> Option<(i64, &str)> {
    let t = v.trim_start();
    let (sign, digits) = match t.as_bytes().first() {
        Some(b'-') => (-1, &t[1..]),
        Some(b'+') => (1, &t[1..]),
        _ => (1, t),
    };
    let n = digits.bytes().take_while(u8::is_ascii_digit).count();
    if n == 0 {
        return None;
    }
    let value = digits[..n].parse::<i64>().unwrap_or(i64::MAX);
    Some((sign * value, &digits[n..]))
}

/// "/name" (also "-name" and "--name", BrailleCraft's own), without its lead; None for an argument that is not an
/// option.
fn option_text(arg: &str) -> Option<&str> {
    arg.strip_prefix('/').or_else(|| arg.strip_prefix("--")).or_else(|| arg.strip_prefix('-')).filter(|r| !r.is_empty())
}

/// An option's name and value, cut at the first ':' (and at the first '=' too when `equals`).
fn cut(rest: &str, equals: bool) -> (String, Option<String>) {
    match rest.find(|c| c == ':' || (equals && c == '=')) {
        Some(i) => (rest[..i].to_ascii_lowercase(), Some(rest[i + 1..].to_string())),
        None => (rest.to_ascii_lowercase(), None),
    }
}

/// Reads the arguments (without the program's own name). `expand` lists the files a file argument names (its
/// wildcards expanded); an empty list means nothing was found.
///
/// Each option is matched the way its kind says: /print, /simulated and the three remove options only as the whole
/// argument (no value); /braille and /prompt cut at ':' only; the options with a value (/copies, /pagesize,
/// /template, /prf, /brf, /lbf, /ebrl, /dumpstyles, /dumptemplate, /wordstylemap) cut at ':' or '='. Any other
/// argument is a file argument, also one that starts with '/' or '-': it is "Cannot open file ..." when it names no
/// file. A /copies or /pagesize value that is not a number goes on as a file argument too.
pub fn parse(args: &[String], expand: impl Fn(&str) -> Vec<PathBuf>) -> CommandLine {
    let mut cl = CommandLine::default();
    let mut mode = Mode::Open;
    let mut k = 0;
    if args.first().is_some_and(|a| is_help(a)) {
        cl.help = true;
        k = 1;
    }
    while k < args.len() {
        let arg = &args[k];
        k += 1;
        let Some(rest) = option_text(arg) else {
            expand_file(&mut cl, arg, &mode, &expand);
            continue;
        };
        let whole = rest.to_ascii_lowercase();
        let (colon, colon_value) = cut(rest, false);
        let (name, value) = cut(rest, true);
        let path = || value.clone().filter(|v| !v.is_empty()).map(PathBuf::from);
        match whole.as_str() {
            "print" => {
                mode = Mode::Print;
                continue;
            }
            // "/simulate" is the name the installer's Print verb uses.
            "simulated" | "simulate" => {
                mode = Mode::Simulated;
                continue;
            }
            "removeembosserinfo" => {
                mode = Mode::RemoveInfo(Remove::Embosser);
                continue;
            }
            "removeprinterinfo" => {
                mode = Mode::RemoveInfo(Remove::Printer);
                continue;
            }
            "removepageinfo" => {
                mode = Mode::RemoveInfo(Remove::Page);
                continue;
            }
            _ => {}
        }
        match colon.as_str() {
            // /braille takes its embosser setup after ":" only.
            "braille" => {
                mode = Mode::Braille(colon_value.filter(|v| !v.is_empty()));
                continue;
            }
            // A value that is none of the three leaves the setting as it is, with nothing said.
            "prompt" => {
                match colon_value.as_deref().map(str::to_ascii_lowercase).as_deref() {
                    Some("stop") => cl.prompt = Prompt::Stop,
                    Some("continue") => cl.prompt = Prompt::Continue,
                    Some("prompt") => cl.prompt = Prompt::Ask,
                    _ => {}
                }
                continue;
            }
            // Options BrailleCraft has nothing for (debugging and automation hooks, a window title): kept out of the
            // run, as noted.
            "debug" | "api" | "title" => {
                cl.log.push(format!("Not used by BrailleCraft: {arg}"));
                continue;
            }
            _ => {}
        }
        match name.as_str() {
            "prf" | "brf" | "lbf" | "ebrl" | "dumpstyles" | "dumptemplate" => match path() {
                Some(p) => {
                    mode = match name.as_str() {
                        "prf" => Mode::Prf(p),
                        "brf" => Mode::Brf(p),
                        "ebrl" => Mode::Ebrl(p),
                        "dumpstyles" => Mode::DumpStyles(p),
                        "dumptemplate" => Mode::DumpStylesXml(p),
                        _ => Mode::Lbf(p),
                    }
                }
                None => cl.log.push(format!("Ignoring command line parameter: {arg}")),
            },
            "copies" => match value.as_deref().and_then(leading_number) {
                Some((n, _)) if n >= 1 => cl.copies = Some(n.min(u32::MAX as i64) as u32),
                // BrailleCraft's own safeguard: no number of copies below 1.
                Some(_) => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                None => expand_file(&mut cl, arg, &mode, &expand),
            },
            // "%d%c%d": a number, any one character, a number.
            "pagesize" => {
                let size = value.as_deref().and_then(|v| {
                    let (r, after) = leading_number(v)?;
                    let mut chars = after.chars();
                    chars.next()?;
                    let (c, _) = leading_number(chars.as_str())?;
                    Some((r, c))
                });
                match size {
                    Some((r, c)) if r > 0 && c > 0 => cl.page_size = Some((r.min(u32::MAX as i64) as u32, c.min(u32::MAX as i64) as u32)),
                    // BrailleCraft's own safeguard: no page of no lines or no cells.
                    Some(_) => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                    None => expand_file(&mut cl, arg, &mode, &expand),
                }
            }
            // With no name nothing is set and nothing is said.
            "template" => {
                if let Some(t) = value.filter(|v| !v.is_empty()) {
                    cl.template = Some(t);
                }
            }
            "wordstylemap" => cl.log.push(format!("Not used by BrailleCraft: {arg}")),
            "resetfilesavefolder" => cl.reset_save_folder = true,
            "resetfileopenfolder" => cl.reset_open_folder = true,
            // --control PORT, --control=PORT or /control:PORT (BrailleCraft's own).
            "control" => {
                let port = match value.as_deref() {
                    Some(v) => v.trim().parse::<u16>().ok(),
                    None => match args.get(k).and_then(|v| v.trim().parse::<u16>().ok()) {
                        Some(p) => {
                            k += 1;
                            Some(p)
                        }
                        None => None,
                    },
                };
                match port {
                    Some(p) => cl.control = Some(p),
                    None => cl.log.push(format!("Ignoring command line parameter: {arg}")),
                }
            }
            // Not an option name: a file argument (on Linux and macOS a path starts with "/").
            _ => expand_file(&mut cl, arg, &mode, &expand),
        }
    }
    cl
}

/// A file argument: one action for each file it names, each counted under the mode in force; one that names none is
/// "Cannot open file ..." and is counted once all the same.
fn expand_file(cl: &mut CommandLine, arg: &str, mode: &Mode, expand: &impl Fn(&str) -> Vec<PathBuf>) {
    let found = expand(arg);
    let count = found.len().max(1);
    if *mode == Mode::Open {
        cl.without_mode += count;
    } else {
        cl.with_mode += count;
    }
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
        assert_eq!(cl.log, vec!["Cannot open file /frob".to_string(), "Cannot open file missing.dxp".into(), "Not used by BrailleCraft: /debug:1".into()]);
        // A path is a file, also when it starts with "/".
        assert_eq!(parse(&args(&["/print", "/home/x/a.dxp"]), found).actions[0].file, PathBuf::from("/home/x/a.dxp"));
        assert_eq!(parse(&args(&["/copies:0"]), found).log, vec!["Ignoring command line parameter: /copies:0".to_string()]);
        assert_eq!(parse(&args(&["/RemovePageInfo", "x.dxp"]), found).actions[0].mode, Mode::RemoveInfo(Remove::Page));
    }

    #[test]
    fn parse_remove_options() {
        let modes: Vec<Mode> = ["/RemoveEmbosserInfo", "/removeprinterinfo", "/REMOVEPAGEINFO"]
            .iter()
            .map(|o| parse(&args(&[o, "x.dxp"]), found).actions[0].mode.clone())
            .collect();
        assert_eq!(modes, vec![Mode::RemoveInfo(Remove::Embosser), Mode::RemoveInfo(Remove::Printer), Mode::RemoveInfo(Remove::Page)]);
        assert!(parse(&args(&["/RemovePageInfo", "x.dxp"]), found).is_batch());
        // These options take no value: with one the argument is a file argument.
        assert_eq!(parse(&args(&["/RemovePageInfo:x"]), found).log, vec!["Cannot open file /RemovePageInfo:x".to_string()]);
        assert_eq!(parse(&args(&["/print:x"]), found).log, vec!["Cannot open file /print:x".to_string()]);
    }

    #[test]
    fn parse_dump_options_as_built() {
        assert_eq!(parse(&args(&["/dumpstyles:s.txt", "x.dxp"]), found).actions[0].mode, Mode::DumpStyles(PathBuf::from("s.txt")));
        assert_eq!(parse(&args(&["/dumptemplate=t.xml", "x.dxp"]), found).actions[0].mode, Mode::DumpStylesXml(PathBuf::from("t.xml")));
        // /dumpstyles_xml is not an option: its argument is a file argument, under the mode in force.
        let cl = parse(&args(&["/dumpstyles_xml:styles", "x.dxp"]), found);
        assert_eq!(cl.log, vec!["Cannot open file /dumpstyles_xml:styles".to_string()]);
        assert_eq!(cl.actions[0].mode, Mode::Open);
    }

    #[test]
    fn unknown_option_is_a_file() {
        let cl = parse(&args(&["/print", "/frob"]), found);
        assert_eq!(cl.log, vec!["Cannot open file /frob".to_string()]);
        assert!(cl.is_batch());
    }

    #[test]
    fn missing_file_with_a_mode_is_still_a_batch() {
        assert!(parse(&args(&["/print", "missing.dxp"]), found).is_batch());
        assert!(!parse(&args(&["missing.dxp"]), found).is_batch());
        assert!(!parse(&args(&["missing.dxp", "/print", "a.dxp"]), found).is_batch());
        assert!(!parse(&args(&["/copies:2"]), found).is_batch());
    }

    #[test]
    fn bad_copies_value_is_a_file_argument() {
        assert_eq!(parse(&args(&["/copies:abc"]), found).log, vec!["Cannot open file /copies:abc".to_string()]);
        assert_eq!(parse(&args(&["/pagesize:big"]), found).log, vec!["Cannot open file /pagesize:big".to_string()]);
        // "%d" reads the number at the start.
        assert_eq!(parse(&args(&["/copies:3x"]), found).copies, Some(3));
        assert_eq!(parse(&args(&["/pagesize:25 40"]), found).page_size, Some((25, 40)));
    }

    #[test]
    fn prompt_unknown_value_is_silent() {
        let cl = parse(&args(&["/prompt:maybe"]), found);
        assert!(cl.log.is_empty());
        assert_eq!(cl.prompt, Prompt::Ask);
        assert!(parse(&args(&["/template"]), found).log.is_empty());
        // /prompt cuts at ':' only.
        assert_eq!(parse(&args(&["/prompt=stop"]), found).log, vec!["Cannot open file /prompt=stop".to_string()]);
        assert_eq!(parse(&args(&["/prompt:STOP"]), found).prompt, Prompt::Stop);
    }

    #[test]
    fn help_is_the_first_argument_only() {
        for a in ["/help", "/HELP", "?", "/?x", "--help"] {
            assert!(parse(&args(&[a]), found).help, "{a}");
        }
        let cl = parse(&args(&["a.dxp", "/help"]), found);
        assert!(!cl.help);
        assert_eq!(cl.log, vec!["Cannot open file /help".to_string()]);
        // The rest of the line is still read (a /prompt after /help decides where the usage goes).
        assert_eq!(parse(&args(&["/help", "/prompt:continue"]), found).prompt, Prompt::Continue);
    }

    #[test]
    fn usage_text_has_the_program_name() {
        assert_eq!(
            usage("braillecraft"),
            "Usage: braillecraft [/print | /braille[:embosser-name] | /simulate | /brf:<filename> | /prf:<filename>] [/pagesize:<rows>x<cols>] [/copies:nn] [/prompt:prompt|continue|stop] [/template:<name>] [/wordstylemap:<name>] [<files>]"
        );
    }

    #[test]
    fn failed_translation_never_stops_the_run() {
        assert_eq!(after_failed_translation(Prompt::Continue), (true, true));
        assert_eq!(after_failed_translation(Prompt::Stop), (false, true));
        assert_eq!(after_failed_translation(Prompt::Ask), (false, true));
        assert!(Prompt::Ask.shows_boxes());
        assert!(!Prompt::Continue.shows_boxes() && !Prompt::Stop.shows_boxes());
    }

    #[test]
    fn ebrl_with_a_path_saves_ebraille() {
        let cl = parse(&args(&["/ebrl=out.ebrl", "x.dxp"]), found);
        assert_eq!(cl.actions[0].mode, Mode::Ebrl(PathBuf::from("out.ebrl")));
        assert!(cl.is_batch());
        assert_eq!(parse(&args(&["/ebrl:o.ebrl", "x.dxb"]), found).actions[0].mode, Mode::Ebrl(PathBuf::from("o.ebrl")));
        assert_eq!(parse(&args(&["/ebrl", "x.dxb"]), found).log, vec!["Ignoring command line parameter: /ebrl".to_string()]);
    }

    #[test]
    fn the_control_port_in_its_three_forms() {
        assert_eq!(parse(&args(&["--control", "7991", "a.dxp"]), found).control, Some(7991));
        assert_eq!(parse(&args(&["--control=7000"]), found).control, Some(7000));
        assert_eq!(parse(&args(&["/control:0"]), found).control, Some(0));
        let cl = parse(&args(&["--control", "a.dxp"]), found);
        assert_eq!(cl.control, None);
        assert_eq!(cl.actions.len(), 1);
        assert!(cl.log.iter().any(|l| l.contains("--control")), "{:?}", cl.log);
        // "--print" is the /print option, as "-print" is.
        assert_eq!(parse(&args(&["--print", "a.dxp"]), found).actions[0].mode, Mode::Print);
    }

    #[test]
    fn wildcards_match_names() {
        let p: Vec<char> = "*.dx?".chars().collect();
        assert!(matches(&p, &"report.dxb".chars().collect::<Vec<_>>()));
        assert!(!matches(&p, &"report.txt".chars().collect::<Vec<_>>()));
        assert!(expand_wildcards("no-such-folder/*.dxp").is_empty());
    }
}
