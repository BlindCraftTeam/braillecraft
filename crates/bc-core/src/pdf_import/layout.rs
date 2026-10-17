//! Layout analysis of a page's characters into lines, text boxes and groups of boxes: a port of the layout analysis
//! of pdfminer.six 20231228 (layout.py and the Plane of utils.py; MIT licence, listed in NOTICE), the version the
//! reference program's PDF importer runs. The rules, thresholds and orders are kept as they are there, so a PDF is
//! cut into the same boxes; names follow pdfminer's where that helps to compare the two.
//!
//! One difference that cannot be kept: pdfminer breaks ties between equal box distances by Python object identity
//! (memory addresses); here the tie goes to the element made first.

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::Hash;

/// pdfminer's INF, (1 << 31) - 1.
pub const INF: f64 = 2147483647.0;

/// A rectangle (x0, y0) to (x1, y1), y upwards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    /// The start of an expandable container: (+INF, +INF, -INF, -INF).
    pub const EMPTY: Rect = Rect::new(INF, INF, -INF, -INF);

    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }

    pub fn union(&self, o: &Rect) -> Rect {
        Rect::new(self.x0.min(o.x0), self.y0.min(o.y0), self.x1.max(o.x1), self.y1.max(o.y1))
    }

    pub fn is_empty(&self) -> bool {
        self.width() <= 0.0 || self.height() <= 0.0
    }

    pub fn is_hoverlap(&self, o: &Rect) -> bool {
        o.x0 <= self.x1 && self.x0 <= o.x1
    }

    pub fn hdistance(&self, o: &Rect) -> f64 {
        if self.is_hoverlap(o) {
            0.0
        } else {
            (self.x0 - o.x1).abs().min((self.x1 - o.x0).abs())
        }
    }

    pub fn hoverlap(&self, o: &Rect) -> f64 {
        if self.is_hoverlap(o) {
            (self.x0 - o.x1).abs().min((self.x1 - o.x0).abs())
        } else {
            0.0
        }
    }

    pub fn is_voverlap(&self, o: &Rect) -> bool {
        o.y0 <= self.y1 && self.y0 <= o.y1
    }

    pub fn vdistance(&self, o: &Rect) -> f64 {
        if self.is_voverlap(o) {
            0.0
        } else {
            (self.y0 - o.y1).abs().min((self.y1 - o.y0).abs())
        }
    }

    pub fn voverlap(&self, o: &Rect) -> f64 {
        if self.is_voverlap(o) {
            (self.y0 - o.y1).abs().min((self.y1 - o.y0).abs())
        } else {
            0.0
        }
    }
}

/// Layout parameters (pdfminer LAParams); the defaults are pdfminer's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaParams {
    pub line_overlap: f64,
    pub char_margin: f64,
    pub line_margin: f64,
    pub word_margin: f64,
    /// None turns the grouping of boxes off (and with it, in the importer, all text of the page).
    pub boxes_flow: Option<f64>,
    pub detect_vertical: bool,
    pub all_texts: bool,
}

impl Default for LaParams {
    fn default() -> Self {
        LaParams {
            line_overlap: 0.5,
            char_margin: 2.0,
            line_margin: 0.5,
            word_margin: 0.1,
            boxes_flow: Some(0.5),
            detect_vertical: false,
            all_texts: false,
        }
    }
}

/// One character as drawn: its box and its text (one or more characters, as the font maps the glyph).
#[derive(Debug, Clone, PartialEq)]
pub struct Char {
    pub rect: Rect,
    pub text: String,
}

/// What a line holds: a character of the page, or text the analysis adds (a space between words, the line end).
#[derive(Debug, Clone, PartialEq)]
pub enum LineItem {
    Char(usize),
    Anno(&'static str),
}

/// A text line (LTTextLineHorizontal or LTTextLineVertical).
#[derive(Debug, Clone)]
pub struct Line {
    pub vertical: bool,
    pub rect: Rect,
    pub items: Vec<LineItem>,
    word_margin: f64,
    // The last character's right edge (horizontal) or bottom edge (vertical).
    last: f64,
}

impl Line {
    fn new(vertical: bool, word_margin: f64) -> Line {
        Line { vertical, rect: Rect::EMPTY, items: Vec::new(), word_margin, last: if vertical { -INF } else { INF } }
    }

