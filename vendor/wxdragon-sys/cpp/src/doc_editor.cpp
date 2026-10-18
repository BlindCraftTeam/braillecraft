// Added for BrailleCraft: the drawn document editing window (wxd_doc_editor.h).
//
// The reference program draws its own editing window: rows of text, a row with a line across after each formatted
// page, the caret where the program puts it, and a line number column on the left when that is on; it gives screen
// readers its own text for the window (spec 27 sections 3 and 6). This window does the same with rows the program
// gives it. Columns are measured with the font's partial text extents, so a proportional print font places the caret
// and a click as well as a fixed-width braille font does. The system caret (wxCaret, the native caret on Windows) is
// kept at the caret place, as screen readers and magnifiers follow it.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/caret.h>
#include <wx/dcbuffer.h>
#include <wx/scrolwin.h>
#include <algorithm>
#include <vector>
#include "../include/wxdragon.h"
#include "doc_editor_internal.h"

// How many rows the window holds at once, each in the slot its number maps to: more than a screen of rows, so the
// rows shown and the ones around them stay, and a bounded amount for a document of any length (plan line L1).
static const int kCacheRows = 1024;

// The window's Windows class name and its control id. Screen reader scripts tell the editing area apart from every
// other window by its class (the reference program registers a class of its own for its editing window) and test
// its control id against 0 (spec 17 section 6); wxWidgets' shared "wxWindow" class and generated ids would hide it
// among the panels.
static const long kEditorStyle = wxWANTS_CHARS | wxVSCROLL | wxHSCROLL | wxBORDER_THEME | wxFULL_REPAINT_ON_RESIZE;
static const wxWindowID kEditorId = 0;

DocEditor::DocEditor(wxWindow* parent, void* data, wxd_DocEditor_KeyFn key, wxd_DocEditor_CharFn chr,
                     wxd_DocEditor_MouseFn mouse, wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data,
                     wxd_DocEditor_RowFn row)
    : m_data(data), m_key(key), m_char(chr), m_mouse(mouse), m_focus(focus), m_free(free_data), m_row(row),
      m_cache(kCacheRows), m_accessCache(kCacheRows), m_labelCache(kCacheRows)
{
    m_rowStart.assign(1, 0);
#ifdef __WXMSW__
    // As wxScrolled<T>::Create does, then the window under a class registered as "BrailleCraftEditor" (with the
    // redraw styles, since the window repaints on a resize) instead of the shared wxWindow class.
    m_targetWindow = this;
    CreateUsingMSWClass(wxApp::GetRegisteredClassName(wxT("BrailleCraftEditor")), parent, kEditorId, wxDefaultPosition,
                        wxDefaultSize, kEditorStyle);
#else
    wxScrolledCanvas::Create(parent, kEditorId, wxDefaultPosition, wxDefaultSize, kEditorStyle);
#endif
    SetBackgroundStyle(wxBG_STYLE_PAINT);
    SetFont(GetFont());
#ifndef __WXOSX__
    SetCaret(new wxCaret(this, 2, m_rowHeight));
    GetCaret()->Show();
#endif
    Bind(wxEVT_PAINT, &DocEditor::OnPaint, this);
    Bind(wxEVT_KEY_DOWN, &DocEditor::OnKeyDown, this);
    Bind(wxEVT_CHAR, &DocEditor::OnChar, this);
    Bind(wxEVT_KEY_UP, [this](wxKeyEvent& e) {
        if (m_key) m_key(m_data, -e.GetKeyCode(), (int)e.GetUnicodeKey(), e.GetModifiers());
        e.Skip();
    });
    Bind(wxEVT_LEFT_DOWN, &DocEditor::OnMouse, this);
    Bind(wxEVT_LEFT_UP, &DocEditor::OnMouse, this);
    Bind(wxEVT_LEFT_DCLICK, &DocEditor::OnMouse, this);
    Bind(wxEVT_MOTION, &DocEditor::OnMouse, this);
    Bind(wxEVT_MOUSE_CAPTURE_LOST, [this](wxMouseCaptureLostEvent&) {});
    Bind(wxEVT_SET_FOCUS, &DocEditor::OnFocus, this);
    Bind(wxEVT_KILL_FOCUS, &DocEditor::OnFocus, this);
    Bind(wxEVT_SIZE, [this](wxSizeEvent& e) { UpdateVirtualSize(); e.Skip(); });
    for (auto t : {wxEVT_SCROLLWIN_TOP, wxEVT_SCROLLWIN_BOTTOM, wxEVT_SCROLLWIN_LINEUP, wxEVT_SCROLLWIN_LINEDOWN,
                   wxEVT_SCROLLWIN_PAGEUP, wxEVT_SCROLLWIN_PAGEDOWN, wxEVT_SCROLLWIN_THUMBRELEASE}) {
        Bind(t, [this](wxScrollWinEvent& e) { e.Skip(); CallAfter([this] { PlaceCaret(); }); });
    }
    // Dragging the scroll box does not scroll until it is let go (spec 30).
    Bind(wxEVT_SCROLLWIN_THUMBTRACK, [](wxScrollWinEvent&) {});
    // The wheel scrolls one row for each notch (120 units, spec 30).
    Bind(wxEVT_MOUSEWHEEL, [this](wxMouseEvent& e) {
        if (e.GetWheelAxis() != wxMOUSE_WHEEL_VERTICAL) {
            e.Skip();
            return;
        }
        m_wheel += e.GetWheelRotation();
        const int rows = m_wheel / 120;
        m_wheel -= rows * 120;
        if (rows != 0) {
            int vx = 0, vy = 0;
            GetViewStart(&vx, &vy);
            Scroll(vx, wxMax(vy - rows, 0));
            PlaceCaret();
        }
    });
    m_locator.SetOwner(this, 1);
    m_blink.SetOwner(this, 2);
    Bind(wxEVT_TIMER, [this](wxTimerEvent&) {
        m_blinkOn = !m_blinkOn;
        RefreshRect(CaretRect());
    }, 2);
    Bind(wxEVT_TIMER, [this](wxTimerEvent&) {
        // One step: show the rectangle at the current size, or take it away and close in by half a row.
        if (m_locatorShown) {
            m_locatorShown = false;
            m_locatorOut -= wxMax(m_rowHeight / 2, 1);
            if (m_locatorOut < 0) {
                m_locator.Stop();
                m_locatorOut = -1;
            }
        } else {
            m_locatorShown = true;
        }
        Refresh();
    }, 1);
    AccessibleInit();
}

