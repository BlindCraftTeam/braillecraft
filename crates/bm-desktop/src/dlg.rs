//! The dialogs, as native dialogs. ui-native/dialogs.xrc (tools/dialogs/dialogs_to_xrc.py) holds every dialog with
//! its controls at their positions in dialog units; this module opens one, sizes it to its client size, and reads
//! and sets its controls by control id. Values of settings dialogs are kept between openings.

use std::collections::HashMap;

use wxdragon::prelude::*;
use wxdragon::window::Window;
use wxdragon::xrc::XmlResource;

use crate::dialog_table::DIALOGS;

/// The kind of a dialog control, as the generator records it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ctl {
    Button,
    Box,
    Check,
    Radio,
    Static,
    Text,
    List,
    Combo,
}

/// A remembered control value.
#[derive(Clone, Debug)]
pub enum Value {
    Text(String),
    Flag(bool),
    Choice(Option<u32>),
}

const XRC: &str = include_str!("../../../ui-native/dialogs.xrc");

/// Loads the dialog resources; call once at start.
pub fn load_resources() -> Result<(), String> {
    let res = XmlResource::get();
    res.init_all_handlers();
    res.load_from_string(XRC)
}

fn spec(id: i32) -> Option<&'static (i32, i32, i32, &'static [(i32, Ctl)])> {
    DIALOGS.iter().find(|d| d.0 == id)
}

pub struct Dlg {
    pub id: i32,
    pub dialog: Dialog,
}

/// Dialog units to pixels as Windows converts them: horizontal units are a quarter of the average character
/// width of the dialog font, vertical units an eighth of its height.
fn dialog_units(win: &dyn WxWidget, w: i32, h: i32) -> Size {
    let sample = win.get_text_extent("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz");
    let avg = (sample.width / 26 + 1) / 2;
    let height = win.get_char_height();
    Size::new(w * avg / 4, h * height / 8)
}

impl Dlg {
    /// Opens dialog `id` over `parent`, with its title replaced when `title` is given.
    pub fn open(parent: &dyn WxWidget, id: i32, title: Option<&str>) -> Dlg {
        let dialog = XmlResource::get()
            .load_dialog(Some(parent), &format!("dlg{id}"))
            .unwrap_or_else(|| panic!("dialog dlg{id} is missing from dialogs.xrc"));
        if let Some((_, w, h, _)) = spec(id) {
            dialog.set_client_size(dialog_units(&dialog, *w, *h));
        }
        if let Some(t) = title {
            dialog.set_label(t);
        }
        dialog.centre();
        Dlg { id, dialog }
    }

