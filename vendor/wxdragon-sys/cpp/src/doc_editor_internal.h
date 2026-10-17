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
              wxd_DocEditor_MouseFn mouse, wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data);
    ~DocEditor() override;

    bool SetFont(const wxFont& font) override;
    bool AcceptsFocus() const override { return true; }
    bool AcceptsFocusFromKeyboard() const override { return true; }

    void SetRows(std::vector<wxString> rows, std::vector<int> kinds, int changedFrom);
    void SetLineNumbers(std::vector<wxString> labels, int width);
    void SetCaretPlace(int row, int col, int anchorRow, int anchorCol, int keepFromTop);
    void PlaceCaret();
    int GetTopRow() const;
    int GetVisibleRows() const;
    int CaretRow() const { return m_caretRow; }
    int CaretCol() const { return m_caretCol; }
    int AnchorRow() const { return m_anchorRow; }
    int AnchorCol() const { return m_anchorCol; }
    int RowCount() const { return (int)m_rows.size(); }
    int RowHeight() const { return m_rowHeight; }
    const wxString& RowText(int row) const { return m_rows[row]; }
    bool IsBreakRow(int row) const { return row >= 0 && row < (int)m_kinds.size() && m_kinds[row] == 1; }
    wxRect CaretRectOnScreen();

    // The accessible text and the maps between its offsets and rows and columns.
    const wxString& AccessibleText();
    int OffsetOf(int row, int col);
    void PlaceOf(int offset, int* row, int* col);
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

    // Per system (doc_editor_gtk.cpp, doc_editor_msw.cpp, doc_editor_osx.mm).
    void AccessibleInit();
    void AccessibleDone();
    void AccessibleTextChanged(int fromRow);
    void AccessibleCaretMoved();
    void AccessibleNameChanged();
    void* m_accessible = nullptr;
    // What screen readers were last told: the text, the caret and the selection's ends (offsets in the text).
    wxString m_accessiblePrevious;
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
    void UpdateVirtualSize();
    const std::vector<int>& Extents(int row);
    void ShowRow(int row, int keepFromTop);
    void OnPaint(wxPaintEvent& e);
    void OnKeyDown(wxKeyEvent& e);
    void OnChar(wxKeyEvent& e);
    void OnMouse(wxMouseEvent& e);
    void OnFocus(wxFocusEvent& e);

    void* m_data;
    wxd_DocEditor_KeyFn m_key;
    wxd_DocEditor_CharFn m_char;
    wxd_DocEditor_MouseFn m_mouse;
    wxd_DocEditor_FocusFn m_focus;
    wxd_DocEditor_FreeFn m_free;

    std::vector<wxString> m_rows;
    std::vector<int> m_kinds;
    std::vector<std::vector<int>> m_extents;
    std::vector<wxString> m_labels;
    int m_gutter = 0;
    int m_margin = 4;
    int m_rowHeight = 16;
    int m_ascent = 12;
    int m_wheel = 0;
    int m_charWidth = 8;
    int m_caretRow = 0, m_caretCol = 0, m_anchorRow = 0, m_anchorCol = 0;

    wxString m_accessibleText;
    std::vector<int> m_rowStart;
    bool m_accessibleValid = false;

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
