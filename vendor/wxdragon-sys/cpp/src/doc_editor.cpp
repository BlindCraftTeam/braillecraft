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
#include <vector>
#include "../include/wxdragon.h"
#include "doc_editor_internal.h"

DocEditor::DocEditor(wxWindow* parent, void* data, wxd_DocEditor_KeyFn key, wxd_DocEditor_CharFn chr,
                     wxd_DocEditor_MouseFn mouse, wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data)
    : wxScrolledCanvas(parent, wxID_ANY, wxDefaultPosition, wxDefaultSize,
                       wxWANTS_CHARS | wxVSCROLL | wxHSCROLL | wxBORDER_THEME | wxFULL_REPAINT_ON_RESIZE),
      m_data(data), m_key(key), m_char(chr), m_mouse(mouse), m_focus(focus), m_free(free_data)
{
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
    // The caret: a tenth of the row wide (1 to 5 pixels), the row less a twentieth above and below (spec 30).
    if (GetCaret()) GetCaret()->SetSize(wxClip(m_rowHeight / 10, 1, 5), wxMax(m_rowHeight - 2 * (m_rowHeight / 20), 1));
    SetScrollRate(m_charWidth, m_rowHeight);
    UpdateVirtualSize();
    PlaceCaret();
    Refresh();
    return changed;
}

void DocEditor::SetRows(std::vector<wxString> rows, std::vector<int> kinds, int changedFrom)
{
    m_rows = std::move(rows);
    m_kinds = std::move(kinds);
    m_kinds.resize(m_rows.size(), 0);
    m_extents.clear();
    m_extents.resize(m_rows.size());
    m_accessibleText.clear();
    m_accessibleValid = false;
    UpdateVirtualSize();
    Refresh();
    if (changedFrom >= 0) AccessibleTextChanged(changedFrom);
}

void DocEditor::SetLineNumbers(std::vector<wxString> labels, int width)
{
    m_labels = std::move(labels);
    m_gutter = width > 0 ? FromDIP(width) : 0;
    UpdateVirtualSize();
    PlaceCaret();
    Refresh();
}

void DocEditor::UpdateVirtualSize()
{
    size_t longest = 0;
    for (const auto& r : m_rows) longest = wxMax(longest, r.length());
    const int width = m_gutter + m_margin + (int)(longest + 2) * m_charWidth;
    SetVirtualSize(width, (int)m_rows.size() * m_rowHeight + m_rowHeight);
}

