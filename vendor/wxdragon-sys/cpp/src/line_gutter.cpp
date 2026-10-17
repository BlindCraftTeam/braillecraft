// Added for BrailleCraft: line numbers drawn beside a text control (wxd_line_gutter.h).
//
// The column is a plain window in the parent of the text control. It asks the control where each labelled position
// is drawn (PositionToCoords, in the control's own coordinates) and paints the label right-aligned at that height.
// A timer checks a few times a second whether the control has scrolled or changed size, and repaints then; the
// system text control sends no scroll event on every platform.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/textctrl.h>
#include <wx/timer.h>
#include <vector>
#include "../include/wxdragon.h"

namespace {

class LineGutter : public wxWindow {
public:
    LineGutter(wxWindow* parent, wxTextCtrl* text, int width)
        : wxWindow(parent, wxID_ANY, wxDefaultPosition, wxSize(width, -1), wxBORDER_NONE | wxFULL_REPAINT_ON_RESIZE),
          m_text(text), m_timer(this) {
        SetBackgroundStyle(wxBG_STYLE_PAINT);
        SetMinSize(wxSize(width, -1));
        Bind(wxEVT_PAINT, &LineGutter::OnPaint, this);
        Bind(wxEVT_TIMER, &LineGutter::OnTimer, this);
        m_timer.Start(150);
    }

    void SetMarks(std::vector<std::pair<long, wxString>> marks) {
        m_marks = std::move(marks);
        Refresh();
    }

private:
    // The text control's first drawn position and its size: when either changes, the numbers move.
    wxPoint Where() const {
        if (!m_text) return wxDefaultPosition;
        long pos = 0;
        m_text->HitTest(wxPoint(1, 1), &pos);
        return wxPoint((int)pos, m_text->GetClientSize().GetHeight());
    }

    void OnTimer(wxTimerEvent&) {
        if (!IsShownOnScreen()) return;
        const wxPoint now = Where();
        if (now != m_last) {
            m_last = now;
            Refresh();
        }
    }

    void OnPaint(wxPaintEvent&) {
        wxPaintDC dc(this);
        dc.SetBackground(wxBrush(wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW)));
        dc.Clear();
        if (!m_text) return;
        // The system's interface font: the text control's font may be a braille font, which shows digits as cells.
        dc.SetFont(wxSystemSettings::GetFont(wxSYS_DEFAULT_GUI_FONT));
        dc.SetTextForeground(wxSystemSettings::GetColour(wxSYS_COLOUR_GRAYTEXT));
        // The control's top inside the parent against the column's own top.
        const int shift = m_text->GetPosition().y - GetPosition().y;
        const int height = GetClientSize().GetHeight();
        const int width = GetClientSize().GetWidth();
        for (const auto& m : m_marks) {
            const wxPoint at = m_text->PositionToCoords(m.first);
            if (at == wxDefaultPosition) continue;
            const int y = at.y + shift;
            if (y < -50 || y > height) continue;
            const wxSize size = dc.GetTextExtent(m.second);
            dc.DrawText(m.second, width - size.GetWidth() - 2, y);
        }
    }

    wxTextCtrl* m_text;
    wxTimer m_timer;
    wxPoint m_last;
    std::vector<std::pair<long, wxString>> m_marks;
};

} // namespace

wxd_Window_t* wxd_LineGutter_Create(wxd_Window_t* parent, wxd_Window_t* text, int width)
{
    wxWindow* p = reinterpret_cast<wxWindow*>(parent);
    wxTextCtrl* t = wxDynamicCast(reinterpret_cast<wxWindow*>(text), wxTextCtrl);
    if (!p || !t) return nullptr;
    return reinterpret_cast<wxd_Window_t*>(new LineGutter(p, t, width));
}

void wxd_LineGutter_SetMarks(wxd_Window_t* gutter, const long* positions, const char* const* labels, int count)
{
    LineGutter* g = dynamic_cast<LineGutter*>(reinterpret_cast<wxWindow*>(gutter));
    if (!g) return;
    std::vector<std::pair<long, wxString>> marks;
    for (int i = 0; i < count; ++i) {
        marks.emplace_back(positions[i], wxString::FromUTF8(labels[i] ? labels[i] : ""));
    }
    g->SetMarks(std::move(marks));
}
