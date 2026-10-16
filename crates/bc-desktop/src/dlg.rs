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

/// Room a push button needs beside its text: the padding and border each side.
const BUTTON_PADDING: i32 = 24;
/// The room a GTK text field takes around its text (its frame and inner padding, both sides).
const FIELD_PADDING: i32 = 20;

/// Loads the dialog resources; call once at start.
pub fn load_resources() -> Result<(), String> {
    let res = XmlResource::get();
    res.init_all_handlers();
    res.load_from_string(XRC)
}

/// The mnemonic letter of a label: the character after a single &.
fn mnemonic(label: &str) -> Option<char> {
    let mut chars = label.chars();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.next() {
                Some('&') => continue,
                Some(m) if m.is_ascii_alphanumeric() => return Some(m),
                _ => return None,
            }
        }
    }
    None
}

fn spec(id: i32) -> Option<&'static (i32, i32, i32, &'static [(i32, Ctl)])> {
    DIALOGS.iter().find(|d| d.0 == id)
}

pub struct Dlg {
    pub id: i32,
    pub dialog: Dialog,
    /// A page of a property sheet: its controls are in this panel, the sheet is `dialog`.
    pub page: Option<Panel>,
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
        let d = Dlg { id, dialog, page: None };
        d.fit_labels();
        d.dialog.centre();
        d
    }

    /// A property sheet: dialog `shell` (its OK and Cancel buttons and its text) holding `pages` (panels, by resource
    /// id, with their tab names) as tabs. Returns the sheet and its pages; the pages' controls are read and set
    /// through them, and the sheet is shown and destroyed as a dialog.
    pub fn sheet(parent: &dyn WxWidget, shell: i32, title: &str, pages: &[(i32, &str)]) -> (Dlg, Vec<Dlg>) {
        let dialog = XmlResource::get()
            .load_dialog(Some(parent), &format!("dlg{shell}"))
            .unwrap_or_else(|| panic!("dialog dlg{shell} is missing from dialogs.xrc"));
        dialog.set_label(title);
        let notebook = Notebook::builder(&dialog).build();
        let mut out = Vec::new();
        for (pid, tab) in pages {
            let panel = XmlResource::get()
                .load_panel(Some(&notebook), &format!("dlg{pid}"))
                .unwrap_or_else(|| panic!("page dlg{pid} is missing from dialogs.xrc"));
            if let Some((_, w, h, _)) = spec(*pid) {
                panel.set_client_size(dialog_units(&panel, *w, *h));
            }
            let d = Dlg { id: *pid, dialog, page: Some(panel) };
            d.fit_labels();
            panel.set_min_size(panel.get_client_size());
            notebook.add_page(&panel, tab, out.is_empty(), None);
            out.push(d);
        }
        // The tabs fill the top; OK and Cancel stand in a row below at the right, the sheet's text at the left.
        let margin = dialog_units(&dialog, 4, 4);
        let book = notebook.get_best_size();
        notebook.move_window(margin.width, margin.height);
        notebook.set_size(book);
        let sheet = Dlg { id: shell, dialog, page: None };
        if cfg!(not(windows)) {
            sheet.plain_ok_cancel();
        }
        let row = margin.height * 2 + book.height;
        let mut x = margin.width + book.width;
        let mut height = 0;
        // From the right: Cancel then OK (OK on the left of Cancel), as Windows places them; on macOS OK is at the
        // right with Cancel beside it, as Mac programs place them.
        let order = if cfg!(target_os = "macos") { [1, 2] } else { [2, 1] };
        for cid in order {
            if let Some(b) = sheet.win(cid) {
                let s = b.get_size();
                x -= s.width;
                b.move_window(x, row);
                x -= margin.width;
                height = height.max(s.height);
            }
        }
        if let Some(t) = sheet.win(6) {
            t.move_window(margin.width, row + margin.height);
        }
        dialog.set_client_size(Size::new(book.width + margin.width * 2, row + height + margin.height * 2));
        dialog.centre();
        (sheet, out)
    }

    /// The client size of the dialog, or of the page.
    fn client(&self) -> Size {
        match &self.page {
            Some(p) => p.get_client_size(),
            None => self.dialog.get_client_size(),
        }
    }

    fn set_client(&self, s: Size) {
        match &self.page {
            Some(p) => p.set_client_size(s),
            None => self.dialog.set_client_size(s),
        }
    }

    /// The dialogs are laid out in dialog units for the font of the system they came from. Where a one-line label
    /// needs more room in this system's font, the whole dialog is widened in proportion, positions and widths alike,
    /// so its layout stays as it is; text that runs over several lines is then wrapped to its width, as Windows
    /// static text wraps by itself.
    fn fit_labels(&self) {
        if cfg!(not(windows)) {
            self.fit_drop_downs();
            self.fit_short_fields();
            self.plain_ok_cancel();
            self.wire_label_mnemonics();
        }
        if cfg!(target_os = "macos") {
            self.mac_button_order();
        }
        let mut factor = 1.0f64;
        let mut several_lines = Vec::new();
        for (cid, kind) in self.controls() {
            let Some(w) = self.win(*cid) else { continue };
            let (best, given) = (w.get_best_size(), w.get_size());
            if given.width <= 0 {
                continue;
            }
            // The size a control is given becomes its minimum, so its best height is never less than that; the
            // number of lines it holds comes from the height of one line of its font.
            let line = w.get_text_extent("Ag").height.max(1);
            let needed = match kind {
                Ctl::Static if given.height >= 2 * line => {
                    several_lines.push(*cid);
                    continue;
                }
                Ctl::Check | Ctl::Radio | Ctl::Static | Ctl::Box => best.width,
                // A button's best size has a minimum width of its own; its text and the button's padding are what
                // must fit.
                Ctl::Button => {
                    let label = w.get_label().unwrap_or_default().replace('&', "");
                    w.get_text_extent(&label).width + BUTTON_PADDING
                }
                _ => continue,
            };
            factor = factor.max(needed as f64 / given.width as f64);
        }
        if factor > 1.0 {
            for (cid, _) in self.controls() {
                if let Some(w) = self.win(*cid) {
                    let (p, s) = (w.get_position(), w.get_size());
                    w.move_window((p.x as f64 * factor).round() as i32, p.y);
                    w.set_size(Size::new((s.width as f64 * factor).ceil() as i32, s.height));
                }
            }
            let c = self.client();
            self.set_client(Size::new((c.width as f64 * factor).ceil() as i32, c.height));
        }
        // Wrapped to the room it has in the dialog: on macOS a label takes the width of its whole text when it is
        // made, so its own width would leave it on one line, far past the dialog's edge.
        let room = self.client().width - self.margin().width;
        for cid in several_lines {
            if let Some(s) = self.win(cid).and_then(|w| w.as_widget::<StaticText>()) {
                let (p, size) = (s.get_position(), s.get_size());
                let width = size.width.min(room - p.x).max(1);
                s.set_size(Size::new(width, size.height));
                s.wrap(width);
                s.set_size(Size::new(width, size.height.max(s.get_best_size().height)));
            }
        }
        self.cover_controls();
    }

    /// macOS places Cancel to the left of OK, OK at the right (the reference's Mac version does the same). Where a
    /// dialog has OK left of Cancel in one row, the two change places: the right one keeps its right edge.
    fn mac_button_order(&self) {
        let (Some(ok), Some(cancel)) = (self.win(1), self.win(2)) else { return };
        let (op, os, cp, cs) = (ok.get_position(), ok.get_size(), cancel.get_position(), cancel.get_size());
        let same_row = (op.y - cp.y).abs() <= os.height / 2;
        if !same_row || op.x >= cp.x {
            return;
        }
        let right = cp.x + cs.width;
        ok.move_window(right - os.width, op.y);
        cancel.move_window(op.x, cp.y);
    }

    /// The dialog's own margin: 4 dialog units.
    fn margin(&self) -> Size {
        match &self.page {
            Some(p) => dialog_units(p, 4, 4),
            None => dialog_units(&self.dialog, 4, 4),
        }
    }

    /// After the controls have grown, the dialog (or page) grows until every control lies inside it, with the
    /// dialog's own margin beyond the furthest one.
    fn cover_controls(&self) {
        let margin = self.margin();
        let (mut right, mut bottom) = (0, 0);
        for (cid, _) in self.controls() {
            if let Some(w) = self.win(*cid) {
                let (p, s) = (w.get_position(), w.get_size());
                right = right.max(p.x + s.width);
                bottom = bottom.max(p.y + s.height);
            }
        }
        let c = self.client();
        let wanted = Size::new(c.width.max(right + margin.width), c.height.max(bottom + margin.height));
        if wanted.width > c.width || wanted.height > c.height {
            self.set_client(wanted);
        }
    }

    /// GTK gives the standard OK and Cancel buttons its own stock labels with letters of their own (Alt+O, Alt+C),
    /// which take those keys from the dialog's labels; the reference's OK and Cancel have none. They are given their
    /// own text again, which has no letter.
    fn plain_ok_cancel(&self) {
        for cid in [1, 2] {
            if let Some(b) = self.win(cid).and_then(|w| w.as_widget::<Button>()) {
                let text = b.get_label().replace(['&', '_'], "");
                b.set_label(&text);
            }
        }
    }

    /// In a Windows dialog, Alt with a label's underlined letter moves to the control after the label (the next in
    /// tab order). Elsewhere a label does not do that by itself (buttons and check boxes still take their own
    /// letters), so each control of the dialog passes Alt with a label's letter to the control after that label.
    fn wire_label_mnemonics(&self) {
        let controls = self.controls();
        let mut keys: Vec<(i32, Window)> = Vec::new();
        for (i, (cid, kind)) in controls.iter().enumerate() {
            if !matches!(kind, Ctl::Static | Ctl::Box) {
                continue;
            }
            let Some(letter) = self.win(*cid).and_then(|w| mnemonic(&w.get_label().unwrap_or_default())) else { continue };
            let target = controls[i + 1..]
                .iter()
                .find(|(_, k)| !matches!(k, Ctl::Static | Ctl::Box))
                .and_then(|(c, _)| self.win(*c));
            if let Some(t) = target {
                keys.push((letter.to_ascii_uppercase() as i32, t));
            }
        }
        if keys.is_empty() {
            return;
        }
        for (cid, kind) in controls {
            if matches!(kind, Ctl::Static | Ctl::Box) {
                continue;
            }
            let Some(w) = self.win(*cid) else { continue };
            let keys = keys.clone();
            let handler = move |e: WindowEventData| {
                if let WindowEventData::Keyboard(k) = &e {
                    let code = k.get_key_code().unwrap_or(0);
                    let to = keys.iter().find(|(c, _)| k.alt_down() && !k.control_down() && *c == code);
                    match to {
                        Some((_, t)) => t.set_focus(),
                        None => k.event.skip(true),
                    }
                }
            };
            match kind {
                Ctl::Text => w.as_widget::<TextCtrl>().map(|c| c.on_key_down(handler)),
                Ctl::List => w.as_widget::<ListBox>().map(|c| c.on_key_down(handler)),
                Ctl::Combo => w.as_widget::<Choice>().map(|c| c.on_key_down(handler)),
                Ctl::Check => w.as_widget::<CheckBox>().map(|c| c.on_key_down(handler)),
                Ctl::Radio => w.as_widget::<RadioButton>().map(|c| c.on_key_down(handler)),
                _ => w.as_widget::<Button>().map(|c| c.on_key_down(handler)),
            };
        }
    }

    /// A text field elsewhere than Windows has wider room around its text than a Windows edit box, so a short field
    /// (a number of cells or lines) would hide its own digits. Such a field grows to the right until it shows three
    /// digits, where no other control lies beside it in that room.
    fn fit_short_fields(&self) {
        for (cid, kind) in self.controls() {
            if *kind != Ctl::Text {
                continue;
            }
            let Some(w) = self.win(*cid) else { continue };
            let (p, s) = (w.get_position(), w.get_size());
            let wanted = w.get_text_extent("000").width + FIELD_PADDING;
            if s.width <= 0 || s.width >= wanted {
                continue;
            }
            let blocked = self.controls().iter().any(|(other, _)| {
                self.win(*other).filter(|_| other != cid).is_some_and(|o| {
                    let (op, os) = (o.get_position(), o.get_size());
                    let level = op.y < p.y + s.height && p.y < op.y + os.height;
                    level && op.x >= p.x + s.width && op.x < p.x + wanted
                })
            });
            if !blocked {
                w.set_min_size(Size::new(wanted, s.height));
                w.set_size(Size::new(wanted, s.height));
            }
        }
    }

    /// A drop-down list is given the height of its open list, as in Windows dialogs, where that height sizes the
    /// list and the closed box is one row (12 dialog units) high. Elsewhere the given height would size the closed
    /// box, so it takes its own height, which is more than that row: what lies below the row moves down by the
    /// difference, a group around it grows, and the dialog grows. A Windows list box shortens itself to whole rows,
    /// which keeps it clear of a control placed just below its edge; here such a list ends above that control.
    fn fit_drop_downs(&self) {
        let row = match &self.page {
            Some(p) => dialog_units(p, 0, 12).height,
            None => dialog_units(&self.dialog, 0, 12).height,
        };
        let mut drop_downs: Vec<(i32, i32)> = self
            .controls()
            .iter()
            .filter(|(_, kind)| *kind == Ctl::Combo)
            .filter_map(|(cid, _)| self.win(*cid).map(|w| (w.get_position().y, *cid)))
            .collect();
        drop_downs.sort();
        for (_, cid) in drop_downs {
            let Some(w) = self.win(cid) else { continue };
            let (p, s) = (w.get_position(), w.get_size());
            w.set_min_size(Size::new(-1, -1));
            let height = w.get_best_size().height;
            w.set_size(Size::new(s.width, height));
            let extra = height - row;
            for (other, kind) in self.controls() {
                let Some(o) = self.win(*other).filter(|_| *other != cid) else { continue };
                let (op, os) = (o.get_position(), o.get_size());
                let beside = op.x < p.x + s.width && p.x < op.x + os.width;
                if *kind == Ctl::List && beside && op.y < p.y {
                    // Ended above the drop-down below.
                    continue;
                } else if extra > 0 && op.y >= p.y + row / 2 {
                    o.move_window(op.x, op.y + extra);
                } else if extra > 0 && op.y < p.y && op.y + os.height > p.y + row / 2 {
                    o.set_min_size(Size::new(-1, -1));
                    o.set_size(Size::new(os.width, os.height + extra));
                }
            }
            if extra > 0 {
                let c = self.client();
                self.set_client(Size::new(c.width, c.height + extra));
            }
        }
        for (cid, kind) in self.controls() {
            let Some(l) = self.win(*cid).filter(|_| *kind == Ctl::List) else { continue };
            let (lp, ls) = (l.get_position(), l.get_size());
            let below = self
                .controls()
                .iter()
                .filter(|(other, _)| other != cid)
                .filter_map(|(other, _)| self.win(*other))
                .map(|o| (o.get_position(), o.get_size()))
                .filter(|(op, os)| op.x < lp.x + ls.width && lp.x < op.x + os.width)
                .map(|(op, _)| op.y)
                .filter(|y| *y > lp.y && *y < lp.y + ls.height)
                .min();
            if let Some(y) = below {
                l.set_min_size(Size::new(-1, -1));
                l.set_size(Size::new(ls.width, y - lp.y - 2));
            }
        }
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
        match &self.page {
            Some(p) => p.find_window_by_name(&name),
            None => self.dialog.find_window_by_name(&name),
        }
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

    /// Fills a list or combo box with `items` and chooses `items[selected]`. Many reference boxes sort their items,
    /// so the choice is made by its text, not by its place in `items`.
    pub fn set_items(&self, cid: i32, items: &[&str], selected: Option<u32>) {
        let chosen = selected.and_then(|s| items.get(s as usize));
        if let Some(w) = self.win(cid) {
            if let Some(l) = w.as_widget::<ListBox>() {
                l.clear();
                for i in items {
                    l.append(i);
                }
                if let Some(t) = chosen {
                    l.set_string_selection(t, true);
                }
            } else if let Some(c) = w.as_widget::<Choice>() {
                c.clear();
                for i in items {
                    c.append(i);
                }
                if let Some(t) = chosen {
                    if let Some(at) = (0..c.get_count()).find(|n| c.get_string(*n).as_deref() == Some(*t)) {
                        c.set_selection(at);
                    }
                }
            }
        }
    }

    /// The place in `items` of the chosen item (found by its text, as the box may sort).
    pub fn chosen_in(&self, cid: i32, items: &[&str]) -> Option<usize> {
        let t = self.selected_text(cid)?;
        items.iter().position(|i| *i == t)
    }

    pub fn selection(&self, cid: i32) -> Option<u32> {
        let w = self.win(cid)?;
        if let Some(l) = w.as_widget::<ListBox>() {
            l.get_selection()
        } else {
            w.as_widget::<Choice>()?.get_selection()
        }
    }

    pub fn selected_text(&self, cid: i32) -> Option<String> {
        let w = self.win(cid)?;
        if let Some(l) = w.as_widget::<ListBox>() {
            l.get_string_selection()
        } else {
            w.as_widget::<Choice>()?.get_string_selection()
        }
    }

    pub fn select(&self, cid: i32, index: u32) {
        if let Some(w) = self.win(cid) {
            if let Some(l) = w.as_widget::<ListBox>() {
                l.set_selection(index, true);
            } else if let Some(c) = w.as_widget::<Choice>() {
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

    /// A radio button runs `handler` when it is chosen.
    pub fn on_radio(&self, cid: i32, handler: impl FnMut() + 'static) {
        let mut handler = handler;
        if let Some(r) = self.win(cid).and_then(|w| w.as_widget::<RadioButton>()) {
            r.on_selected(move |_| handler());
        }
    }

    /// Hides a control.
    pub fn hide(&self, cid: i32) {
        self.set_shown(cid, false);
    }

    /// Shows or hides a control.
    pub fn set_shown(&self, cid: i32, on: bool) {
        if let Some(w) = self.win(cid) {
            w.show(on);
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
            // GTK leaves focus where it puts it when the dialog comes up (on a button), whatever was asked before; so
            // the first field is given focus again once, when the dialog is first active, as in a Windows dialog.
            let once = std::rc::Rc::new(std::cell::Cell::new(false));
            self.dialog.on_activate(move |e| {
                if let WindowEventData::Activate(a) = &e {
                    if a.is_active() && !once.replace(true) {
                        w.set_focus();
                    }
                    a.event.skip(true);
                }
            });
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

/// Layout check (BC_LAYOUT_CHECK=<file>): every dialog and the embosser setup sheet are laid out as on this system
/// and each pair of shown controls that overlap, and each control that reaches past its dialog, is written to the
/// file, with a count per dialog. Group boxes are left out of the pairs: they hold other controls by design.
pub fn layout_check(path: &str) {
    let mut out = String::new();
    let frame = crate::app::frame();
    for (id, _, _, _) in DIALOGS.iter() {
        if (10000..=10004).contains(id) {
            continue;
        }
        let d = Dlg::open(&frame, *id, None);
        report_layout(&d, &format!("dialog {id}"), &mut out);
        d.destroy();
    }
    let models = crate::embosser::layout_models();
    if models.is_empty() {
        out.push_str("embosser setup sheet: no embosser models\n");
    }
    for model in models {
        if let Some((sheet, pages)) = crate::embosser::layout_sheet(&model) {
            for p in &pages {
                report_layout(p, &format!("sheet page {} ({model})", p.id), &mut out);
            }
            sheet.destroy();
        }
    }
    if let Err(e) = std::fs::write(path, out) {
        eprintln!("BrailleCraft: layout check not written to {path}: {e}");
    }
}

/// Layout check, the picture (BC_LAYOUT_SHOW=<page id>): the setup sheet with that one page, shown and left open.
pub fn layout_show(page: i32) {
    if !(10000..=10004).contains(&page) {
        // Any other dialog, shown as it opens.
        let d = Dlg::open(&crate::app::frame(), page, None);
        d.dialog.show(true);
        d.dialog.raise();
        return;
    }
    let name = match page {
        10001 => "General",
        10002 => "Device Settings",
        10003 => "TranSend SE",
        _ => "Advanced",
    };
    let (sheet, pages) = Dlg::sheet(&crate::app::frame(), 10000, "Embosser Setup - layout check", &[(page, name)]);
    if page == 10002 {
        crate::embosser::layout_device_page(&pages[0]);
    }
    sheet.dialog.show(true);
    sheet.dialog.raise();
}

fn report_layout(d: &Dlg, name: &str, out: &mut String) {
    use std::fmt::Write;
    let client = d.client();
    let mut rects = Vec::new();
    for (cid, kind) in d.controls() {
        let Some(w) = d.win(*cid) else { continue };
        let (p, s) = (w.get_position(), w.get_size());
        if !w.is_shown() || s.width <= 0 || s.height <= 0 {
            continue;
        }
        rects.push((*cid, *kind, p.x, p.y, s.width, s.height));
    }
    let mut found = 0;
    for (i, a) in rects.iter().enumerate() {
        let (acid, akind, ax, ay, aw, ah) = *a;
        if ax + aw > client.width + 1 || ay + ah > client.height + 1 {
            found += 1;
            let _ = writeln!(out, "{name}: {acid} {akind:?} {ax},{ay} {aw}x{ah} reaches past the {}x{} dialog", client.width, client.height);
        }
        if akind == Ctl::Box {
            continue;
        }
        for &(bcid, bkind, bx, by, bw, bh) in &rects[i + 1..] {
            if bkind == Ctl::Box {
                continue;
            }
            if ax + 1 < bx + bw && bx + 1 < ax + aw && ay + 1 < by + bh && by + 1 < ay + ah {
                found += 1;
                let _ = writeln!(out, "{name}: {acid} {akind:?} {ax},{ay} {aw}x{ah} overlaps {bcid} {bkind:?} {bx},{by} {bw}x{bh}");
            }
        }
    }
    let _ = writeln!(out, "{name}: {found} found");
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