    fn controls(&self) -> &'static [(i32, Ctl)] {
        spec(self.id).map_or(&[], |s| s.3)
    }

    pub fn win(&self, cid: i32) -> Option<Window> {
        let name = match cid {
            1 => "wxID_OK".to_string(),
            2 => "wxID_CANCEL".to_string(),
            n => format!("c{n}"),
        };
        self.dialog.find_window_by_name(&name)
    }

    pub fn text(&self, cid: i32) -> String {
        self.win(cid).and_then(|w| w.as_widget::<TextCtrl>()).map(|t| t.get_value()).unwrap_or_default()
    }

    pub fn set_text(&self, cid: i32, value: &str) {
        if let Some(w) = self.win(cid) {
            if let Some(t) = w.as_widget::<TextCtrl>() {
                t.set_value(value);
            } else if let Some(s) = w.as_widget::<StaticText>() {
                s.set_label(value);
            } else {
                w.set_label(value);
            }
        }
    }

    /// A number typed in a text field; None when it is not a whole number.
    pub fn number(&self, cid: i32) -> Option<i64> {
        self.text(cid).trim().parse().ok()
    }

    pub fn checked(&self, cid: i32) -> bool {
        match self.win(cid) {
            Some(w) => {
                if let Some(c) = w.as_widget::<CheckBox>() {
                    c.is_checked()
                } else if let Some(r) = w.as_widget::<RadioButton>() {
                    r.get_value()
                } else {
                    false
                }
            }
            None => false,
        }
    }

    pub fn set_checked(&self, cid: i32, on: bool) {
        if let Some(w) = self.win(cid) {
            if let Some(c) = w.as_widget::<CheckBox>() {
                c.set_value(on);
            } else if let Some(r) = w.as_widget::<RadioButton>() {
                r.set_value(on);
            }
        }
    }

    /// The position of the chosen button among `ids` (a radio group), or None.
    pub fn radio(&self, ids: &[i32]) -> Option<usize> {
        ids.iter().position(|id| self.checked(*id))
    }

    pub fn set_radio(&self, ids: &[i32], index: usize) {
        if let Some(id) = ids.get(index) {
            self.set_checked(*id, true);
        }
    }

    pub fn set_items(&self, cid: i32, items: &[&str], selected: Option<u32>) {
        if let Some(w) = self.win(cid) {
            if let Some(l) = w.as_widget::<ListBox>() {
                l.clear();
                for i in items {
                    l.append(i);
                }
                if let Some(s) = selected {
                    l.set_selection(s, true);
                }
            } else if let Some(c) = w.as_widget::<ComboBox>() {
                c.clear();
                for i in items {
                    c.append(i);
                }
                if let Some(s) = selected {
                    c.set_selection(s);
                }
            }
        }
    }

    pub fn selection(&self, cid: i32) -> Option<u32> {
        let w = self.win(cid)?;
        if let Some(l) = w.as_widget::<ListBox>() {
            l.get_selection()
        } else {
            w.as_widget::<ComboBox>()?.get_selection()
        }
    }

    pub fn selected_text(&self, cid: i32) -> Option<String> {
        let w = self.win(cid)?;
        if let Some(l) = w.as_widget::<ListBox>() {
            l.get_string_selection()
        } else {
            w.as_widget::<ComboBox>()?.get_string_selection()
        }
    }

    pub fn select(&self, cid: i32, index: u32) {
        if let Some(w) = self.win(cid) {
            if let Some(l) = w.as_widget::<ListBox>() {
                l.set_selection(index, true);
            } else if let Some(c) = w.as_widget::<ComboBox>() {
                c.set_selection(index);
            }
        }
    }

    pub fn enable(&self, cid: i32, on: bool) {
        if let Some(w) = self.win(cid) {
            w.enable(on);
        }
    }

    /// A push button inside the dialog (other than OK and Cancel) runs `handler` when pressed.
    pub fn on_button(&self, cid: i32, handler: impl FnMut() + 'static) {
        let mut handler = handler;
        if let Some(b) = self.win(cid).and_then(|w| w.as_widget::<Button>()) {
            b.on_click(move |_| handler());
        }
    }

    /// A push button that closes the dialog, returning its control id from `show`.
    pub fn closes_with(&self, cid: i32) {
        let dialog = self.dialog;
        self.on_button(cid, move || dialog.end_modal(cid));
    }

    /// Shows the dialog and waits for it to close. Focus starts in the first field (a text field, list, choice,
    /// check box or radio button), so typing goes straight into it. Returns the closing control id:
    /// 1 for OK, 2 for Cancel (also Escape and the close box), or the id given to `closes_with`.
    pub fn show(&self) -> i32 {
        let first = self.controls().iter().find(|(cid, kind)| {
            matches!(kind, Ctl::Text | Ctl::List | Ctl::Combo | Ctl::Check | Ctl::Radio)
                && self.win(*cid).is_some_and(|w| w.is_shown() && w.is_enabled())
                && (*kind != Ctl::Radio || self.checked(*cid) || !self.group_has_choice(*cid))
        });
        if let Some(w) = first.and_then(|(cid, _)| self.win(*cid)) {
            w.set_focus();
        }
        let code = self.dialog.show_modal();
        if code == wxdragon::id::ID_OK {
            1
        } else if code == wxdragon::id::ID_CANCEL {
            2
        } else {
            code
        }
    }

    fn group_has_choice(&self, cid: i32) -> bool {
        // A radio button takes first focus only if it is the chosen one of its group, as in Windows dialogs.
        let ctls = self.controls();
        let Some(pos) = ctls.iter().position(|(c, _)| *c == cid) else { return false };
        let mut start = pos;
        while start > 0 && ctls[start - 1].1 == Ctl::Radio {
            start -= 1;
        }
        let mut end = pos;
        while end + 1 < ctls.len() && ctls[end + 1].1 == Ctl::Radio {
            end += 1;
        }
        ctls[start..=end].iter().any(|(c, _)| self.checked(*c))
    }

    /// Every field's value, for remembering a settings dialog.
    pub fn values(&self) -> Vec<(i32, Value)> {
        let mut out = Vec::new();
        for (cid, kind) in self.controls() {
            let v = match kind {
                Ctl::Text => Value::Text(self.text(*cid)),
                Ctl::Check | Ctl::Radio => Value::Flag(self.checked(*cid)),
                Ctl::List | Ctl::Combo => Value::Choice(self.selection(*cid)),
                _ => continue,
            };
            out.push((*cid, v));
        }
        out
    }

    pub fn restore(&self, values: &[(i32, Value)]) {
        for (cid, v) in values {
            match v {
                Value::Text(t) => self.set_text(*cid, t),
                Value::Flag(f) => {
                    if *f {
                        self.set_checked(*cid, true)
                    } else if self.controls().iter().any(|(c, k)| c == cid && *k == Ctl::Check) {
                        self.set_checked(*cid, false)
                    }
                }
                Value::Choice(Some(i)) => self.select(*cid, *i),
                Value::Choice(None) => {}
            }
        }
    }

    pub fn destroy(self) {
        self.dialog.destroy();
    }
}

/// Remembered values of settings dialogs, by dialog id (and a key, for dialogs opened in two roles).
#[derive(Default)]
pub struct Memory(HashMap<(i32, &'static str), Vec<(i32, Value)>>);

impl Memory {
    pub fn get(&self, id: i32, key: &'static str) -> Option<&Vec<(i32, Value)>> {
        self.0.get(&(id, key))
    }

    pub fn put(&mut self, id: i32, key: &'static str, values: Vec<(i32, Value)>) {
        self.0.insert((id, key), values);
    }
}
