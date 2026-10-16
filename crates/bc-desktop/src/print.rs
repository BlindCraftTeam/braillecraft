//! File, Print and Printer Setup, as the reference program does them (re/spec/24-print.md).
//!
//! Printer Setup (dialog 1071, for the document or for the program) keeps the printer, the paper, the orientation,
//! the margins with their unit and the formatted page size. File, Print (dialog 1040) takes the copies, the page
//! range and, for a braille document, interline print, then prints the laid-out pages: one print job per copy, each
//! line drawn in the document's view font at a size that fills the page (Global, Fonts: "Auto-determine point size")
//! or at the size set there, with the line numbers of a braille page in a column on each side when Global, View
//! Preferences shows line numbers, and under each braille line its print in a 9-point font when interline print is on.
//!
//! The reference program prints through the Windows printer directly; BrailleCraft prints through wxWidgets printing
//! (wxPrinter and a wxPrintout), our port choice, so the same pages print on Windows, macOS and Linux.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_void, CString};
use std::rc::Rc;

use wxdragon::dc::GenericDC;
use wxdragon::prelude::*;
use wxdragon_sys as ffi;

use bc_core::format::Page;

/// What Printer Setup keeps, shared with the web build (bc_core::printer_setup).
pub use bc_core::printer_setup::{parse_margin, PrinterSetup, PAPERS};

// ------------------------------------------------------------------ fonts

/// The system's font face names in the order the system gives them (wxFontEnumerator); the TranSend SE page lists
/// them as the reference lists every font family of the screen (spec 25 14.1).
pub fn font_names() -> Vec<String> {
    let n = unsafe { ffi::wxd_FontEnumerator_GetFacenames(std::ptr::null_mut(), 0) };
    if n <= 0 {
        return Vec::new();
    }
    let mut buf = vec![0u8; n as usize + 1];
    unsafe { ffi::wxd_FontEnumerator_GetFacenames(buf.as_mut_ptr() as *mut _, buf.len() as i32) };
    buf.truncate(n as usize);
    String::from_utf8_lossy(&buf).lines().filter(|l| !l.is_empty()).map(str::to_string).collect()
}

// ------------------------------------------------------------------ print data

/// A wxWidgets print data object (printer, paper, orientation, and the driver's own settings).
pub struct PrintData(*mut ffi::wxd_PrintData_t);

impl PrintData {
    fn new() -> PrintData {
        PrintData(unsafe { ffi::wxd_PrintData_Create() })
    }

    fn copy_of(other: *mut ffi::wxd_PrintData_t) -> PrintData {
        let d = PrintData::new();
        unsafe { ffi::wxd_PrintData_Assign(d.0, other) };
        d
    }

