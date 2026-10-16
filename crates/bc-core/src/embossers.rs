//! The embosser models (embossers/embossers.txt, re/spec/25-emboss.md section 3): what each model can do and how it
//! is driven. A model has one or more names (with an Index firmware range on some), properties (flags, numbers,
//! texts, words, encodings, serial port blocks) and paper forms. Property names are matched without regard to case.
//! The model "Generic" holds the values every model starts from; a model without a Driver uses the Generic driver.
//! A user's own file (myembossers.txt) may replace models by name.

use std::path::Path;

/// One property value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A property named alone.
    Flag,
    Number(i64),
    /// A value in double quotes.
    Text(String),
    /// A bare word (Handshaking = Hardware).
    Word(String),
    /// A HardwareDCB or SoftwareDCB block: its name = value lines.
    Block(Vec<(String, String)>),
}

/// A paper form a model can use. Sizes are in tenths of an inch; cells and lines are (cells, lines).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub id: String,
    pub width: u32,
    pub length: u32,
    /// A folded booklet sheet: two pages side by side.
    pub double_page: bool,
    /// Cells the form's first cell is offset by (SinglePage 1).
    pub cell_offset: u32,
    /// The largest and the recommended page, for braille alone.
    pub maximum: (u32, u32),
    pub recommended: (u32, u32),
    /// For models with interline print: the largest and recommended page with interline print.
    pub interline: Option<((u32, u32), (u32, u32))>,
    /// The least margins (tenths of a millimetre) set before the form: top, left, right, bottom.
    pub margins: [u32; 4],
}

/// One name of a model, with the Index firmware versions it applies to (V2, V3_early-V3), if given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub name: String,
    pub versions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub names: Vec<Name>,
    pub properties: Vec<(String, Value)>,
    pub forms: Vec<Form>,
}

impl Model {
    /// The first value of property `key` (any case).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.properties.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn number(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// A text or word value.
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Text(t) | Value::Word(t) => Some(t),
            _ => None,
        }
    }

    /// The encodings the model can emboss in, its default first.
    pub fn encodings(&self) -> Vec<&str> {
        self.properties
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("Encoding"))
            .filter_map(|(_, v)| match v {
                Value::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    /// The model's driver: its Driver property, else the Generic driver.
    pub fn driver(&self) -> &str {
        self.text("Driver").unwrap_or("Emb_GenericEmbosserDriver")
    }

    pub fn has_name(&self, name: &str) -> bool {
        self.names.iter().any(|n| n.name == name)
    }
}

/// A parse error: the line (from 1) and what was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

/// The line without a # comment, keeping a # inside double quotes.
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn value(text: &str) -> Value {
    let t = text.trim();
    if let Some(q) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Value::Text(q.to_string())
    } else if let Ok(n) = t.parse::<i64>() {
        Value::Number(n)
    } else {
        Value::Word(t.to_string())
    }
}