// The caret's rectangle in the window (scrolled): a tenth of the row wide (1 to 5 pixels), the row less a twentieth
// above and below (spec 30).
wxRect DocEditor::CaretRect()
{
    int x = 0, y = 0;
    CalcScrolledPosition(ColumnX(m_caretRow, m_caretCol), m_caretRow * m_rowHeight + m_rowHeight / 20, &x, &y);
    return wxRect(x, y, wxClip(m_rowHeight / 10, 1, 5), wxMax(m_rowHeight - 2 * (m_rowHeight / 20), 1));
}

// The Cursor Locator (spec 24 2d.2): around a square one row high at the caret, rectangles from three rows out closing
// in by half a row a step, each shown and taken away again with a pause of the caret blink time / 28 between; the
// border a fifth of a row wide, in the window text colour. The reference draws it in its editing window.
void DocEditor::ShowCursorLocator()
{
    ShowRow(m_caretRow, -1);
    PlaceCaret();
    int blink = wxCaret::GetBlinkTime();
    if (blink < 0) blink = 1200;
    m_locatorOut = wxMax(m_rowHeight / 2, 1) * 6;
    m_locatorShown = true;
    Refresh();
    m_locator.Start(wxMax(blink / 28, 1));
}

DocEditor::~DocEditor()
{
    AccessibleDone();
    if (m_free) m_free(m_data);
}

bool DocEditor::SetFont(const wxFont& font)
{
    const bool changed = wxScrolledCanvas::SetFont(font);
    wxClientDC dc(this);
    dc.SetFont(GetFont());
    wxCoord w = 0, h = 0, descent = 0, leading = 0;
    dc.GetTextExtent("M", &w, &h, &descent, &leading);
    // A row is the font's ascent, descent and external leading (spec 30).
    m_rowHeight = wxMax(h + leading, 1);
    m_ascent = h - descent;
    m_charWidth = wxMax(w, 1);
    // The code font (spec 27 4a; the reference's font setup makes it from the default font): the box padding is a
    // twentieth of the row; starting at the text font's point size, the size goes down a point at a time, to 4 at
    // the least, until the code font's ascent and descent fit the text font's less the box border (2 x padding + 1)
    // above and below; a code's baseline is then raised by half of what is left, less that border.
    m_codePad = m_rowHeight / 20;
    const int border = 2 * m_codePad + 1;
    wxFont code = wxSystemSettings::GetFont(wxSYS_DEFAULT_GUI_FONT);
    int size = GetFont().GetPointSize();
    wxCoord cw = 0, ch = 0, cd = 0, cl = 0;
    for (;;) {
        code.SetPointSize(wxMax(size, 1));
        dc.SetFont(code);
        dc.GetTextExtent("M", &cw, &ch, &cd, &cl);
        if (ch <= h - 2 * border || size <= 4) break;
        --size;
    }
    m_codeFont = code;
    m_codeAscent = ch - cd;
    m_codeDescent = cd;
    m_codeRaise = (h - ch) / 2 - border;
    // The rows held keep their text; their measurements are made again in the new font.
    for (RowEntry& e : m_cache) e.extents.clear();
    // The caret: a tenth of the row wide (1 to 5 pixels), the row less a twentieth above and below (spec 30).
    if (GetCaret()) GetCaret()->SetSize(wxClip(m_rowHeight / 10, 1, 5), wxMax(m_rowHeight - 2 * (m_rowHeight / 20), 1));
    SetScrollRate(m_charWidth, m_rowHeight);
    UpdateVirtualSize();
    PlaceCaret();
    Refresh();
    return changed;
}