    fn printer_name(&self) -> String {
        let n = unsafe { ffi::wxd_PrintData_GetPrinterName(self.0, std::ptr::null_mut(), 0) };
        if n <= 0 {
            return String::new();
        }
        let mut buf = vec![0u8; n as usize + 1];
        unsafe { ffi::wxd_PrintData_GetPrinterName(self.0, buf.as_mut_ptr() as *mut _, buf.len() as i32) };
        buf.truncate(n as usize);
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn paper(&self) -> (i32, i32) {
        let (mut w, mut h) = (0, 0);
        unsafe { ffi::wxd_PrintData_GetPaperSizeTenthsMM(self.0, &mut w, &mut h) };
        (w, h)
    }

    fn landscape(&self) -> bool {
        unsafe { ffi::wxd_PrintData_IsLandscape(self.0) }
    }

    /// Sets the printer, paper and orientation of `setup`.
    fn apply(&self, setup: &PrinterSetup) {
        let name = CString::new(setup.printer.as_str()).unwrap_or_default();
        unsafe {
            ffi::wxd_PrintData_SetPrinterName(self.0, name.as_ptr());
            if setup.paper.0 > 0 && setup.paper.1 > 0 {
                ffi::wxd_PrintData_SetPaperSizeTenthsMM(self.0, setup.paper.0, setup.paper.1);
            }
            ffi::wxd_PrintData_SetLandscape(self.0, setup.landscape);
            ffi::wxd_PrintData_SetNoCopies(self.0, 1);
        }
    }
}

impl Drop for PrintData {
    fn drop(&mut self) {
        unsafe { ffi::wxd_PrintData_Destroy(self.0) };
    }
}

thread_local! {
    /// The driver settings chosen with Printer Setup's Printer... button, by document key (0 for Global, Printer
    /// Setup), kept while the program runs.
    static DRIVER: RefCell<HashMap<u64, PrintData>> = RefCell::new(HashMap::new());
}

/// The print data for document `key` (0 for the program): the driver settings last chosen for it, or for the
/// program, with the printer, paper and orientation of `setup`.
fn print_data(key: u64, setup: &PrinterSetup) -> PrintData {
    let d = DRIVER.with(|m| {
        let m = m.borrow();
        match m.get(&key).or_else(|| m.get(&0)) {
            Some(kept) => PrintData::copy_of(kept.0),
            None => PrintData::new(),
        }
    });
    d.apply(setup);
    d
}

/// Printer Setup's Printer... button: the system's printer dialog, to choose the printer and its own settings.
/// Returns the chosen print data, and `setup` takes its printer, paper and orientation; None when cancelled.
pub fn choose_printer(key: u64, setup: &mut PrinterSetup) -> Option<PrintData> {
    let data = print_data(key, setup);
    let frame = crate::app::frame();
    unsafe {
        // wxWidgets 3.3 has no separate printer setup dialog: the system's print dialog chooses the printer and its
        // settings, and nothing prints from here.
        let dd = ffi::wxd_PrintDialogData_CreateFromData(data.0);
        let dialog = ffi::wxd_PrintDialog_Create(frame.handle_ptr(), dd);
        let ok = ffi::wxd_PrintDialog_ShowModal(dialog) == ID_OK;
        let chosen = ok.then(|| {
            let got = ffi::wxd_PrintDialog_GetPrintDialogData(dialog);
            PrintData::copy_of(ffi::wxd_PrintDialogData_GetPrintData(got))
        });
        ffi::wxd_PrintDialog_Destroy(dialog);
        ffi::wxd_PrintDialogData_Destroy(dd);
        let chosen = chosen?;
        setup.printer = chosen.printer_name();
        let paper = chosen.paper();
        if paper.0 > 0 && paper.1 > 0 {
            setup.paper = paper;
        }
        setup.landscape = chosen.landscape();
        Some(chosen)
    }
}

/// Keeps the driver settings chosen with Printer... for document `key` (0 for the program).
pub fn keep_driver(key: u64, data: PrintData) {
    DRIVER.with(|m| m.borrow_mut().insert(key, data));
}

// ------------------------------------------------------------------ page ranges

/// The parser is shared with the web build (bc_core::range).
pub use bc_core::range::{parse_range, Range};

// ------------------------------------------------------------------ the print job

/// One print job: the pages to print and how to draw them.
pub struct Job {
    pub title: String,
    pub pages: Vec<Option<Rc<Page>>>,
    /// For each page and line, the line's print for interline print (braille documents only).
    pub interline: Vec<Vec<Option<String>>>,
    pub face: String,
    /// The point size to print at; None to find the largest that fits the page.
    pub size: Option<i32>,
    pub print_face: String,
    pub lines: usize,
    pub chars: usize,
    pub line_numbers: bool,
    pub margins: [i32; 4],
}

/// The page geometry and fonts found for a job on its first page.
struct Fit {
    size: i32,
    char_w: i32,
    text_h: i32,
    line_h: i32,
    number_w: i32,
}

struct Proxy {
    job: Rc<Job>,
    printout: *mut ffi::wxd_Printout_t,
    fit: Option<Fit>,
}

impl Proxy {
    unsafe fn from(user_data: *mut c_void) -> &'static mut Proxy {
        unsafe { &mut *(user_data as *mut Proxy) }
    }
}

unsafe extern "C" fn has_page(user_data: *mut c_void, page: i32) -> bool {
    let p = unsafe { Proxy::from(user_data) };
    page >= 1 && page as usize <= p.job.pages.len()
}

unsafe extern "C" fn page_info(user_data: *mut c_void, min: *mut i32, max: *mut i32, from: *mut i32, to: *mut i32) {
    let p = unsafe { Proxy::from(user_data) };
    let n = p.job.pages.len() as i32;
    unsafe {
        for (ptr, v) in [(min, 1), (max, n), (from, 1), (to, n)] {
            if !ptr.is_null() {
                *ptr = v;
            }
        }
    }
}

