//! The braille encodings of Global, Internationalization: for each of the 18 national encodings, the characters
//! that stand for each of the 64 cells (ui/fonts/encodings.json, written by tools/fonts/make_braille_fonts.py; the
//! braille fonts are built from the same table). A braille document holds its text in these characters, the
//! way braille is shown on screen; the translator works in cells (Unicode braille, U+2800 plus the dot pattern).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Encoding {
    pub id: String,
    pub name: String,
    pub order: u32,
    /// Per cell (dot pattern 0 to 63): the characters for it, the first being the one written.
    cells: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Entry {
    Chars(String),
    Mixed(Vec<String>),
}

impl Entry {
    fn chars(&self) -> Vec<char> {
        match self {
            Entry::Chars(s) => s.chars().collect(),
            Entry::Mixed(items) => items
                .iter()
                .filter_map(|i| match i.strip_prefix("U+") {
                    Some(hex) if !hex.is_empty() => u32::from_str_radix(hex, 16).ok().and_then(char::from_u32),
                    _ => i.chars().next(),
                })
                .collect(),
        }
    }
}

pub const DEFAULT: &str = "north-american";

pub fn all() -> &'static [Encoding] {
    static LIST: OnceLock<Vec<Encoding>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut list: Vec<Encoding> = serde_json::from_str(include_str!("../../../ui/fonts/encodings.json"))
            .expect("ui/fonts/encodings.json");
        list.sort_by_key(|e| e.order);
        list
    })
}

pub fn find(id: &str) -> &'static Encoding {
    all().iter().find(|e| e.id == id).or_else(|| all().iter().find(|e| e.id == DEFAULT)).expect("encodings")
}

impl Encoding {
    /// The character written for a cell.
    pub fn cell_char(&self, pattern: usize) -> char {
        self.cells.get(pattern).and_then(|e| e.chars().first().copied()).unwrap_or_else(|| {
            char::from_u32(0x2800 + pattern as u32).unwrap_or(' ')
        })
    }

    /// Unicode braille cells to this encoding's characters; anything else (line breaks) passes through.
    pub fn cells_to_text(&self, cells: &str) -> String {
        cells
            .chars()
            .map(|c| {
                let cp = c as u32;
                if (0x2800..=0x283f).contains(&cp) {
                    self.cell_char((cp - 0x2800) as usize)
                } else {
                    c
                }
            })
            .collect()
    }

    fn reverse(&self) -> HashMap<char, usize> {
        let mut map = HashMap::new();
        for (p, e) in self.cells.iter().enumerate() {
            for c in e.chars() {
                map.entry(c).or_insert(p);
            }
        }
        map
    }

    /// The cell (dot pattern) a character of this encoding stands for, upper and lower case alike; None for a
    /// character the encoding does not have.
    pub fn cell_of(&self, c: char) -> Option<usize> {
        let map = self.reverse();
        map.get(&c)
            .or_else(|| c.to_lowercase().next().and_then(|l| map.get(&l)))
            .or_else(|| c.to_uppercase().next().and_then(|u| map.get(&u)))
            .copied()
    }

    /// This encoding's characters back to cells, for back-translation and for changing encodings. Upper and lower
    /// case stand for the same cell; a character with no cell becomes a blank cell.
    pub fn text_to_cells(&self, text: &str) -> String {
        let map = self.reverse();
        text.chars()
            .map(|c| {
                if c == '\n' {
                    return c;
                }
                let p = map
                    .get(&c)
                    .or_else(|| c.to_lowercase().next().and_then(|l| map.get(&l)))
                    .or_else(|| c.to_uppercase().next().and_then(|u| map.get(&u)));
                p.map_or('\u{2800}', |p| char::from_u32(0x2800 + *p as u32).unwrap_or('\u{2800}'))
            })
            .collect()
    }

    /// The family names of this encoding's braille fonts (ui/fonts/BrailleCraft-<kind>-<id>.ttf).
    pub fn font_family(&self, kind: &str) -> String {
        format!("BrailleCraft {kind} {}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn north_american_round_trip() {
        let e = find("north-american");
        assert_eq!(e.cells_to_text("⠮⠀⠟⠅"), "! qk");
        assert_eq!(e.text_to_cells("! QK"), "⠮⠀⠟⠅");
    }

    #[test]
    fn eighteen_encodings_with_64_cells() {
        assert_eq!(all().len(), 18);
        for e in all() {
            assert_eq!(e.cells.len(), 64, "{}", e.id);
        }
    }
}