// Takes the code markers (U+E000 before a code, U+E001 after it) out of a row's text, keeping the column ranges they
// marked.
static void SplitCodes(const wxString& in, wxString& out, std::vector<std::pair<int, int>>& codes)
{
    out.clear();
    codes.clear();
    int start = -1;
    int length = 0;
    for (wxUniChar c : in) {
        const wxUint32 v = c.GetValue();
        if (v == 0xE000) {
            start = length;
        } else if (v == 0xE001) {
            if (start >= 0 && length > start) codes.emplace_back(start, length);
            start = -1;
        } else {
            out += c;
            ++length;
        }
    }
}

void DocEditor::SetRows(int count, const unsigned char* kinds, const int* lengths, int longest, int changedAt,
                        const wxString& removed, const wxString& inserted)
{
    m_rowCount = wxMax(count, 0);
    m_kinds.assign(m_rowCount, 0);
    m_rowStart.assign(m_rowCount + 1, 0);
    int at = 0;
    for (int r = 0; r < m_rowCount; ++r) {
        m_kinds[r] = kinds && kinds[r] == 1 ? 1 : 0;
        m_rowStart[r] = at;
        // A text row and its line feed; a page break row holds nothing.
        if (m_kinds[r] == 0) at += (lengths ? wxMax(lengths[r], 0) : 0) + 1;
    }
    m_rowStart[m_rowCount] = at;
    m_longest = wxMax(longest, 0);
    // The rows held are let go: the program gives each again as it is needed.
    for (RowEntry& e : m_cache) e.row = -1;
    for (TextEntry& e : m_accessCache) e.row = -1;
    for (TextEntry& e : m_labelCache) e.row = -1;
    UpdateVirtualSize();
    Refresh();
    if (changedAt >= 0) AccessibleTextChanged(changedAt, removed, inserted);
}

void DocEditor::SetLineNumbers(int width)
{
    m_gutter = width > 0 ? FromDIP(width) : 0;
    UpdateVirtualSize();
    PlaceCaret();
    Refresh();
}

void DocEditor::UpdateVirtualSize()
{
    const int width = m_gutter + m_margin + (m_longest + 2) * m_charWidth;
    SetVirtualSize(width, m_rowCount * m_rowHeight + m_rowHeight);
}

int DocEditor::RowLength(int row) const
{
    if (row < 0 || row >= m_rowCount || m_kinds[row] == 1) return 0;
    return m_rowStart[row + 1] - m_rowStart[row] - 1;
}

// Row `row` as held, asked of the program when the window does not hold it. A row the program cannot give now is
// answered as empty and not kept, so it is asked for again.
DocEditor::RowEntry& DocEditor::Entry(int row)
{
    if (row < 0 || row >= m_rowCount) {
        m_unanswered = RowEntry();
        return m_unanswered;
    }
    RowEntry& e = m_cache[row % kCacheRows];
    if (e.row == row) return e;
    const char* given = m_kinds[row] == 1 ? "" : (m_row ? m_row(m_data, row, 0) : "");
    if (!given) {
        m_unanswered = RowEntry();
        return m_unanswered;
    }
    e.row = row;
    e.extents.clear();
    SplitCodes(wxString::FromUTF8(given), e.text, e.codes);
    return e;
}

const wxString& DocEditor::AccessRowText(int row)
{
    if (row < 0 || row >= m_rowCount || m_kinds[row] == 1) return m_empty;
    TextEntry& e = m_accessCache[row % kCacheRows];
    if (e.row == row) return e.text;
    const char* given = m_row ? m_row(m_data, row, 1) : "";
    if (!given) return m_empty;
    e.row = row;
    e.text = wxString::FromUTF8(given);
    return e.text;
}

const wxString& DocEditor::Label(int row)
{
    if (row < 0 || row >= m_rowCount) return m_empty;
    TextEntry& e = m_labelCache[row % kCacheRows];
    if (e.row == row) return e.text;
    const char* given = m_row ? m_row(m_data, row, 2) : "";
    if (!given) return m_empty;
    e.row = row;
    e.text = wxString::FromUTF8(given);
    return e.text;
}