unsafe extern "C" fn print_page(user_data: *mut c_void, page: i32) -> bool {
    let p = unsafe { Proxy::from(user_data) };
    let dc_ptr = unsafe { ffi::wxd_Printout_GetDC(p.printout) };
    if dc_ptr.is_null() {
        return false;
    }
    let dc = unsafe { GenericDC::from_ffi_ptr_unowned(dc_ptr) };
    let mut g = Geometry::default();
    unsafe {
        ffi::wxd_Printout_GetPPIPrinter(p.printout, &mut g.ppi_printer.0, &mut g.ppi_printer.1);
        ffi::wxd_Printout_GetPPIScreen(p.printout, &mut g.ppi_screen.0, &mut g.ppi_screen.1);
        ffi::wxd_Printout_GetPageSizePixels(p.printout, &mut g.page.0, &mut g.page.1);
        ffi::wxd_Printout_GetPaperRectPixels(p.printout, &mut g.paper.0, &mut g.paper.1, &mut g.paper.2, &mut g.paper.3);
    }
    let job = p.job.clone();
    draw_page(&job, &mut p.fit, &dc, &g, page as usize);
    true
}

/// What the printer reports about a page: its resolution and the screen's, the printable area's size, and the
/// paper's rectangle relative to the printable area's origin, all in printer pixels.
#[derive(Default)]
struct Geometry {
    ppi_printer: (i32, i32),
    ppi_screen: (i32, i32),
    page: (i32, i32),
    paper: (i32, i32, i32, i32),
}

/// Draws page `n` (from 1) of the job. Drawing is in screen-sized units scaled to the printer (as wxWidgets maps a
/// page), so point sizes print at their size.
fn draw_page<D: DeviceContext>(job: &Job, fit_found: &mut Option<Fit>, dc: &D, g: &Geometry, n: usize) {
    let (ppx, ppy) = g.ppi_printer;
    let (psx, psy) = (g.ppi_screen.0.max(1) as f64, g.ppi_screen.1.max(1) as f64);
    let (pw, ph) = g.page;
    let (rx, ry, rw, rh) = g.paper;
    let (sx, sy) = (ppx.max(1) as f64 / psx, ppy.max(1) as f64 / psy);
    dc.set_user_scale(sx, sy);
    let lx = |v: i32| (v as f64 / sx).round() as i32;
    let ly = |v: i32| (v as f64 / sy).round() as i32;
    let m = job.margins;
    let margin_x = |t: i32| (t as f64 * psx / 1000.0).round() as i32;
    let margin_y = |t: i32| (t as f64 * psy / 1000.0).round() as i32;
    // The text starts at the larger of the printable area's edge and the margin from the paper's edge (spec 24, 3c).
    let left = (lx(rx) + margin_x(m[0])).max(0);
    let top = (ly(ry) + margin_y(m[1])).max(0);
    let right = (lx(rx + rw) - margin_x(m[2])).min(lx(pw)).max(left + 1);
    let bottom = (ly(ry + rh) - margin_y(m[3])).min(ly(ph)).max(top + 1);
    if fit_found.is_none() {
        *fit_found = Some(fit(dc, job, right - left, bottom - top));
    }
    let Some(f) = fit_found.as_ref() else { return };
    let Some(Some(page)) = job.pages.get(n - 1) else { return };
    let text_font = crate::app::font(&job.face, f.size);
    let number_font = crate::app::font(&job.print_face, f.size);
    let interline_font = crate::app::font(&job.print_face, 9);
    // Line numbers: one column at the left edge, the text moved right by the width of "25" and a half, and one
    // column after the text block (spec 24, 3d).
    let text_x = if job.line_numbers { left + f.number_w + f.number_w / 2 } else { left };
    let right_numbers = text_x + job.chars as i32 * f.char_w + f.number_w / 2;
    for (i, line) in page.lines.iter().enumerate().take(job.lines) {
        let y = top + i as i32 * f.line_h;
        if job.line_numbers {
            if let Some(font) = &number_font {
                dc.set_font(font);
            }
            let number = (i + 1).to_string();
            dc.draw_text(&number, left, y);
            dc.draw_text(&number, right_numbers, y);
        }
        if !line.text.trim().is_empty() {
            if let Some(font) = &text_font {
                dc.set_font(font);
            }
            dc.draw_text(line.text.trim_end(), text_x, y);
        }
        if let Some(Some(print)) = job.interline.get(n - 1).and_then(|l| l.get(i)) {
            if let Some(font) = &interline_font {
                dc.set_font(font);
            }
            dc.draw_text(print, text_x, y + f.text_h);
        }
    }
}