/// "C x L" as (cells, lines).
fn pair(text: &str) -> Option<(u32, u32)> {
    let (a, b) = text.split_once(['x', 'X'])?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// A Form line after the word Form.
fn form(rest: &str, margins: [u32; 4]) -> Option<Form> {
    let rest = rest.trim();
    let rest = rest.strip_prefix('"')?;
    let (id, rest) = rest.split_once('"')?;
    let rest = rest.trim().strip_prefix('=')?;
    let parts: Vec<&str> = rest.split(';').map(str::trim).collect();
    let (width, length) = pair(parts.first()?)?;
    let mut kind = parts.get(1)?.split_whitespace();
    let double_page = match kind.next()? {
        k if k.eq_ignore_ascii_case("SinglePage") => false,
        k if k.eq_ignore_ascii_case("DoublePage") => true,
        _ => return None,
    };
    let cell_offset = kind.next().map(|o| o.parse().ok()).unwrap_or(Some(0))?;
    let named = |s: &str, key: &str| -> Option<(u32, u32)> {
        let (k, v) = s.split_once('=')?;
        k.trim().eq_ignore_ascii_case(key).then(|| pair(v)).flatten()
    };
    let (maximum, recommended, interline) = if parts.get(2).is_some_and(|p| p.contains('=')) {
        // BrailleOnly = C x L ; C x L ; Interline = C x L ; C x L
        let bmax = named(parts.get(2)?, "BrailleOnly")?;
        let brec = pair(parts.get(3)?)?;
        let imax = named(parts.get(4)?, "Interline")?;
        let irec = pair(parts.get(5)?)?;
        (bmax, brec, Some((imax, irec)))
    } else {
        (pair(parts.get(2)?)?, pair(parts.get(3)?)?, None)
    };
    Some(Form { id: id.to_string(), width, length, double_page, cell_offset, maximum, recommended, interline, margins })
}

/// The models of an embosser model file.
pub fn parse(text: &str) -> Result<Vec<Model>, Error> {
    let mut models = Vec::new();
    let mut names: Vec<Name> = Vec::new();
    let mut current: Option<Model> = None;
    let mut margins = [0u32; 4];
    // A HardwareDCB / SoftwareDCB block: its property name, whether its { was seen, and its lines.
    let mut block: Option<(String, bool, Vec<(String, String)>)> = None;
    // A form line that ends with ; goes on on the next lines; its text so far and its first line.
    let mut pending: Option<(String, usize)> = None;
    let err = |line: usize, message: &str| Error { line, message: message.to_string() };
    for (i, raw) in text.lines().enumerate() {
        let mut n = i + 1;
        let stripped = strip_comment(raw).trim();
        if stripped.is_empty() {
            continue;
        }
        let joined;
        let line = match pending.take() {
            Some((mut so_far, first)) => {
                so_far.push(' ');
                so_far.push_str(stripped);
                if stripped.ends_with(';') {
                    pending = Some((so_far, first));
                    continue;
                }
                n = first;
                joined = so_far;
                joined.as_str()
            }
            None if stripped.ends_with(';') && current.is_some() && stripped.get(..4).is_some_and(|w| w.eq_ignore_ascii_case("Form")) => {
                pending = Some((stripped.to_string(), n));
                continue;
            }
            None => stripped,
        };
        if let Some((key, open, mut lines)) = block.take() {
            if !open {
                if line != "{" {
                    return Err(err(n, "a { must follow a serial port block name"));
                }
                block = Some((key, true, lines));
            } else if line == "}" {
                if let Some(m) = current.as_mut() {
                    m.properties.push((key, Value::Block(lines)));
                }
            } else {
                let (k, v) = line.split_once('=').ok_or_else(|| err(n, "a serial port setting needs name = value"))?;
                lines.push((k.trim().to_string(), v.trim().to_string()));
                block = Some((key, true, lines));
            }
            continue;
        }
        let Some(model) = current.as_mut() else {
            if line == "{" {
                if names.is_empty() {
                    return Err(err(n, "a model needs a name before its {"));
                }
                current = Some(Model { names: std::mem::take(&mut names), properties: Vec::new(), forms: Vec::new() });
                margins = [0; 4];
                continue;
            }
            let rest = line.strip_prefix('"').ok_or_else(|| err(n, "expected a model name in double quotes"))?;
            let (name, versions) = rest.split_once('"').ok_or_else(|| err(n, "a model name is missing its closing quote"))?;
            let versions = versions.trim();
            names.push(Name { name: name.to_string(), versions: (!versions.is_empty()).then(|| versions.to_string()) });
            continue;
        };
        if line == "}" {
            if let Some(m) = current.take() {
                models.push(m);
            }
            continue;
        }
        let word_end = line.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(line.len());
        let (key, rest) = line.split_at(word_end);
        if key.is_empty() {
            return Err(err(n, "expected a property name"));
        }
        if key.eq_ignore_ascii_case("Form") {
            model.forms.push(form(rest, margins).ok_or_else(|| err(n, "a form line is not written properly"))?);
            continue;
        }
        if key.eq_ignore_ascii_case("HardwareDCB") || key.eq_ignore_ascii_case("SoftwareDCB") {
            block = Some((key.to_string(), false, Vec::new()));
            continue;
        }
        let rest = rest.trim();
        let v = if rest.is_empty() {
            Value::Flag
        } else {
            value(rest.strip_prefix('=').ok_or_else(|| err(n, "expected = after the property name"))?)
        };
        for (i, side) in ["TopMargin", "LeftMargin", "RightMargin", "BottomMargin"].iter().enumerate() {
            if key.eq_ignore_ascii_case(side) {
                if let Value::Number(m) = v {
                    margins[i] = m.max(0) as u32;
                }
            }
        }
        model.properties.push((key.to_string(), v));
    }
    if current.is_some() || block.is_some() || pending.is_some() {
        return Err(err(text.lines().count(), "the last model has no closing }"));
    }
    Ok(models)
}

/// The models of the shipped file, with the models of the user's file (if any) replacing those of the same names.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub models: Vec<Model>,
}

impl Catalog {
    pub fn read(shipped: &Path, user: Option<&Path>) -> Result<Catalog, String> {
        let text = std::fs::read_to_string(shipped).map_err(|e| format!("{}: {e}", shipped.display()))?;
        let own = match user.filter(|p| p.exists()) {
            Some(user) => Some((std::fs::read_to_string(user).map_err(|e| format!("{}: {e}", user.display()))?, user.display().to_string())),
            None => None,
        };
        Self::from_named_texts((&text, &shipped.display().to_string()), own.as_ref().map(|(t, n)| (t.as_str(), n.as_str())))
    }

    /// The catalog from the texts of the shipped file and the user's file (the web build has no folder to read).
    pub fn from_texts(shipped: &str, user: Option<&str>) -> Result<Catalog, String> {
        Self::from_named_texts((shipped, "embossers.txt"), user.map(|t| (t, "myembossers.txt")))
    }