// The x of each character boundary of row `row` (0 .. length), from the row's start: text measured in the window
// font, a code in the code font with the box padding twice on each side of it (the reference draws a code's text
// 2 x padding in from the box's start and ends the box 2 x padding after it).
const std::vector<int>& DocEditor::Extents(int row)
{
    static const std::vector<int> none{0};
    if (row < 0 || row >= m_rowCount) return none;
    RowEntry& entry = Entry(row);
    auto& e = entry.extents;
    if (e.empty()) {
        wxClientDC dc(this);
        const wxString& text = entry.text;
        const auto& codes = entry.codes;
        const int n = (int)text.length();
        e.push_back(0);
        int x = 0, col = 0;
        size_t ci = 0;
        while (col < n) {
            while (ci < codes.size() && codes[ci].second <= col) ++ci;
            const bool inCode = ci < codes.size() && codes[ci].first <= col;
            int end = inCode ? codes[ci].second : (ci < codes.size() ? codes[ci].first : n);
            end = wxClip(end, col + 1, n);
            dc.SetFont(inCode ? m_codeFont : GetFont());
            wxArrayInt widths;
            if (!dc.GetPartialTextExtents(text.Mid(col, end - col), widths) || (int)widths.size() != end - col) {
                widths.clear();
                for (int k = col; k < end; ++k) widths.push_back((k - col + 1) * m_charWidth);
            }
            const int pad = inCode ? 2 * m_codePad : 0;
            for (size_t k = 0; k < widths.size(); ++k) {
                const bool last = inCode && k + 1 == widths.size();
                e.push_back(x + pad + widths[k] + (last ? pad : 0));
            }
            x = e.back();
            col = end;
        }
    }
    return e;
}

// The x (in the window's virtual area) of column `col` of row `row`; past the row's end, whole characters on.
int DocEditor::ColumnX(int row, int col)
{
    const auto& e = Extents(row);
    const int last = (int)e.size() - 1;
    const int x = col <= last ? e[wxMax(col, 0)] : e[last] + (col - last) * m_charWidth;
    return m_gutter + m_margin + x;
}

// The column at x (in the virtual area) in row `row`: the nearer boundary.
int DocEditor::ColumnAt(int row, int x)
{
    const auto& e = Extents(row);
    x -= m_gutter + m_margin;
    if (x <= 0) return 0;
    const int last = (int)e.size() - 1;
    for (int k = 0; k < last; ++k) {
        if (x < (e[k] + e[k + 1]) / 2) return k;
    }
    if (x <= e[last]) return last;
    return last + (x - e[last] + m_charWidth / 2) / m_charWidth;
}

void DocEditor::SetCaretPlace(int row, int col, int anchorRow, int anchorCol, int keepFromTop, const wxString* caretRowText)
{
    // The caret row's text, given with the caret: the program may be unable to answer for it while the caret is
    // placed, and the caret's x needs the row measured.
    if (caretRowText && row >= 0 && row < m_rowCount) {
        RowEntry& e = m_cache[row % kCacheRows];
        e.row = row;
        e.extents.clear();
        SplitCodes(*caretRowText, e.text, e.codes);
    }
    m_caretRow = row;
    m_caretCol = col;
    m_anchorRow = anchorRow;
    m_anchorCol = anchorCol;
    ShowRow(row, keepFromTop);
    PlaceCaret();
    Refresh();
    AccessibleCaretMoved();
}

// Scrolls so `row` is shown: one row at a time until it is (spec 27 5d), or `keepFromTop` rows from the top.
void DocEditor::ShowRow(int row, int keepFromTop)
{
    int vx = 0, vy = 0;
    GetViewStart(&vx, &vy);
    const int shown = GetVisibleRows();
    int top = vy;
    if (keepFromTop >= 0) {
        top = wxMax(row - keepFromTop, 0);
    } else if (row < top) {
        top = row;
    } else if (row >= top + shown) {
        top = row - shown + 1;
    }
    // Sideways: the caret column in view, five characters at a time.
    const int x = ColumnX(row, m_caretCol) / m_charWidth;
    const int width = wxMax(GetClientSize().GetWidth() / m_charWidth, 1);
    int left = vx;
    if (x < left + 1) {
        left = wxMax(x - 5, 0);
    } else if (x >= left + width - 1) {
        left = x - width + 5;
    }
    if (top != vy || left != vx) Scroll(left, top);
}

void DocEditor::PlaceCaret()
{
#ifdef __WXOSX__
    // The drawn caret: shown at once at its new place, the blink starting again.
    m_blinkOn = true;
    Refresh();
    if (m_hasFocus) {
        int blink = wxCaret::GetBlinkTime();
        m_blink.Start(blink > 0 ? blink : 500);
    }
#endif
    wxCaret* caret = GetCaret();
    if (!caret) return;
    int x = 0, y = 0;
    CalcScrolledPosition(ColumnX(m_caretRow, m_caretCol), m_caretRow * m_rowHeight + m_rowHeight / 20, &x, &y);
    caret->Move(x, y);
}

