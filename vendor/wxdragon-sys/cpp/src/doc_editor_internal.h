// Added for BrailleCraft: the drawn document editing window (wxd_doc_editor.h), shared by doc_editor.cpp and the
// accessibility layer of each system (doc_editor_gtk.cpp, doc_editor_msw.cpp, doc_editor_osx.mm).
#ifndef BC_DOC_EDITOR_INTERNAL_H
#define BC_DOC_EDITOR_INTERNAL_H

#include <wx/scrolwin.h>
#include <wx/timer.h>
#include <vector>
#include "../include/wxdragon.h"

class DocEditor : public wxScrolledCanvas {
public:
    DocEditor(wxWindow* parent, void* data, wxd_DocEditor_KeyFn key, wxd_DocEditor_CharFn chr,
              wxd_DocEditor_MouseFn mouse, wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data,
              wxd_DocEditor_RowFn row);
    ~DocEditor() override;

    bool SetFont(const wxFont& font) override;
    bool AcceptsFocus() const override { return true; }
    bool AcceptsFocusFromKeyboard() const override { return true; }

    void SetRows(int count, const unsigned char* kinds, const int* lengths, int longest, int changedAt,
                 const wxString& removed, const wxString& inserted);
    void SetLineNumbers(int width);
    void SetCaretPlace(int row, int col, int anchorRow, int anchorCol, int keepFromTop, const wxString* caretRowText);
    void PlaceCaret();
    int GetTopRow() const;
    int GetVisibleRows() const;
    int CaretRow() const { return m_caretRow; }
    int CaretCol() const { return m_caretCol; }
    int AnchorRow() const { return m_anchorRow; }
    int AnchorCol() const { return m_anchorCol; }
    int RowCount() const { return m_rowCount; }
    int RowHeight() const { return m_rowHeight; }
    bool IsBreakRow(int row) const { return row >= 0 && row < m_rowCount && m_kinds[row] == 1; }
    // The length in characters of row `row`'s text (0 for a page break row), known without the text.
    int RowLength(int row) const;
    // Row `row`'s text as drawn (codes without their markers), asked of the program when the window does not hold it.
    const wxString& RowText(int row) { return Entry(row).text; }
    wxRect CaretRectOnScreen();

    // The text screen readers get (each text row and a line feed; page break rows not in it), by parts, and the
    // maps between its offsets and rows and columns. The whole is never held: a part is put together from the rows
    // it covers when it is asked for.
    int AccessibleLength() const;
    wxString AccessibleText(int start, int end);
    wxUniChar AccessibleChar(int offset);
    // Screen readers' text of row `row` (empty for a page break row).
    const wxString& AccessRowText(int row);
    int OffsetOf(int row, int col);
    void PlaceOf(int offset, int* row, int* col);
    // The row holding offset `offset` of the accessible text.
    int RowOfOffset(int offset) const;
    // The line (0-based, text rows only) holding offset `offset`, and the row of line `line`.
    int LineOfOffset(int offset) const;
    int RowOfLine(int line) const;
    wxRect RowRect(int row, int from, int to);
    int ColumnAt(int row, int x);
    int ColumnX(int row, int col);
    // Hands a typed character on as the keyboard does (screen readers that type through the accessible text).
    void TypeText(const wxString& text)
    {
        for (wxUniChar c : text) {
            if (m_char) m_char(m_data, (int)c.GetValue(), 0);
        }
    }

    // Per system (doc_editor_gtk.cpp, doc_editor_msw.cpp, doc_editor_osx.mm). The text changed at offset `at` of
    // the accessible text: `removed` went out there and `inserted` came in.
    void AccessibleInit();
    void AccessibleDone();
    void AccessibleTextChanged(int at, const wxString& removed, const wxString& inserted);
    void AccessibleCaretMoved();
    void AccessibleNameChanged();
    void* m_accessible = nullptr;
    // What screen readers were last told: the caret and the selection's ends (offsets in the text).
    int m_accessibleCaret = -1;
    int m_accessibleAnchor = -1;
    int m_accessibleCaretSelection = -1;