/// The point size and line height for the job: its own size, or the largest from 1 to 48 points at which its
/// characters per line and lines per page (with the line-number columns and interline print) fit the space
/// (spec 24, 3c).
fn fit<D: DeviceContext>(dc: &D, job: &Job, width: i32, height: i32) -> Fit {
    let interline = job.interline.iter().flatten().any(Option::is_some);
    let extra = if interline {
        crate::app::font(&job.print_face, 9).map(|f| {
            dc.set_font(&f);
            dc.get_text_extent("Xg").1
        })
    } else {
        None
    }
    .unwrap_or(0);
    let measure = |size: i32| -> Option<Fit> {
        let font = crate::app::font(&job.face, size)?;
        dc.set_font(&font);
        let (w, h) = dc.get_text_extent(&"x".repeat(job.chars.max(1)));
        let number_w = if job.line_numbers {
            crate::app::font(&job.print_face, size).map(|nf| {
                dc.set_font(&nf);
                dc.get_text_extent("25").0
            })?
        } else {
            0
        };
        let line_h = h + extra + extra / 10;
        Some(Fit { size, char_w: w / job.chars.max(1) as i32, text_h: h, line_h, number_w })
    };
    if let Some(size) = job.size {
        if let Some(f) = measure(size.max(1)) {
            return f;
        }
    }
    for size in (1..=48).rev() {
        if let Some(f) = measure(size) {
            let used_w = f.char_w * job.chars as i32 + if job.line_numbers { f.number_w * 2 + f.number_w / 2 } else { 0 };
            if used_w <= width && f.line_h * job.lines as i32 <= height {
                return f;
            }
        }
    }
    measure(1).unwrap_or(Fit { size: 1, char_w: 1, text_h: 1, line_h: 1, number_w: 0 })
}

/// How a print ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Printed,
    Cancelled,
    Failed,
    /// No print server to get printers from (print_server_reachable).
    NoPrinter,
}

/// Prints `job` with the printer, paper and orientation of `setup` (and the driver settings kept for document
/// `key`), `copies` times: one print job per copy, as the reference program does (spec 24, 3a).
pub fn run(key: u64, setup: &PrinterSetup, job: Job, copies: u32) -> Outcome {
    #[cfg(debug_assertions)]
    if let Ok(folder) = std::env::var("BC_PRINT_TO_PPM") {
        return render_pages(setup, &job, std::path::Path::new(&folder));
    }
    if !print_server_reachable() {
        return Outcome::NoPrinter;
    }
    let data = print_data(key, setup);
    let frame = crate::app::frame();
    let job = Rc::new(job);
    let title = CString::new(job.title.as_str()).unwrap_or_default();
    unsafe {
        let dd = ffi::wxd_PrintDialogData_CreateFromData(data.0);
        let printer = ffi::wxd_Printer_Create(dd);
        let mut outcome = Outcome::Printed;
        for _ in 0..copies.max(1) {
            let mut proxy = Box::new(Proxy { job: job.clone(), printout: std::ptr::null_mut(), fit: None });
            let user_data = &mut *proxy as *mut Proxy as *mut c_void;
            proxy.printout = ffi::wxd_Printout_CreateWithCallbacks(
                title.as_ptr(),
                user_data,
                None,
                None,
                None,
                None,
                None,
                Some(print_page),
                Some(has_page),
                Some(page_info),
            );
            let ok = ffi::wxd_Printer_Print(printer, frame.handle_ptr(), proxy.printout, false);
            ffi::wxd_Printout_Destroy(proxy.printout);
            if !ok {
                outcome = if ffi::wxd_Printer_GetLastError() == 1 { Outcome::Cancelled } else { Outcome::Failed };
                break;
            }
        }
        ffi::wxd_Printer_Destroy(printer);
        ffi::wxd_PrintDialogData_Destroy(dd);
        outcome
    }
}

// ------------------------------------------------------------------ the ink pass of an embossing job

/// The ink pass of an embossing job with interline print (spec 25 14.5 and 17): its pages, each a list of pieces of
/// ink placed in twips from the printable area's top left, drawn in font `face` ("Times New Roman" when empty, as in
/// the reference) at a cell height of `height` twips.
pub struct InkJob {
    pub title: String,
    pub pages: Vec<Vec<bc_core::ink::Item>>,
    pub face: String,
    pub height: i32,
}

struct InkProxy {
    job: Rc<InkJob>,
    printout: *mut ffi::wxd_Printout_t,
    copies: u32,
}

impl InkProxy {
    unsafe fn from(user_data: *mut c_void) -> &'static mut InkProxy {
        unsafe { &mut *(user_data as *mut InkProxy) }
    }

    fn count(&self) -> i32 {
        (self.job.pages.len() as u32 * self.copies.max(1)) as i32
    }
}