    fn add(&mut self, i: usize, chars: &[Char]) {
        let r = chars[i].rect;
        if self.word_margin != 0.0 {
            let margin = self.word_margin * r.width().max(r.height());
            if self.vertical {
                if r.y1 + margin < self.last {
                    self.items.push(LineItem::Anno(" "));
                }
                self.last = r.y0;
            } else {
                if self.last < r.x0 - margin {
                    self.items.push(LineItem::Anno(" "));
                }
                self.last = r.x1;
            }
        }
        self.items.push(LineItem::Char(i));
        self.rect = self.rect.union(&r);
    }

    pub fn text(&self, chars: &[Char]) -> String {
        self.items
            .iter()
            .map(|it| match it {
                LineItem::Char(i) => chars[*i].text.as_str(),
                LineItem::Anno(s) => s,
            })
            .collect()
    }

    /// Empty: no area, or only white space (Python's isspace: at least one character, all white space).
    fn is_empty(&self, chars: &[Char]) -> bool {
        let t = self.text(chars);
        self.rect.is_empty() || (!t.is_empty() && t.chars().all(char::is_whitespace))
    }
}

/// A text box: lines, in the order the analysis sorts them.
#[derive(Debug, Clone)]
pub struct TextBox {
    pub vertical: bool,
    pub rect: Rect,
    pub lines: Vec<usize>,
}

/// A member of a group: a box or a smaller group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Node {
    Box(usize),
    Group(usize),
}

/// A group of boxes and groups (LTTextGroupLRTB, or LTTextGroupTBRL when vertical text is in it).
#[derive(Debug, Clone)]
pub struct Group {
    pub tbrl: bool,
    pub rect: Rect,
    pub members: Vec<Node>,
}

/// A page after layout analysis.
#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub chars: Vec<Char>,
    pub lines: Vec<Line>,
    pub boxes: Vec<TextBox>,
    pub group_store: Vec<Group>,
    /// The top-level groups (pdfminer's layout.groups): None when the page has no text or boxes_flow is None.
    pub groups: Option<Vec<Node>>,
}

impl Layout {
    pub fn rect(&self, n: Node) -> Rect {
        match n {
            Node::Box(i) => self.boxes[i].rect,
            Node::Group(i) => self.group_store[i].rect,
        }
    }

    pub fn line_text(&self, line: usize) -> String {
        self.lines[line].text(&self.chars)
    }