    // A screen reader moving the caret or selecting through the window's text does it as the mouse would: kind 0
    // puts the caret at `row`, `col`, 1 extends the selection to it, 2 ends.
    void Click(int kind, int row, int col)
    {
        if (m_mouse) m_mouse(m_data, kind, row, col, 0);
    }

#ifdef __WXMSW__
    // WM_GETOBJECT for UI Automation (doc_editor_msw.cpp).
    WXLRESULT MSWWindowProc(WXUINT message, WXWPARAM wParam, WXLPARAM lParam) override;
#endif

private:
    // A row the window holds: its text as drawn, the column ranges of its codes, and the x of each character
    // boundary once measured. The window holds a bounded number of rows (each in the slot its number maps to).
    struct RowEntry {
        int row = -1;
        wxString text;
        std::vector<std::pair<int, int>> codes;
        std::vector<int> extents;
    };
    struct TextEntry {
        int row = -1;
        wxString text;
    };
    RowEntry& Entry(int row);
    const wxString& Label(int row);
    void UpdateVirtualSize();
    const std::vector<int>& Extents(int row);
    void ShowRow(int row, int keepFromTop);
    void OnPaint(wxPaintEvent& e);
    void OnKeyDown(wxKeyEvent& e);
    void OnChar(wxKeyEvent& e);
    void OnMouse(wxMouseEvent& e);
    void OnFocus(wxFocusEvent& e);

    void DrawPart(wxDC& dc, int row, int from, int to, bool selected, int y);

    void* m_data;
    wxd_DocEditor_KeyFn m_key;
    wxd_DocEditor_CharFn m_char;
    wxd_DocEditor_MouseFn m_mouse;
    wxd_DocEditor_FocusFn m_focus;
    wxd_DocEditor_FreeFn m_free;

    wxd_DocEditor_RowFn m_row;

    int m_rowCount = 0;
    // Per row: its kind (0 text, 1 page break), and the offset of its start in the accessible text (one more entry,
    // the text's length; a page break row starts where the next row does).
    std::vector<unsigned char> m_kinds;
    std::vector<int> m_rowStart;
    int m_longest = 0;
    // The rows held: as drawn, as screen readers get them, and their line number labels.
    std::vector<RowEntry> m_cache;
    std::vector<TextEntry> m_accessCache;
    std::vector<TextEntry> m_labelCache;
    // A row the program could not give now: used once, not kept.
    RowEntry m_unanswered;
    wxString m_empty;
    int m_gutter = 0;
    int m_margin = 4;
    int m_rowHeight = 16;
    int m_ascent = 12;
    int m_wheel = 0;
    int m_charWidth = 8;
    // The code font (spec 27 4a): the system default font at the text font's point size, made smaller until its
    // ascent and descent fit the text's less the box border above and below; its ascent and descent; the box padding
    // (a twentieth of the row); and how far above the text's baseline a code's baseline sits.
    wxFont m_codeFont;
    int m_codeAscent = 10;
    int m_codeDescent = 2;
    int m_codePad = 0;
    int m_codeRaise = 0;
    int m_caretRow = 0, m_caretCol = 0, m_anchorRow = 0, m_anchorCol = 0;

    // The Cursor Locator (spec 24 2d.2): how far out the rectangle is now, and whether it is drawn.
    wxTimer m_locator;
    int m_locatorOut = -1;
    bool m_locatorShown = false;

    // macOS: the caret is drawn by the window itself, blinking while it has focus (the system caret wxWidgets gives
    // there is drawn outside painting, which macOS does not show).
    wxTimer m_blink;
    bool m_blinkOn = true;
    bool m_hasFocus = false;
    wxRect CaretRect();

public:
    void ShowCursorLocator();
};

#endif // BC_DOC_EDITOR_INTERNAL_H