    fn from_named_texts(shipped: (&str, &str), user: Option<(&str, &str)>) -> Result<Catalog, String> {
        let syntax = |name: &str, e: Error| format!("Syntax error in {name} at or near line {}: {}", e.line, e.message);
        let mut models = parse(shipped.0).map_err(|e| syntax(shipped.1, e))?;
        if let Some((text, name)) = user {
            let own = parse(text).map_err(|e| syntax(name, e))?;
            models.retain(|m| !own.iter().any(|o| o.names.iter().any(|n| m.has_name(&n.name))));
            models.extend(own);
        }
        Ok(Catalog { models })
    }

    /// The Index firmware versions a model label carries ("V5", "V3_early-V3"), if any: the part after the name in a
    /// label as `names` gives it.
    pub fn versions(&self, label: &str) -> Option<&str> {
        self.models.iter().flat_map(|m| m.names.iter()).find_map(|n| match &n.versions {
            Some(v) if format!("{} {}", n.name, v) == label => Some(v.as_str()),
            _ => None,
        })
    }

    /// The model with this name.
    pub fn model(&self, name: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.has_name(name))
    }

    /// Every model name, the sort-to-top models first, then the rest in alphabetical order (as the embosser setup
    /// lists them).
    pub fn names(&self) -> Vec<String> {
        let mut top = Vec::new();
        let mut rest = Vec::new();
        for m in &self.models {
            for n in &m.names {
                let label = match &n.versions {
                    Some(v) => format!("{} {}", n.name, v),
                    None => n.name.clone(),
                };
                if m.flag("SortToTop") { top.push(label) } else { rest.push(label) }
            }
        }
        rest.sort_by_key(|s| s.to_lowercase());
        top.extend(rest);
        top
    }

    /// A number of `model`, or of the Generic model, which every model starts from.
    pub fn number(&self, model: &Model, key: &str) -> Option<i64> {
        model.number(key).or_else(|| self.model("Generic").and_then(|g| g.number(key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Vec<Model> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../embossers/embossers.txt");
        parse(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn shipped_models_read_whole() {
        let models = shipped();
        assert_eq!(models.len(), 139);
        assert_eq!(models.iter().map(|m| m.names.len()).sum::<usize>(), 149);
        let forms: Vec<&Form> = models.iter().flat_map(|m| &m.forms).collect();
        // Counted in the file directly: lines starting "Form ", DoublePage forms, BrailleOnly pairs (some forms go on
        // over several lines).
        assert_eq!(forms.len(), 881);
        assert_eq!(forms.iter().filter(|f| f.double_page).count(), 69);
        assert_eq!(forms.iter().filter(|f| f.interline.is_some()).count(), 59);
    }

    #[test]
    fn model_properties() {
        let cat = Catalog { models: shipped() };
        let g = cat.model("Generic").unwrap();
        assert_eq!(g.number("baud"), Some(9600));
        assert_eq!(g.driver(), "Emb_GenericEmbosserDriver");
        assert!(g.flag("SortToTop"));
        let a4 = &g.forms[0];
        assert_eq!((a4.id.as_str(), a4.width, a4.length, a4.maximum, a4.recommended), ("A4", 83, 117, (35, 29), (30, 28)));
        let blaster = cat.model("APH PageBlaster").unwrap();
        assert_eq!(blaster.names[0].versions.as_deref(), Some("V5"));
        assert_eq!(blaster.text("InitSequence"), Some("BT0,TD0,PN0,DP,LS,TM,BI,CH,LP,MC"));
        assert!(blaster.flag("CanDoInterpoint"));
        // A model without its own baud rate starts from Generic's.
        assert_eq!(cat.number(blaster, "Baud"), Some(9600));
        let nbp = cat.models.iter().flat_map(|m| &m.forms).find(|f| f.id == "nbp").unwrap();
        assert_eq!(nbp.cell_offset, 1);
        let a3 = cat.models.iter().flat_map(|m| &m.forms).find(|f| f.id == "A3" && f.interline.is_some()).unwrap();
        assert_eq!((a3.maximum, a3.recommended, a3.interline), ((42, 41), (40, 40), Some(((42, 41), (40, 40)))));
        let names = cat.names();
        assert_eq!(names[0], "Generic");
        assert!(names.contains(&"Index Everest-D V5".to_string()));
    }

    #[test]
    fn serial_blocks_and_errors() {
        let text = "\"X\"\n{\n  Handshaking = Software\n  SoftwareDCB\n  {\n    fInX = FALSE\n  }\n  TopMargin = 50\n  Form \"a\" = 85 x 110 ; SinglePage ; 34 x 27 ; 32 x 25\n}\n";
        let m = &parse(text).unwrap()[0];
        assert_eq!(m.text("handshaking"), Some("Software"));
        assert_eq!(m.get("SoftwareDCB"), Some(&Value::Block(vec![("fInX".into(), "FALSE".into())])));
        assert_eq!(m.forms[0].margins, [50, 0, 0, 0]);
        assert_eq!(parse("\"X\"\n{\n  Form \"a\" = 85 x 110\n}\n").unwrap_err().line, 3);
        assert_eq!(parse("\"X\"\n{\n  Baud = 9600\n").unwrap_err().message, "the last model has no closing }");
    }
}
