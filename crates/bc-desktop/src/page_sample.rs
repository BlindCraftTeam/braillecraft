//! The sample page of Printer Setup (dialog 1071). The reference opens the Windows Page Setup dialog with this
//! template, and the system draws a sample page in controls 1080 (the paper), 1081 and 1082 (its shadows to the right
//! and below): a rectangle in the paper's proportions for the chosen orientation, the margins as a dotted rectangle,
//! and small text inside them (spec 24, 2b; the Microsoft PagePaintHook documentation). Here the three controls are
//! replaced by one panel that draws the same, on every system, following the open Page Setup code of Wine
//! (pagesetup_change_preview and its page paint procedure): the paper fills the height of 1080 in portrait and its
//! width in landscape, centred; each shadow is 4 pixels; the margins are a 1-pixel dashed line in the shadow colour;
//! the text sits 2 pixels inside them. The shadows' colour and the sample's words are this program's choice: the
//! system fills SS_GRAYRECT with its screen background colour, and its sample words are its own.

use std::cell::RefCell;
use std::rc::Rc;

use wxdragon::prelude::*;

use crate::dlg::Dlg;
use crate::print::PrinterSetup;

const SHADOW: i32 = 4;
const SAMPLE_TEXT: &str = "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. \
How vexingly quick daft zebras jump. The five boxing wizards jump quickly. Sphinx of black quartz, judge my vow.";

#[derive(Clone)]
pub struct PageSample {
    panel: Panel,
    state: Rc<RefCell<PrinterSetup>>,
}

impl PageSample {
    /// Replaces controls 1080 to 1082 of `d` with the drawn sample page, showing `setup`.
    pub fn install(d: &Dlg, setup: &PrinterSetup) -> Option<PageSample> {
        let paper = d.win(1080)?;
        let (p, s) = (paper.get_position(), paper.get_size());
        for cid in [1080, 1081, 1082] {
            d.hide(cid);
        }
        let panel = Panel::builder(d.container())
            .with_pos(Point::new(p.x, p.y))
            .with_size(Size::new(s.width + SHADOW, s.height + SHADOW))
            .build();
        panel.set_background_style(BackgroundStyle::Paint);
        // Around the paper the dialog shows through, as around the reference's three controls.
        panel.set_background_color(d.container().get_background_color());
        let state = Rc::new(RefCell::new(setup.clone()));
        let (pn, st) = (panel.clone(), state.clone());
        panel.on_paint(move |_| paint(&pn, &st.borrow(), s));
        Some(PageSample { panel, state })
    }

    /// Shows `setup` (paper, orientation, margins).
    pub fn set(&self, setup: &PrinterSetup) {
        *self.state.borrow_mut() = setup.clone();
        self.panel.refresh(true, None);
    }
}

fn paint(panel: &Panel, s: &PrinterSetup, area: Size) {
    let dc = PaintDC::new(panel);
    dc.set_background(panel.get_background_color());
    dc.clear();
    // The paper in thousandths of an inch, as the margins are, turned for landscape.
    let thou = |tenths_mm: i32| (tenths_mm as i64 * 1000 / 254) as i32;
    let (mut pw, mut ph) = (thou(s.paper.0).max(1), thou(s.paper.1).max(1));
    if s.landscape {
        std::mem::swap(&mut pw, &mut ph);
    }
    let (width, height) = if s.landscape {
        (area.width, (ph as i64 * area.width as i64 / pw as i64) as i32)
    } else {
        ((pw as i64 * area.height as i64 / ph as i64) as i32, area.height)
    };
    let (x, y) = ((area.width - width) / 2, (area.height - height) / 2);
    let shadow = SystemSettings::get_colour(SystemColour::BtnShadow);
    let dark = SystemSettings::get_colour(SystemColour::ThreeDDkShadow);
    let light = SystemSettings::get_colour(SystemColour::BtnHighlight);
    // The shadows, right and below.
    dc.set_pen(shadow, 1, PenStyle::Solid);
    dc.set_brush(shadow, BrushStyle::Solid);
    dc.draw_rectangle(x + width, y + SHADOW, SHADOW, height);
    dc.draw_rectangle(x + SHADOW, y + height, width, SHADOW);
    // The paper: filled, its left and top edges in the shadow colour, its right and bottom ones darker.
    dc.set_pen(light, 1, PenStyle::Solid);
    dc.set_brush(light, BrushStyle::Solid);
    dc.draw_rectangle(x, y, width, height);
    dc.set_pen(shadow, 1, PenStyle::Solid);
    dc.draw_line(x, y, x + width - 1, y);
    dc.draw_line(x, y, x, y + height - 1);
    dc.set_pen(dark, 1, PenStyle::Solid);
    dc.draw_line(x + width - 1, y, x + width - 1, y + height);
    dc.draw_line(x, y + height - 1, x + width, y + height - 1);
    // The margins (left, top, right, bottom) in proportion to the paper.
    let [ml, mt, mr, mb] = s.margins;
    let sx = |m: i32| (m.max(0) as i64 * width as i64 / pw as i64) as i32;
    let sy = |m: i32| (m.max(0) as i64 * height as i64 / ph as i64) as i32;
    let (left, top) = (x + sx(ml), y + sy(mt));
    let (right, bottom) = (x + width - sx(mr), y + height - sy(mb));
    if right - left < 4 || bottom - top < 4 {
        return;
    }
    dc.set_pen(shadow, 1, PenStyle::ShortDash);
    dc.set_brush(light, BrushStyle::Transparent);
    dc.draw_rectangle(left, top, right - left, bottom - top);
    // The sample text, wrapped inside the margins.
    if let Some(font) = Font::builder().with_point_size(4).build() {
        dc.set_font(&font);
    }
    dc.set_text_foreground(shadow);
    dc.set_background_mode(BackgroundMode::Transparent);
    let (tl, tt, tr, tb) = (left + 2, top + 2, right - 2, bottom - 2);
    let line = dc.get_text_extent("Ag").1.max(1);
    let (mut cx, mut cy) = (tl, tt);
    for word in SAMPLE_TEXT.split(' ').cycle() {
        let w = dc.get_text_extent(word).0;
        let space = dc.get_text_extent(" ").0;
        if cx > tl && cx + w > tr {
            cx = tl;
            cy += line;
        }
        if cy + line > tb {
            break;
        }
        if w <= tr - tl {
            dc.draw_text(word, cx, cy);
        }
        cx += w + space;
    }
}