unsafe extern "C" fn ink_has_page(user_data: *mut c_void, page: i32) -> bool {
    let p = unsafe { InkProxy::from(user_data) };
    page >= 1 && page <= p.count()
}

unsafe extern "C" fn ink_page_info(user_data: *mut c_void, min: *mut i32, max: *mut i32, from: *mut i32, to: *mut i32) {
    let p = unsafe { InkProxy::from(user_data) };
    let n = p.count();
    unsafe {
        for (ptr, v) in [(min, 1), (max, n), (from, 1), (to, n)] {
            if !ptr.is_null() {
                *ptr = v;
            }
        }
    }
}

unsafe extern "C" fn ink_print_page(user_data: *mut c_void, page: i32) -> bool {
    let p = unsafe { InkProxy::from(user_data) };
    let dc_ptr = unsafe { ffi::wxd_Printout_GetDC(p.printout) };
    if dc_ptr.is_null() {
        return false;
    }
    let dc = unsafe { GenericDC::from_ffi_ptr_unowned(dc_ptr) };
    let mut ppi = (0, 0);
    unsafe { ffi::wxd_Printout_GetPPIPrinter(p.printout, &mut ppi.0, &mut ppi.1) };
    let pages = p.job.pages.len().max(1);
    draw_ink_page(&p.job, &dc, ppi, (page as usize - 1) % pages);
    true
}

/// Draws ink page `n` (from 0): in twips, as the reference draws on a device context in MM_TWIPS; each piece against
/// the bottom of its box and cut to the box, as its DrawTextW with DT_SINGLELINE | DT_BOTTOM | DT_NOPREFIX and no
/// DT_NOCLIP does (spec 25 14.5).
fn draw_ink_page<D: DeviceContext>(job: &InkJob, dc: &D, ppi: (i32, i32), n: usize) {
    dc.set_user_scale(ppi.0.max(1) as f64 / 1440.0, ppi.1.max(1) as f64 / 1440.0);
    let face = if job.face.is_empty() { "Times New Roman" } else { job.face.as_str() };
    // The font at the cell height the reference asks for: measured at 12 points, then sized to it.
    let Some(probe) = crate::app::font(face, 12) else { return };
    dc.set_font(&probe);
    let at12 = dc.get_char_height().max(1);
    let points = ((12 * job.height) as f64 / at12 as f64).round().max(1.0) as i32;
    let Some(font) = crate::app::font(face, points) else { return };
    dc.set_font(&font);
    let text_h = dc.get_char_height();
    let Some(items) = job.pages.get(n) else { return };
    for item in items {
        dc.set_clipping_region(item.x, item.top, item.width, item.bottom - item.top);
        dc.draw_text(&item.text, item.x, item.bottom - text_h);
        dc.destroy_clipping_region();
    }
}

/// Prints the ink pass `job` to printer `printer` with its own paper, `copies` times within the one print job (the
/// reference's copies are made inside each pass, spec 25 17.2), without a print dialog.
pub fn run_ink(printer: &str, job: InkJob, copies: u32) -> Outcome {
    #[cfg(debug_assertions)]
    if let Ok(folder) = std::env::var("BC_PRINT_TO_PPM") {
        return render_ink_pages(&job, std::path::Path::new(&folder));
    }
    if !print_server_reachable() {
        return Outcome::NoPrinter;
    }
    let data = PrintData::new();
    let name = CString::new(printer).unwrap_or_default();
    unsafe {
        ffi::wxd_PrintData_SetPrinterName(data.0, name.as_ptr());
        ffi::wxd_PrintData_SetNoCopies(data.0, 1);
    }
    let frame = crate::app::frame();
    let title = CString::new(job.title.as_str()).unwrap_or_default();
    let job = Rc::new(job);
    unsafe {
        let dd = ffi::wxd_PrintDialogData_CreateFromData(data.0);
        let wx_printer = ffi::wxd_Printer_Create(dd);
        let mut proxy = Box::new(InkProxy { job, printout: std::ptr::null_mut(), copies });
        let user_data = &mut *proxy as *mut InkProxy as *mut c_void;
        proxy.printout = ffi::wxd_Printout_CreateWithCallbacks(
            title.as_ptr(),
            user_data,
            None,
            None,
            None,
            None,
            None,
            Some(ink_print_page),
            Some(ink_has_page),
            Some(ink_page_info),
        );
        let ok = ffi::wxd_Printer_Print(wx_printer, frame.handle_ptr(), proxy.printout, false);
        ffi::wxd_Printout_Destroy(proxy.printout);
        let outcome = if ok {
            Outcome::Printed
        } else if ffi::wxd_Printer_GetLastError() == 1 {
            Outcome::Cancelled
        } else {
            Outcome::Failed
        };
        ffi::wxd_Printer_Destroy(wx_printer);
        ffi::wxd_PrintDialogData_Destroy(dd);
        outcome
    }
}