int DocEditor::GetTopRow() const
{
    int vx = 0, vy = 0;
    GetViewStart(&vx, &vy);
    return vy;
}

int DocEditor::GetVisibleRows() const
{
    return wxMax(GetClientSize().GetHeight() / wxMax(m_rowHeight, 1), 1);
}

void DocEditor::OnPaint(wxPaintEvent&)
{
    wxAutoBufferedPaintDC dc(this);
    DoPrepareDC(dc);
    const wxColour back = wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW);
    const wxColour fore = wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOWTEXT);
    const wxColour selBack = wxSystemSettings::GetColour(wxSYS_COLOUR_HIGHLIGHT);
    const wxColour grey = wxSystemSettings::GetColour(wxSYS_COLOUR_GRAYTEXT);
    dc.SetBackground(wxBrush(back));
    dc.Clear();
    dc.SetFont(GetFont());
    const int top = GetTopRow();
    const int last = wxMin(top + GetVisibleRows() + 1, m_rowCount - 1);
    const int width = GetVirtualSize().GetWidth() + GetClientSize().GetWidth();
    // The selection, from the earlier end to the later one.
    int r1 = m_anchorRow, c1 = m_anchorCol, r2 = m_caretRow, c2 = m_caretCol;
    if (r2 < r1 || (r2 == r1 && c2 < c1)) {
        std::swap(r1, r2);
        std::swap(c1, c2);
    }
    const bool selection = r1 != r2 || c1 != c2;
    for (int row = top; row <= last; ++row) {
        const int y = row * m_rowHeight;
        if (IsBreakRow(row)) {
            // The page break row: one line across the whole width at the text's baseline (spec 30). INFERENCE: its
            // pen is the window text colour, one pixel (spec 30 unknown: the page-break pen).
            dc.SetPen(wxPen(fore, 1));
            dc.DrawLine(0, y + m_ascent, width, y + m_ascent);
            continue;
        }
        const wxString& text = RowText(row);
        int from = 0, to = 0;
        if (selection && row >= r1 && row <= r2) {
            from = row == r1 ? c1 : 0;
            to = row == r2 ? c2 : (int)text.length() + 1;
        }
        if (to > from) {
            const int x1 = ColumnX(row, from), x2 = ColumnX(row, to);
            dc.SetPen(*wxTRANSPARENT_PEN);
            dc.SetBrush(wxBrush(selBack));
            dc.DrawRectangle(x1, y, x2 - x1, m_rowHeight);
        }
        if (!text.empty()) {
            const int n = (int)text.length();
            const int a = wxClip(from, 0, n), b = wxClip(to, 0, n);
            if (a > 0) DrawPart(dc, row, 0, a, false, y);
            if (b > a) DrawPart(dc, row, a, b, true, y);
            if (b < n) DrawPart(dc, row, b, n, false, y);
        }
    }
    if (m_locatorShown && m_locatorOut >= 0) {
        const int h = m_rowHeight;
        const int x = ColumnX(m_caretRow, m_caretCol) - m_locatorOut, y = m_caretRow * h - m_locatorOut;
        const int w = h + 2 * m_locatorOut;
        dc.SetPen(*wxTRANSPARENT_PEN);
        dc.SetBrush(wxBrush(fore));
        const int pen = wxMax(h / 5, 1);
        dc.DrawRectangle(x, y, w, pen);
        dc.DrawRectangle(x, y + w - pen, w, pen);
        dc.DrawRectangle(x, y, pen, w);
        dc.DrawRectangle(x + w - pen, y, pen, w);
    }
#ifdef __WXOSX__
    // The caret, drawn here on macOS while the window has focus, in the window text colour.
    if (m_hasFocus && m_blinkOn && m_caretRow < m_rowCount) {
        dc.SetPen(*wxTRANSPARENT_PEN);
        dc.SetBrush(wxBrush(fore));
        const int w = wxClip(m_rowHeight / 10, 1, 5), h = wxMax(m_rowHeight - 2 * (m_rowHeight / 20), 1);
        dc.DrawRectangle(ColumnX(m_caretRow, m_caretCol), m_caretRow * m_rowHeight + m_rowHeight / 20, w, h);
    }
#endif
    // The line number column, on the left of what is shown, over the text scrolled under it.
    if (m_gutter > 0) {
        int vx = 0, vy = 0;
        CalcUnscrolledPosition(0, 0, &vx, &vy);
        dc.SetPen(*wxTRANSPARENT_PEN);
        dc.SetBrush(wxBrush(back));
        dc.DrawRectangle(vx, vy, m_gutter, GetClientSize().GetHeight() + m_rowHeight);
        dc.SetFont(wxSystemSettings::GetFont(wxSYS_DEFAULT_GUI_FONT));
        dc.SetTextForeground(grey);
        for (int row = top; row <= last; ++row) {
            const wxString& label = Label(row);
            if (label.empty()) continue;
            const wxSize size = dc.GetTextExtent(label);
            dc.DrawText(label, vx + m_gutter - size.GetWidth() - FromDIP(2),
                        row * m_rowHeight + (m_rowHeight - size.GetHeight()) / 2);
        }
    }
}