// The x of each character boundary of row `row` (0 .. length), from the row's start.
const std::vector<int>& DocEditor::Extents(int row)
{
    static const std::vector<int> none{0};
    if (row < 0 || row >= (int)m_rows.size()) return none;
    auto& e = m_extents[row];
    if (e.empty()) {
        wxClientDC dc(this);
        dc.SetFont(GetFont());
        wxArrayInt widths;
        e.push_back(0);
        if (!m_rows[row].empty() && dc.GetPartialTextExtents(m_rows[row], widths)) {
            for (int w : widths) e.push_back(w);
        } else {
            for (size_t k = 0; k < m_rows[row].length(); ++k) e.push_back((int)(k + 1) * m_charWidth);
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

void DocEditor::SetCaretPlace(int row, int col, int anchorRow, int anchorCol, int keepFromTop)
{
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
    const wxColour selFore = wxSystemSettings::GetColour(wxSYS_COLOUR_HIGHLIGHTTEXT);
    const wxColour grey = wxSystemSettings::GetColour(wxSYS_COLOUR_GRAYTEXT);
    dc.SetBackground(wxBrush(back));
    dc.Clear();
    dc.SetFont(GetFont());
    const int top = GetTopRow();
    const int last = wxMin(top + GetVisibleRows() + 1, (int)m_rows.size() - 1);
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
        if (m_kinds[row] == 1) {
            // The page break row: one line across the whole width at the text's baseline (spec 30). INFERENCE: its
            // pen is the window text colour, one pixel (spec 30 unknown: the page-break pen).
            dc.SetPen(wxPen(fore, 1));
            dc.DrawLine(0, y + m_ascent, width, y + m_ascent);
            continue;
        }
        const wxString& text = m_rows[row];
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
            dc.SetTextForeground(fore);
            if (a > 0) dc.DrawText(text.Mid(0, a), ColumnX(row, 0), y);
            if (b > a) {
                dc.SetTextForeground(selFore);
                dc.DrawText(text.Mid(a, b - a), ColumnX(row, a), y);
                dc.SetTextForeground(fore);
            }
            if (b < n) dc.DrawText(text.Mid(b), ColumnX(row, b), y);
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
    if (m_hasFocus && m_blinkOn && m_caretRow < (int)m_rows.size()) {
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
        for (int row = top; row <= last && row < (int)m_labels.size(); ++row) {
            if (m_labels[row].empty()) continue;
            const wxSize size = dc.GetTextExtent(m_labels[row]);
            dc.DrawText(m_labels[row], vx + m_gutter - size.GetWidth() - FromDIP(2),
                        row * m_rowHeight + (m_rowHeight - size.GetHeight()) / 2);
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
    const int col = ColumnAt(wxMin(row, (int)m_rows.size() - 1), x);
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

// The text screen readers get: each text row followed by a line feed; page break rows are not in it.
const wxString& DocEditor::AccessibleText()
{
    if (!m_accessibleValid) {
        m_accessibleText.clear();
        m_rowStart.assign(m_rows.size() + 1, 0);
        size_t at = 0;
        for (size_t r = 0; r < m_rows.size(); ++r) {
            m_rowStart[r] = (int)at;
            if (m_kinds[r] == 1) continue;
            m_accessibleText += m_rows[r];
            m_accessibleText += '\n';
            at += m_rows[r].length() + 1;
        }
        m_rowStart[m_rows.size()] = (int)at;
        m_accessibleValid = true;
    }
    return m_accessibleText;
}

// The offset in the accessible text of row `row`, column `col` (a column past the row's end counts as its end).
int DocEditor::OffsetOf(int row, int col)
{
    AccessibleText();
    if (row < 0) return 0;
    if (row >= (int)m_rows.size()) return (int)m_accessibleText.length();
    const int len = m_kinds[row] == 1 ? 0 : (int)m_rows[row].length();
    return m_rowStart[row] + wxClip(col, 0, len);
}

// The row and column of an offset in the accessible text.
void DocEditor::PlaceOf(int offset, int* row, int* col)
{
    AccessibleText();
    int r = 0;
    for (size_t k = 0; k < m_rows.size(); ++k) {
        if (m_kinds[k] == 1) continue;
        if (m_rowStart[k] <= offset) r = (int)k;
        else break;
    }
    *row = r;
    *col = wxMax(offset - m_rowStart[r], 0);
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
                                   wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data)
{
    wxWindow* p = reinterpret_cast<wxWindow*>(parent);
    if (!p) return nullptr;
    return reinterpret_cast<wxd_Window_t*>(new DocEditor(p, data, key, chr, mouse, focus, free_data));
}

void wxd_DocEditor_SetRows(wxd_Window_t* editor, const char* const* rows, const int* kinds, int count, int changed_from)
{
    DocEditor* e = Ed(editor);
    if (!e) return;
    std::vector<wxString> r;
    std::vector<int> k;
    r.reserve(count);
    k.reserve(count);
    for (int i = 0; i < count; ++i) {
        r.push_back(wxString::FromUTF8(rows[i] ? rows[i] : ""));
        k.push_back(kinds ? kinds[i] : 0);
    }
    e->SetRows(std::move(r), std::move(k), changed_from);
}

void wxd_DocEditor_SetLineNumbers(wxd_Window_t* editor, const char* const* labels, int count, int width)
{
    DocEditor* e = Ed(editor);
    if (!e) return;
    std::vector<wxString> l;
    for (int i = 0; i < count; ++i) l.push_back(wxString::FromUTF8(labels && labels[i] ? labels[i] : ""));
    e->SetLineNumbers(std::move(l), width);
}

void wxd_DocEditor_SetCaret(wxd_Window_t* editor, int row, int col, int anchor_row, int anchor_col, int keep_from_top)
{
    if (DocEditor* e = Ed(editor)) e->SetCaretPlace(row, col, anchor_row, anchor_col, keep_from_top);
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
void DocEditor::AccessibleTextChanged(int) {}
void DocEditor::AccessibleCaretMoved() {}
void DocEditor::AccessibleNameChanged() {}
#endif