/// For checks: the ink pages drawn to ink-<n>.ppm files in `folder` on US Letter at 96 pixels per inch.
#[cfg(debug_assertions)]
fn render_ink_pages(job: &InkJob, folder: &std::path::Path) -> Outcome {
    use wxdragon::dc::MemoryDC;
    let (w, h) = (816, 1056);
    for (n, _) in job.pages.iter().enumerate() {
        let Some(mut bitmap) = wxdragon::bitmap::Bitmap::new(w, h) else { return Outcome::Failed };
        {
            let mut dc = MemoryDC::new();
            dc.select_object(&mut bitmap);
            dc.set_brush(Colour::new(255, 255, 255, 255), BrushStyle::Solid);
            dc.set_pen(Colour::new(255, 255, 255, 255), 1, PenStyle::Solid);
            dc.draw_rectangle(0, 0, w, h);
            dc.set_text_foreground(Colour::new(0, 0, 0, 255));
            draw_ink_page(job, &dc, (96, 96), n);
        }
        let Some(rgba) = bitmap.get_rgba_data() else { return Outcome::Failed };
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for px in rgba.chunks(4) {
            ppm.extend_from_slice(&px[..3]);
        }
        if std::fs::write(folder.join(format!("ink-{}.ppm", n + 1)), ppm).is_err() {
            return Outcome::Failed;
        }
    }
    Outcome::Printed
}

/// Whether the system has a print server to ask for printers. On Linux the system print dialog's printer list comes
/// from CUPS; without a CUPS server to reach, a print waits for it without end, so File, Print says so instead
/// (BrailleCraft's own check; the reference program shows message 2632 when it cannot get printer information).
pub fn print_server_reachable() -> bool {
    if cfg!(target_os = "linux") {
        std::env::var_os("CUPS_SERVER").is_some()
            || ["/run/cups/cups.sock", "/var/run/cups/cups.sock"].iter().any(|p| std::path::Path::new(p).exists())
    } else {
        true
    }
}

/// Debug builds only: with BC_PRINT_TO_PPM set to a folder, File, Print draws each page of the job, with the same
/// drawing as the printer, into page-N.ppm there (a 96-pixels-per-inch image of the paper) instead of printing, so the
/// page layout can be checked where no printer exists.
#[cfg(debug_assertions)]
fn render_pages(setup: &PrinterSetup, job: &Job, folder: &std::path::Path) -> Outcome {
    use wxdragon::dc::MemoryDC;
    let (pw, ph) = if setup.landscape { (setup.paper.1, setup.paper.0) } else { setup.paper };
    // Tenths of a millimetre to 96 pixels per inch.
    let (w, h) = ((pw as f64 * 96.0 / 254.0).round() as i32, (ph as f64 * 96.0 / 254.0).round() as i32);
    let g = Geometry { ppi_printer: (96, 96), ppi_screen: (96, 96), page: (w, h), paper: (0, 0, w, h) };
    let mut fit_found = None;
    for n in 1..=job.pages.len() {
        let Some(mut bitmap) = wxdragon::bitmap::Bitmap::new(w, h) else { return Outcome::Failed };
        {
            let mut dc = MemoryDC::new();
            dc.select_object(&mut bitmap);
            dc.set_brush(Colour::new(255, 255, 255, 255), BrushStyle::Solid);
            dc.set_pen(Colour::new(255, 255, 255, 255), 1, PenStyle::Solid);
            dc.draw_rectangle(0, 0, w, h);
            dc.set_text_foreground(Colour::new(0, 0, 0, 255));
            draw_page(job, &mut fit_found, &dc, &g, n);
        }
        let Some(rgba) = bitmap.get_rgba_data() else { return Outcome::Failed };
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for px in rgba.chunks(4) {
            ppm.extend_from_slice(&px[..3]);
        }
        if std::fs::write(folder.join(format!("page-{n}.ppm")), ppm).is_err() {
            return Outcome::Failed;
        }
    }
    Outcome::Printed
}