// Draws columns `from` to `to` of row `row` (top at `y`): text in the window font; each code (spec 27 4a, the
// reference's row drawing and code box) in the code font, raised, inside a rounded box with corners a fifth of the
// row, filled with the button face colour and framed with the window frame colour a padding wide (the highlight
// colours when selected), its text in the button text colour (the window text colour when selected). A code is drawn
// whole with the part its start is in.
void DocEditor::DrawPart(wxDC& dc, int row, int from, int to, bool selected, int y)
{
    const RowEntry& entry = Entry(row);
    const wxString& text = entry.text;
    const auto& codes = entry.codes;
    const wxColour fore = wxSystemSettings::GetColour(selected ? wxSYS_COLOUR_HIGHLIGHTTEXT : wxSYS_COLOUR_WINDOWTEXT);
    const int base = y + m_ascent;
    int col = from;
    size_t ci = 0;
    while (ci < codes.size() && codes[ci].second <= col) ++ci;
    // A code the part starts inside was drawn whole with the part before it.
    if (ci < codes.size() && codes[ci].first < col) {
        col = codes[ci].second;
        ++ci;
    }
    while (col < to) {
        const bool inCode = ci < codes.size() && codes[ci].first <= col;
        if (inCode) {
            const int a = codes[ci].first, b = codes[ci].second;
            const int x1 = ColumnX(row, a), x2 = ColumnX(row, b);
            const int top = base - m_codeAscent - m_codePad - 1, bottom = base + m_codeDescent + m_codePad + 1;
            dc.SetBrush(wxBrush(wxSystemSettings::GetColour(selected ? wxSYS_COLOUR_HIGHLIGHT : wxSYS_COLOUR_BTNFACE)));
            dc.SetPen(wxPen(wxSystemSettings::GetColour(selected ? wxSYS_COLOUR_BTNHIGHLIGHT : wxSYS_COLOUR_WINDOWFRAME),
                            wxMax(m_codePad, 1)));
            dc.DrawRoundedRectangle(x1 + m_codePad, top, wxMax(x2 - x1 - 2 * m_codePad, 1), bottom - top, m_rowHeight / 5.0);
            dc.SetFont(m_codeFont);
            dc.SetTextForeground(wxSystemSettings::GetColour(selected ? wxSYS_COLOUR_WINDOWTEXT : wxSYS_COLOUR_BTNTEXT));
            dc.DrawText(text.Mid(a, b - a), x1 + 2 * m_codePad, base - m_codeRaise - m_codeAscent);
            dc.SetFont(GetFont());
            col = b;
            ++ci;
        } else {
            const int end = ci < codes.size() ? wxMin(codes[ci].first, to) : to;
            dc.SetTextForeground(fore);
            dc.DrawText(text.Mid(col, end - col), ColumnX(row, col), y);
            col = end;
        }
    }
}

void DocEditor::OnKeyDown(wxKeyEvent& e)
{
    if (m_key && m_key(m_data, e.GetKeyCode(), (int)e.GetUnicodeKey(), e.GetModifiers())) return;
    e.Skip();
}

void DocEditor::OnChar(wxKeyEvent& e)
{
    const int c = (int)e.GetUnicodeKey();
    const int mods = e.GetModifiers();
    // Tab and Escape, and keys with Control or with Alt alone, go on: Tab moves the focus, the others belong to the
    // menus (Alt with a letter opens a menu). AltGr (Control and Alt on Windows) types. On macOS Option (Alt) with a
    // key types a character and the menus use Command, so Alt there types.
#ifdef __WXOSX__
    const bool menuKey = (mods & wxMOD_CONTROL) != 0;
#else
    const bool menuKey = ((mods & wxMOD_CONTROL) != 0) != ((mods & wxMOD_ALT) != 0);
#endif
    if (c == WXK_NONE || c == WXK_TAB || c == WXK_ESCAPE || c < 32 || menuKey) {
        if (c == WXK_TAB) {
            Navigate((mods & wxMOD_SHIFT) ? wxNavigationKeyEvent::IsBackward : wxNavigationKeyEvent::IsForward);
            return;
        }
        e.Skip();
        return;
    }
    if (m_char) m_char(m_data, c, mods);
}

