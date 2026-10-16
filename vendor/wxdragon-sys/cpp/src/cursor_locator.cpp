// Added for BrailleCraft: the Cursor Locator, a rectangle that closes in on the caret of a text control so the eye
// finds it.
//
// Around a square one line high at the caret, rectangles are drawn from four line heights out, closing in by half a
// line height a step down to one line height; each is shown and taken away again, with a pause of the caret blink
// time / 28 between, and its border is a fifth of a line height wide. The rectangle is made of four thin borderless
// popup windows in the window text colour, so it is drawn over the system text control on every system. A timer
// steps it, so the program keeps drawing and taking input while it runs.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/popupwin.h>
#include <wx/caret.h>
#include <wx/settings.h>
#include <wx/timer.h>
#include "../include/wxdragon.h"

namespace {

class CursorLocator : public wxTimer {
public:
    CursorLocator(wxWindow* top, wxPoint at, int h, wxColour colour)
        : m_at(at), m_h(h), m_pen(wxMax(h / 5, 1)), m_half(wxMax(h / 2, 1)), m_out(m_half * 6 + h) {
        for (auto& s : m_sides) {
            s = new wxPopupWindow(top, wxBORDER_NONE);
            s->SetBackgroundColour(colour);
        }
    }

    // One step: show the rectangle at the current size, or take it away and close in.
    void Notify() override {
        if (m_shown) {
            for (auto s : m_sides)
                s->Hide();
            m_shown = false;
            m_out -= m_half;
            if (m_out < m_h) {
                Stop();
                for (auto s : m_sides)
                    s->Destroy();
                delete this;
            }
            return;
        }
        const int x = m_at.x - m_out, y = m_at.y - m_out, w = m_h + 2 * m_out;
        m_sides[0]->SetSize(x, y, w, m_pen);
        m_sides[1]->SetSize(x, y + w - m_pen, w, m_pen);
        m_sides[2]->SetSize(x, y, m_pen, w);
        m_sides[3]->SetSize(x + w - m_pen, y, m_pen, w);
        for (auto s : m_sides)
            s->Show();
        m_shown = true;
    }

private:
    wxPoint m_at;
    int m_h, m_pen, m_half, m_out;
    bool m_shown = false;
    wxPopupWindow* m_sides[4];
};

} // namespace

extern "C" void wxd_TextCtrl_ShowCursorLocator(wxd_Window_t* window) {
    wxTextCtrl* text = wxDynamicCast(reinterpret_cast<wxWindow*>(window), wxTextCtrl);
    if (!text || !text->IsShownOnScreen())
        return;
    text->ShowPosition(text->GetInsertionPoint());
    text->Update();
    const wxPoint at = text->PositionToCoords(text->GetInsertionPoint());
    if (at == wxDefaultPosition)
        return;
    int blink = wxCaret::GetBlinkTime();
    if (blink < 0)
        blink = 1200;
    auto* locator = new CursorLocator(wxGetTopLevelParent(text), text->ClientToScreen(at), wxMax(text->GetCharHeight(), 1),
                                      wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOWTEXT));
    locator->Notify();
    locator->Start(wxMax(blink / 28, 1));
}