    /// The text lines of a node, depth first in order.
    pub fn node_lines(&self, n: Node, out: &mut Vec<usize>) {
        match n {
            Node::Box(i) => out.extend(&self.boxes[i].lines),
            Node::Group(i) => {
                for &m in &self.group_store[i].members {
                    self.node_lines(m, out);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ Plane

/// Python int() then floor division: the grid cells a range covers.
fn drange(v0: f64, v1: f64, d: i64) -> std::ops::Range<i64> {
    ((v0.trunc() as i64).div_euclid(d))..(((v1 + d as f64).trunc() as i64).div_euclid(d))
}

/// pdfminer's Plane: objects on a grid of 50-unit cells, found by area, kept in the order they were added.
pub struct Plane<K: Copy + Eq + Hash> {
    seq: Vec<K>,
    objs: HashSet<K>,
    rects: HashMap<K, Rect>,
    grid: HashMap<(i64, i64), Vec<K>>,
    bbox: Rect,
}

impl<K: Copy + Eq + Hash> Plane<K> {
    const GRID: i64 = 50;

    pub fn new(bbox: Rect) -> Plane<K> {
        Plane { seq: Vec::new(), objs: HashSet::new(), rects: HashMap::new(), grid: HashMap::new(), bbox }
    }

    fn range(&self, r: &Rect) -> Vec<(i64, i64)> {
        let b = &self.bbox;
        if r.x1 <= b.x0 || b.x1 <= r.x0 || r.y1 <= b.y0 || b.y1 <= r.y0 {
            return Vec::new();
        }
        let (x0, y0, x1, y1) = (b.x0.max(r.x0), b.y0.max(r.y0), b.x1.min(r.x1), b.y1.min(r.y1));
        let mut out = Vec::new();
        for gy in drange(y0, y1, Self::GRID) {
            for gx in drange(x0, x1, Self::GRID) {
                out.push((gx, gy));
            }
        }
        out
    }

    pub fn add(&mut self, k: K, r: Rect) {
        for cell in self.range(&r) {
            self.grid.entry(cell).or_default().push(k);
        }
        self.seq.push(k);
        self.objs.insert(k);
        self.rects.insert(k, r);
    }

    pub fn remove(&mut self, k: K) {
        let Some(r) = self.rects.get(&k).copied() else { return };
        for cell in self.range(&r) {
            if let Some(v) = self.grid.get_mut(&cell) {
                if let Some(p) = v.iter().position(|&x| x == k) {
                    v.remove(p);
                }
            }
        }
        self.objs.remove(&k);
    }

    /// The objects that overlap an area (touching edges do not count), in the order of the grid cells.
    pub fn find(&self, r: &Rect) -> Vec<K> {
        let mut done = HashSet::new();
        let mut out = Vec::new();
        for cell in self.range(r) {
            let Some(v) = self.grid.get(&cell) else { continue };
            for &k in v {
                if !done.insert(k) {
                    continue;
                }
                let o = self.rects[&k];
                if o.x1 <= r.x0 || r.x1 <= o.x0 || o.y1 <= r.y0 || r.y1 <= o.y0 {
                    continue;
                }
                out.push(k);
            }
        }
        out
    }

    /// The objects in the plane, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = K> + '_ {
        self.seq.iter().copied().filter(|k| self.objs.contains(k))
    }
}

// ------------------------------------------------------------------ the analysis

/// Characters to lines (group_objects), in the order the characters were drawn.
fn group_objects(p: &LaParams, chars: &[Char]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut line: Option<Line> = None;
    let mut obj0: Option<usize> = None;
    for obj1 in 0..chars.len() {
        if let Some(o0) = obj0 {
            let (a, b) = (chars[o0].rect, chars[obj1].rect);
            let halign = a.is_voverlap(&b)
                && a.height().min(b.height()) * p.line_overlap < a.voverlap(&b)
                && a.hdistance(&b) < a.width().max(b.width()) * p.char_margin;
            let valign = p.detect_vertical
                && a.is_hoverlap(&b)
                && a.width().min(b.width()) * p.line_overlap < a.hoverlap(&b)
                && a.vdistance(&b) < a.height().max(b.height()) * p.char_margin;
            match line.as_mut() {
                Some(l) if (halign && !l.vertical) || (valign && l.vertical) => l.add(obj1, chars),
                Some(_) => out.push(line.take().unwrap()),
                None => {
                    if valign && !halign {
                        let mut l = Line::new(true, p.word_margin);
                        l.add(o0, chars);
                        l.add(obj1, chars);
                        line = Some(l);
                    } else if halign && !valign {
                        let mut l = Line::new(false, p.word_margin);
                        l.add(o0, chars);
                        l.add(obj1, chars);
                        line = Some(l);
                    } else {
                        let mut l = Line::new(false, p.word_margin);
                        l.add(o0, chars);
                        out.push(l);
                    }
                }
            }
        }
        obj0 = Some(obj1);
    }
    match line {
        Some(l) => out.push(l),
        None => {
            if let Some(o0) = obj0 {
                let mut l = Line::new(false, p.word_margin);
                l.add(o0, chars);
                out.push(l);
            }
        }
    }
    out
}

/// The lines near a line that may share its box (find_neighbors).
fn find_neighbors(lines: &[Line], plane: &Plane<usize>, i: usize, ratio: f64) -> Vec<usize> {
    let l = &lines[i];
    let r = l.rect;
    if l.vertical {
        let d = ratio * r.width();
        plane
            .find(&Rect::new(r.x0 - d, r.y0, r.x1 + d, r.y1))
            .into_iter()
            .filter(|&j| {
                let o = &lines[j];
                o.vertical
                    && (o.rect.width() - r.width()).abs() <= d
                    && ((o.rect.y0 - r.y0).abs() <= d
                        || (o.rect.y1 - r.y1).abs() <= d
                        || ((o.rect.y0 + o.rect.y1) / 2.0 - (r.y0 + r.y1) / 2.0).abs() <= d)
            })
            .collect()
    } else {
        let d = ratio * r.height();
        plane
            .find(&Rect::new(r.x0, r.y0 - d, r.x1, r.y1 + d))
            .into_iter()
            .filter(|&j| {
                let o = &lines[j];
                !o.vertical
                    && (o.rect.height() - r.height()).abs() <= d
                    && ((o.rect.x0 - r.x0).abs() <= d
                        || (o.rect.x1 - r.x1).abs() <= d
                        || ((o.rect.x0 + o.rect.x1) / 2.0 - (r.x0 + r.x1) / 2.0).abs() <= d)
            })
            .collect()
    }
}

/// Lines to boxes (group_textlines): `lines` are the indexes of the non-empty lines, in order.
fn group_textlines(p: &LaParams, page: Rect, all: &[Line], lines: &[usize]) -> Vec<TextBox> {
    let mut plane = Plane::new(page);
    for &i in lines {
        plane.add(i, all[i].rect);
    }
    let mut store: Vec<Vec<usize>> = Vec::new();
    let mut box_of: HashMap<usize, usize> = HashMap::new();
    for &line in lines {
        let mut members = vec![line];
        for obj1 in find_neighbors(all, &plane, line, p.line_margin) {
            members.push(obj1);
            if let Some(b) = box_of.remove(&obj1) {
                members.extend(store[b].clone());
            }
        }
        let b = store.len();
        store.push(Vec::new());
        let mut seen = HashSet::new();
        for m in members {
            if seen.insert(m) {
                store[b].push(m);
                box_of.insert(m, b);
            }
        }
    }
    let mut out = Vec::new();
    let mut done = HashSet::new();
    for &line in lines {
        let Some(&b) = box_of.get(&line) else { continue };
        if !done.insert(b) {
            continue;
        }
        let rect = store[b].iter().fold(Rect::EMPTY, |r, &l| r.union(&all[l].rect));
        if !rect.is_empty() {
            out.push(TextBox { vertical: all[line].vertical, rect, lines: store[b].clone() });
        }
    }
    out
}

#[derive(PartialEq)]
struct Dist(f64);
impl Eq for Dist {}
impl PartialOrd for Dist {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Dist {
    fn cmp(&self, o: &Self) -> Ordering {
        self.0.total_cmp(&o.0)
    }
}

/// Boxes to a hierarchy of groups, closest pairs first (group_textboxes). Returns the top-level nodes.
fn group_textboxes(page: Rect, layout: &mut Layout) -> Vec<Node> {
    let nboxes = layout.boxes.len();
    // Element ids: boxes 0..n, then groups in the order they are made (in place of Python's id()).
    let node_of = |id: usize| if id < nboxes { Node::Box(id) } else { Node::Group(id - nboxes) };
    let id_of = |n: Node| match n {
        Node::Box(i) => i,
        Node::Group(i) => nboxes + i,
    };
    let dist = |a: Rect, b: Rect| {
        let u = a.union(&b);
        u.width() * u.height() - a.width() * a.height() - b.width() * b.height()
    };
    let mut heap: BinaryHeap<Reverse<(bool, Dist, usize, usize)>> = BinaryHeap::new();
    for i in 0..nboxes {
        for j in i + 1..nboxes {
            heap.push(Reverse((false, Dist(dist(layout.boxes[i].rect, layout.boxes[j].rect)), i, j)));
        }
    }
    let mut plane: Plane<usize> = Plane::new(page);
    for i in 0..nboxes {
        plane.add(i, layout.boxes[i].rect);
    }
    let mut done: HashSet<usize> = HashSet::new();
    while let Some(Reverse((skip_isany, d, id1, id2))) = heap.pop() {
        if done.contains(&id1) || done.contains(&id2) {
            continue;
        }
        let (n1, n2) = (node_of(id1), node_of(id2));
        let (r1, r2) = (layout.rect(n1), layout.rect(n2));
        if !skip_isany {
            let any = plane.find(&r1.union(&r2)).into_iter().any(|k| k != id1 && k != id2);
            if any {
                heap.push(Reverse((true, d, id1, id2)));
                continue;
            }
        }
        let vertical = |n: Node| match n {
            Node::Box(i) => layout.boxes[i].vertical,
            Node::Group(i) => layout.group_store[i].tbrl,
        };
        let tbrl = vertical(n1) || vertical(n2);
        let g = layout.group_store.len();
        layout.group_store.push(Group { tbrl, rect: r1.union(&r2), members: vec![n1, n2] });
        let gid = id_of(Node::Group(g));
        plane.remove(id1);
        plane.remove(id2);
        done.insert(id1);
        done.insert(id2);
        let grect = layout.group_store[g].rect;
        for other in plane.iter().collect::<Vec<_>>() {
            heap.push(Reverse((false, Dist(dist(grect, layout.rect(node_of(other)))), gid, other)));
        }
        plane.add(gid, grect);
    }
    plane.iter().map(node_of).collect()
}

/// analyze() of a box or group: a line end after every line, lines and members sorted into reading order.
fn analyze_node(p: &LaParams, layout: &mut Layout, n: Node) {
    match n {
        Node::Box(i) => {
            for k in 0..layout.boxes[i].lines.len() {
                let l = layout.boxes[i].lines[k];
                layout.lines[l].items.push(LineItem::Anno("\n"));
            }
            let vertical = layout.boxes[i].vertical;
            let mut lines = std::mem::take(&mut layout.boxes[i].lines);
            if vertical {
                lines.sort_by(|&a, &b| (-layout.lines[a].rect.x1).total_cmp(&-layout.lines[b].rect.x1));
            } else {
                lines.sort_by(|&a, &b| (-layout.lines[a].rect.y1).total_cmp(&-layout.lines[b].rect.y1));
            }
            layout.boxes[i].lines = lines;
        }
        Node::Group(i) => {
            for k in 0..layout.group_store[i].members.len() {
                let m = layout.group_store[i].members[k];
                analyze_node(p, layout, m);
            }
            let flow = p.boxes_flow.unwrap_or(0.0);
            let tbrl = layout.group_store[i].tbrl;
            let key = |r: Rect| {
                if tbrl {
                    -(1.0 + flow) * (r.x0 + r.x1) - (1.0 - flow) * r.y1
                } else {
                    (1.0 - flow) * r.x0 - (1.0 + flow) * (r.y0 + r.y1)
                }
            };
            let mut members = std::mem::take(&mut layout.group_store[i].members);
            members.sort_by(|&a, &b| key(layout.rect(a)).total_cmp(&key(layout.rect(b))));
            layout.group_store[i].members = members;
        }
    }
}

/// The layout analysis of a page (LTLayoutContainer.analyze): `chars` are the page's own characters (not those of
/// figures) in the order they were drawn; `page` is the page's box from (0, 0).
pub fn analyze(p: &LaParams, page: Rect, chars: Vec<Char>) -> Layout {
    let mut layout = Layout { chars, ..Layout::default() };
    if layout.chars.is_empty() {
        return layout;
    }
    layout.lines = group_objects(p, &layout.chars);
    let lines: Vec<usize> = (0..layout.lines.len()).filter(|&i| !layout.lines[i].is_empty(&layout.chars)).collect();
    let boxes = group_textlines(p, page, &layout.lines, &lines);
    layout.boxes = boxes;
    if p.boxes_flow.is_none() {
        // pdfminer sorts the boxes and leaves the groups unset; the importer reads only the groups.
        return layout;
    }
    let groups = group_textboxes(page, &mut layout);
    for &g in &groups {
        analyze_node(p, &mut layout, g);
    }
    layout.groups = Some(groups);
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(x: f64, y: f64, s: &str) -> Vec<Char> {
        s.chars().enumerate().map(|(i, c)| Char { rect: Rect::new(x + 6.0 * i as f64, y, x + 6.0 * (i + 1) as f64, y + 10.0), text: c.to_string() }).collect()
    }

    fn texts(l: &Layout) -> Vec<String> {
        let mut lines = Vec::new();
        for &g in l.groups.as_ref().unwrap() {
            l.node_lines(g, &mut lines);
        }
        lines.into_iter().map(|i| l.line_text(i)).collect()
    }

    #[test]
    fn words_lines_and_boxes() {
        let mut chars = word(72.0, 700.0, "Hello");
        chars.extend(word(110.0, 700.0, "world"));
        chars.extend(word(72.0, 688.0, "Next"));
        // A second column far to the right.
        chars.extend(word(400.0, 700.0, "Side"));
        let l = analyze(&LaParams::default(), Rect::new(0.0, 0.0, 612.0, 792.0), chars);
        assert_eq!(texts(&l), vec!["Hello world\n", "Next\n", "Side\n"]);
        assert_eq!(l.boxes.len(), 2);
    }

    #[test]
    fn plane_finds_by_area_in_grid_order() {
        let mut p: Plane<usize> = Plane::new(Rect::new(0.0, 0.0, 200.0, 200.0));
        p.add(1, Rect::new(10.0, 10.0, 20.0, 20.0));
        p.add(2, Rect::new(60.0, 10.0, 70.0, 20.0));
        p.add(3, Rect::new(10.0, 60.0, 20.0, 70.0));
        assert_eq!(p.find(&Rect::new(0.0, 0.0, 100.0, 100.0)), vec![1, 2, 3]);
        // Touching edges do not count.
        assert_eq!(p.find(&Rect::new(20.0, 0.0, 60.0, 30.0)), Vec::<usize>::new());
        p.remove(2);
        assert_eq!(p.iter().collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(drange(-0.5, 49.0, 50), 0..1);
    }
}