void DocEditor::OnMouse(wxMouseEvent& e)
{
    int x = 0, y = 0;
    CalcUnscrolledPosition(e.GetX(), e.GetY(), &x, &y);
    const int row = wxMax(y / m_rowHeight, 0);
    const int col = ColumnAt(wxMin(row, m_rowCount - 1), x);
    int kind = -1;
    if (e.GetEventType() == wxEVT_LEFT_DOWN) {
        SetFocus();
        if (!HasCapture()) CaptureMouse();
        kind = 0;
    } else if (e.GetEventType() == wxEVT_LEFT_DCLICK) {
        kind = 3;
    } else if (e.GetEventType() == wxEVT_LEFT_UP) {
        if (HasCapture()) ReleaseMouse();
        kind = 2;
    } else if (e.GetEventType() == wxEVT_MOTION && e.LeftIsDown() && HasCapture()) {
        kind = 1;
    }
    if (kind >= 0 && m_mouse) m_mouse(m_data, kind, row, col, e.GetModifiers());
    e.Skip(kind == 0);
}

void DocEditor::OnFocus(wxFocusEvent& e)
{
    const bool got = e.GetEventType() == wxEVT_SET_FOCUS;
    m_hasFocus = got;
    if (!got) {
        m_blink.Stop();
        Refresh();
    }
    if (got) PlaceCaret();
    if (m_focus) m_focus(m_data, got);
    e.Skip();
}

wxRect DocEditor::CaretRectOnScreen()
{
    int x = 0, y = 0;
    CalcScrolledPosition(ColumnX(m_caretRow, m_caretCol), m_caretRow * m_rowHeight, &x, &y);
    const wxPoint p = ClientToScreen(wxPoint(x, y));
    return wxRect(p.x, p.y, wxMax(FromDIP(2), 1), m_rowHeight);
}

// The text screen readers get: each text row followed by a line feed; page break rows are not in it. Its length is
// known from the row lengths; a part of it is put together from the rows it covers.
int DocEditor::AccessibleLength() const
{
    return m_rowStart.empty() ? 0 : m_rowStart[m_rowCount];
}

int DocEditor::RowOfOffset(int offset) const
{
    if (m_rowCount <= 0) return 0;
    offset = wxClip(offset, 0, AccessibleLength());
    // The last row starting at or before the offset; among rows starting at the same place (page break rows start
    // where the row after them does) the later one, which is the text row.
    int r = (int)(std::upper_bound(m_rowStart.begin(), m_rowStart.begin() + m_rowCount, offset) - m_rowStart.begin()) - 1;
    r = wxClip(r, 0, m_rowCount - 1);
    while (r > 0 && m_kinds[r] == 1) --r;
    return r;
}

wxString DocEditor::AccessibleText(int start, int end)
{
    const int n = AccessibleLength();
    start = wxClip(start, 0, n);
    end = wxClip(end, start, n);
    wxString out;
    int at = start;
    int row = RowOfOffset(start);
    while (at < end && row < m_rowCount) {
        if (m_kinds[row] == 1) {
            ++row;
            continue;
        }
        const int len = RowLength(row);
        const int from = at - m_rowStart[row];
        const int take = wxMin(len + 1 - from, end - at);
        if (from < len) {
            const wxString& text = AccessRowText(row);
            out += text.Mid(from, wxMin(take, len - from));
        }
        if (from + take > len) out += '\n';
        at += take;
        ++row;
    }
    return out;
}

wxUniChar DocEditor::AccessibleChar(int offset)
{
    if (offset < 0 || offset >= AccessibleLength()) return wxUniChar(0);
    const int row = RowOfOffset(offset);
    const int col = offset - m_rowStart[row];
    if (col >= RowLength(row)) return wxUniChar('\n');
    const wxString& text = AccessRowText(row);
    return col < (int)text.length() ? text[col] : wxUniChar(' ');
}

int DocEditor::LineOfOffset(int offset) const
{
    const int row = RowOfOffset(offset);
    int line = 0;
    for (int r = 0; r < row; ++r) {
        if (m_kinds[r] == 0) ++line;
    }
    return line;
}

int DocEditor::RowOfLine(int line) const
{
    int seen = 0;
    for (int r = 0; r < m_rowCount; ++r) {
        if (m_kinds[r] == 1) continue;
        if (seen == line) return r;
        ++seen;
    }
    return m_rowCount;
}

// The offset in the accessible text of row `row`, column `col` (a column past the row's end counts as its end).
int DocEditor::OffsetOf(int row, int col)
{
    if (row < 0) return 0;
    if (row >= m_rowCount) return AccessibleLength();
    return m_rowStart[row] + wxClip(col, 0, RowLength(row));
}

// The row and column of an offset in the accessible text.
void DocEditor::PlaceOf(int offset, int* row, int* col)
{
    const int r = RowOfOffset(offset);
    *row = r;
    *col = wxClip(offset - m_rowStart[r], 0, RowLength(r));
}

// The window rectangle (client coordinates) of row `row` from column `from` to column `to`.
wxRect DocEditor::RowRect(int row, int from, int to)
{
    int x1 = 0, y = 0, x2 = 0, y2 = 0;
    CalcScrolledPosition(ColumnX(row, from), row * m_rowHeight, &x1, &y);
    CalcScrolledPosition(ColumnX(row, to), row * m_rowHeight, &x2, &y2);
    return wxRect(x1, y, wxMax(x2 - x1, 1), m_rowHeight);
}

// ---- C interface ----

static DocEditor* Ed(wxd_Window_t* w)
{
    return dynamic_cast<DocEditor*>(reinterpret_cast<wxWindow*>(w));
}

wxd_Window_t* wxd_DocEditor_Create(wxd_Window_t* parent, void* data, wxd_DocEditor_KeyFn key,
                                   wxd_DocEditor_CharFn chr, wxd_DocEditor_MouseFn mouse,
                                   wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data,
                                   wxd_DocEditor_RowFn row)
{
    wxWindow* p = reinterpret_cast<wxWindow*>(parent);
    if (!p) return nullptr;
    return reinterpret_cast<wxd_Window_t*>(new DocEditor(p, data, key, chr, mouse, focus, free_data, row));
}

void wxd_DocEditor_SetRows(wxd_Window_t* editor, int count, const unsigned char* kinds, const int* lengths,
                           int longest, int changed_at, const char* removed, const char* inserted)
{
    DocEditor* e = Ed(editor);
    if (!e) return;
    e->SetRows(count, kinds, lengths, longest, changed_at, wxString::FromUTF8(removed ? removed : ""),
               wxString::FromUTF8(inserted ? inserted : ""));
}

void wxd_DocEditor_SetLineNumbers(wxd_Window_t* editor, int width)
{
    if (DocEditor* e = Ed(editor)) e->SetLineNumbers(width);
}

void wxd_DocEditor_SetCaret(wxd_Window_t* editor, int row, int col, int anchor_row, int anchor_col, int keep_from_top,
                            const char* caret_row_text)
{
    DocEditor* e = Ed(editor);
    if (!e) return;
    if (caret_row_text) {
        const wxString text = wxString::FromUTF8(caret_row_text);
        e->SetCaretPlace(row, col, anchor_row, anchor_col, keep_from_top, &text);
    } else {
        e->SetCaretPlace(row, col, anchor_row, anchor_col, keep_from_top, nullptr);
    }
}

int wxd_DocEditor_GetTopRow(wxd_Window_t* editor)
{
    DocEditor* e = Ed(editor);
    return e ? e->GetTopRow() : 0;
}

int wxd_DocEditor_GetVisibleRows(wxd_Window_t* editor)
{
    DocEditor* e = Ed(editor);
    return e ? e->GetVisibleRows() : 1;
}

void wxd_DocEditor_SetTopRow(wxd_Window_t* editor, int row)
{
    DocEditor* e = Ed(editor);
    if (!e) return;
    int vx = 0, vy = 0;
    e->GetViewStart(&vx, &vy);
    e->Scroll(vx, wxMax(row, 0));
    e->PlaceCaret();
}

int wxd_DocEditor_GetCaretScreenRow(wxd_Window_t* editor)
{
    DocEditor* e = Ed(editor);
    return e ? e->CaretRow() - e->GetTopRow() : 0;
}

void wxd_DocEditor_GetCaretRect(wxd_Window_t* editor, int* x, int* y, int* w, int* h)
{
    DocEditor* e = Ed(editor);
    const wxRect r = e ? e->CaretRectOnScreen() : wxRect();
    if (x) *x = r.x;
    if (y) *y = r.y;
    if (w) *w = r.width;
    if (h) *h = r.height;
}

void wxd_DocEditor_ShowCursorLocator(wxd_Window_t* editor)
{
    if (DocEditor* e = Ed(editor)) e->ShowCursorLocator();
}

void wxd_DocEditor_SetAccessibleName(wxd_Window_t* editor, const char* name)
{
    if (DocEditor* e = Ed(editor)) {
        e->SetName(wxString::FromUTF8(name ? name : ""));
        e->AccessibleNameChanged();
    }
}

// Systems with no accessibility layer of their own here: nothing to tell.
// Other systems: nothing to tell (Linux: doc_editor_gtk.cpp; Windows: doc_editor_msw.cpp; macOS: doc_editor_osx.mm).
#if !defined(__WXGTK__) && !defined(__WXMSW__) && !defined(__WXOSX__)
void DocEditor::AccessibleInit() {}
void DocEditor::AccessibleDone() {}
void DocEditor::AccessibleTextChanged(int, const wxString&, const wxString&) {}
void DocEditor::AccessibleCaretMoved() {}
void DocEditor::AccessibleNameChanged() {}
#endif
