// Added for BrailleCraft: the tab bar of the main window (wxd_doc_tabs.h).
//
// A wxNotebook, so the system's own tab control carries the tabs and screen readers read them as a tab list. What is
// added: a close button in each tab (GTK: a real button packed into the tab's label box; Windows: the tabs are drawn
// by the program, TCS_OWNERDRAWFIXED, with a close sign at the right of the text that closes the tab when the click
// is let go on it), a middle click to close, reordering (GTK's own reorderable tabs, put back and redone through the
// toolkit so its page list stays in step; elsewhere a drag across the tab row moves the tab as the pointer passes
// other tabs), the keys while the tab bar has the focus, and on Windows a context menu on a tab (Close Tab, Move Tab
// Left, Move Tab Right; the right button, Shift+F10 or the Applications key). The program hears of every change
// through three callbacks. On macOS the documents are window tabs (window_tabs_osx.mm) and this tab bar is not made.
//
// On Windows the drawing changes only what is painted: the control is still the system's tab control with each tab's
// text set, so screen readers read a tab list with every tab named by its document and the selected one marked. The
// drawn close sign is not an object screen readers see; the same action is on Ctrl+W, the middle click and the
// context menu.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/notebook.h>
#include <wx/imaglist.h>
#include <wx/dcmemory.h>
#include "../include/wxdragon.h"

#if defined(__WXGTK__)
#include <gtk/gtk.h>
#endif
#if defined(__WXMSW__)
#include <algorithm>
#include <vector>
#include <wx/renderer.h>
#include <wx/msw/private.h>
#include <wx/msw/wrapcctl.h>
#include <wx/msw/dc.h>
#include <wx/msw/private/darkmode.h>
#endif

namespace {

class DocTabs : public wxNotebook {
public:
    DocTabs(wxWindow* parent, void* data, wxd_DocTabs_ChangedFn changed, wxd_DocTabs_CloseFn close,
            wxd_DocTabs_MovedFn moved, wxd_DocTabs_FreeFn free_data)
#if defined(__WXMSW__)
        // Made in two steps so that Create calls this class's MSWGetStyle (the owner drawn style).
        : wxNotebook(),
#else
        : wxNotebook(parent, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxNB_TOP),
#endif
          m_data(data), m_changed(changed), m_close(close), m_moved(moved), m_free(free_data) {
#if defined(__WXMSW__)
        Create(parent, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxNB_TOP);
        // Each tab is as wide as its text, spaced as a browser's tabs, with room after the text for the close sign:
        // a blank image the size of the sign, which the tab control counts in the tab's width and never draws.
        SetSpacing();
        // Bound after wxNotebook's own paint handler (in Create), so it runs first: the whole control in dark mode.
        Bind(wxEVT_PAINT, &DocTabs::OnPaintDark, this);
        Bind(wxEVT_DPI_CHANGED, [this](wxDPIChangedEvent& e) {
            SetSpacing();
            e.Skip();
        });
        Bind(wxEVT_LEAVE_WINDOW, [this](wxMouseEvent& e) {
            SetHotClose(-1);
            e.Skip();
        });
        Bind(wxEVT_CONTEXT_MENU, &DocTabs::OnContextMenu, this);
#endif
#if defined(__WXGTK__)
        g_signal_connect(GetHandle(), "page-reordered", G_CALLBACK(OnPageReordered), this);
#endif
        Bind(wxEVT_NOTEBOOK_PAGE_CHANGED, &DocTabs::OnChanged, this);
        Bind(wxEVT_MIDDLE_DOWN, &DocTabs::OnMiddle, this);
        Bind(wxEVT_LEFT_DOWN, &DocTabs::OnLeftDown, this);
        Bind(wxEVT_MOTION, &DocTabs::OnMotion, this);
        Bind(wxEVT_LEFT_UP, &DocTabs::OnLeftUp, this);
        Bind(wxEVT_MOUSE_CAPTURE_LOST, [this](wxMouseCaptureLostEvent&) {
            m_dragFrom = -1;
            ClearPress();
        });
        Bind(wxEVT_CHAR_HOOK, &DocTabs::OnCharHook, this);
    }

    ~DocTabs() override {
        if (m_free) m_free(m_data);
    }

    // Adds `page` as the last tab. The program makes the new document the active one itself, so the selection the
    // add makes is not reported.
    int Add(wxWindow* page, const wxString& text, bool select) {
        const int index = (int)GetPageCount();
        m_adjusting = true;
        AddPage(page, text, select, ImageIndex());
        m_adjusting = false;
        Decorate(index);
        return index;
    }

    bool SetPageText(size_t n, const wxString& text) override {
        const bool ok = wxNotebook::SetPageText(n, text);
        NameCloseButton((int)n);
        return ok;
    }

    // Moves tab `from` to `to` through the toolkit (the page taken out and put back in), keeping the document that
    // was selected selected; the move is reported.
    bool Move(int from, int to) {
        const int count = (int)GetPageCount();
        if (from < 0 || to < 0 || from >= count || to >= count || from == to) return false;
        wxWindow* page = GetPage(from);
        const wxString text = GetPageText(from);
        const int selected = GetSelection();
        m_adjusting = true;
        RemovePage(from);
        InsertPage(to, page, text, false, ImageIndex());
        Decorate(to);
        int now = selected;
        if (selected == from) {
            now = to;
        } else if (from < selected && to >= selected) {
            now = selected - 1;
        } else if (from > selected && to <= selected) {
            now = selected + 1;
        }
        if (now >= 0 && now < count) ChangeSelection(now);
        m_adjusting = false;
        if (m_moved) m_moved(m_data, from, to);
        return true;
    }

    void RequestClose(int index) {
        if (index >= 0 && index < (int)GetPageCount() && m_close) m_close(m_data, index);
    }

    // The screen rectangle of tab `index`.
    bool TabRect(int index, wxRect& r) const {
        if (index < 0 || index >= (int)GetPageCount()) return false;
#if defined(__WXGTK__)
        GtkWidget* box = TabBox(index);
        return box && WidgetRect(box, r);
#elif defined(__WXMSW__)
        RECT rc;
        if (!TabCtrl_GetItemRect(GetHwnd(), index, &rc)) return false;
        const wxPoint at = ClientToScreen(wxPoint(rc.left, rc.top));
        r = wxRect(at.x, at.y, rc.right - rc.left, rc.bottom - rc.top);
        return true;
#else
        return false;
#endif
    }

    // The screen rectangle of the close button of tab `index`.
    bool CloseRect(int index, wxRect& r) const {
        if (index < 0 || index >= (int)GetPageCount()) return false;
#if defined(__WXGTK__)
        GtkWidget* button = CloseButton(index);
        return button && WidgetRect(button, r);
#elif defined(__WXMSW__)
        // The sign at the right of the tab, after the text.
        const wxRect close = CloseRectClient(index);
        if (close.IsEmpty()) return false;
        r = wxRect(ClientToScreen(close.GetPosition()), close.GetSize());
        return true;
#else
        return false;
#endif
    }

    // Puts the keyboard focus on the tab bar with the keyboard cues shown (the focus rectangle), as after a key;
    // for the build checks' picture of the focused tab bar.
    void ShowFocus() {
        SetFocus();
#if defined(__WXMSW__)
        if (wxWindow* top = wxGetTopLevelParent(this)) {
            ::SendMessage(GetHwndOf(top), WM_CHANGEUISTATE, MAKEWPARAM(UIS_CLEAR, UISF_HIDEFOCUS), 0);
        }
#endif
    }

private:
    static int ImageIndex() {
#if defined(__WXMSW__)
        return 0;
#else
        return -1;
#endif
    }

#if defined(__WXMSW__)
    // The tab control's style with the tabs drawn by the program (WM_DRAWITEM, MSWOnDraw).
    WXDWORD MSWGetStyle(long flags, WXDWORD* exstyle) const override {
        return wxNotebook::MSWGetStyle(flags, exstyle) | TCS_OWNERDRAWFIXED;
    }

    // The inside of one tab, which the tab control asks for after drawing the tab itself (its frame, the hot and the
    // selected look, the focus rectangle).
    bool MSWOnDraw(WXDRAWITEMSTRUCT* item) override {
        DRAWITEMSTRUCT* d = reinterpret_cast<DRAWITEMSTRUCT*>(item);
        if (!d || d->CtlType != ODT_TAB) return false;
        const int index = (int)d->itemID;
        if (index < 0 || index >= (int)GetPageCount()) return false;
        wxDCTemp dc((WXHDC)d->hDC);
        const wxRect r(d->rcItem.left, d->rcItem.top, d->rcItem.right - d->rcItem.left, d->rcItem.bottom - d->rcItem.top);
        DrawTabContent(dc, r, index);
        return true;
    }

    // The padding and the blank image that keeps room for the close sign, at the window's DPI.
    void SetSpacing() {
        const int side = FromDIP(16);
        wxBitmap blank(side, side);
        const wxColour key(255, 0, 255);
        {
            wxMemoryDC dc(blank);
            dc.SetBackground(wxBrush(key));
            dc.Clear();
        }
        blank.SetMask(new wxMask(blank, key));
        wxImageList* images = new wxImageList(side, side, true);
        images->Add(blank);
        AssignImageList(images);
        for (size_t i = 0; i < GetPageCount(); ++i) SetPageImage(i, 0);
        SetPadding(wxSize(FromDIP(10), FromDIP(4)));
    }

    // Tab `index`'s rectangle in the control, or an empty one.
    wxRect TabClientRect(int index) const {
        RECT rc;
        if (index < 0 || index >= (int)GetPageCount() || !TabCtrl_GetItemRect(GetHwnd(), index, &rc)) return wxRect();
        return wxRect(rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top);
    }

    // The close sign's square in a tab's rectangle: 16 by 16 (at 96 DPI), centred from top to bottom, its right edge
    // 6 inside the tab's. The one geometry the drawing, the mouse and CloseButtonRect use.
    wxRect CloseRectIn(const wxRect& tab) const {
        const int side = FromDIP(16);
        return wxRect(tab.x + tab.width - FromDIP(6) - side, tab.y + (tab.height - side) / 2, side, side);
    }

    wxRect CloseRectClient(int index) const {
        const wxRect tab = TabClientRect(index);
        return tab.IsEmpty() ? tab : CloseRectIn(tab);
    }

    // The tab whose close sign is at `at` (control coordinates), or -1.
    int CloseHit(const wxPoint& at) const {
        const int index = TabAt(at);
        return index >= 0 && CloseRectClient(index).Contains(at) ? index : -1;
    }

    void RefreshTab(int index) {
        if (index < 0) return;
        wxRect r = TabClientRect(index);
        if (r.IsEmpty()) return;
        r.Inflate(FromDIP(2));
        RefreshRect(r, false);
    }

    void SetHotClose(int index) {
        if (index == m_hotClose) return;
        const int was = m_hotClose;
        m_hotClose = index;
        RefreshTab(was);
        RefreshTab(index);
    }

    static bool HighContrast() {
        HIGHCONTRAST hc = {};
        hc.cbSize = sizeof(hc);
        return ::SystemParametersInfo(SPI_GETHIGHCONTRAST, sizeof(hc), &hc, 0) && (hc.dwFlags & HCF_HIGHCONTRASTON);
    }

    static wxColour Blend(const wxColour& from, const wxColour& to, double part) {
        const auto mix = [part](unsigned char a, unsigned char b) {
            return (unsigned char)(a + (b - a) * part + 0.5);
        };
        return wxColour(mix(from.Red(), to.Red()), mix(from.Green(), to.Green()), mix(from.Blue(), to.Blue()));
    }

    // Where tab `index`'s text goes in its rectangle: from the padding to the close sign, less a gap.
    wxRect TextRectIn(const wxRect& tab) const {
        const wxRect close = CloseRectIn(tab);
        const int left = tab.x + FromDIP(8);
        return wxRect(left, tab.y, std::max(0, close.x - FromDIP(6) - left), tab.height);
    }

    // The inside of tab `index` in `tab`: its text (no mnemonic: an "&" in a name shows), cut with an ellipsis when
    // it does not fit, and the close sign at the right; the sign on a background while the pointer is on it, a
    // darker one while it is pressed. System colours throughout; in high contrast the highlight colours.
    void DrawTabContent(wxDC& dc, const wxRect& tab, int index) {
        const bool contrast = HighContrast();
        const wxColour text = wxSystemSettings::GetColour(wxSYS_COLOUR_BTNTEXT);
        const wxRect close = CloseRectIn(tab);
        const bool pressed = m_pressClose == index && m_pressInside;
        const bool hot = m_hotClose == index || pressed;
        wxColour sign = text;
        if (hot) {
            wxColour back;
            if (contrast) {
                back = wxSystemSettings::GetColour(wxSYS_COLOUR_HIGHLIGHT);
                sign = wxSystemSettings::GetColour(wxSYS_COLOUR_HIGHLIGHTTEXT);
            } else if (pressed) {
                back = wxSystemSettings::GetColour(wxSYS_COLOUR_BTNSHADOW);
            } else {
                back = Blend(wxSystemSettings::GetColour(wxSYS_COLOUR_BTNFACE),
                             wxSystemSettings::GetColour(wxSYS_COLOUR_BTNSHADOW), 0.4);
            }
            dc.SetPen(*wxTRANSPARENT_PEN);
            dc.SetBrush(wxBrush(back));
            dc.DrawRoundedRectangle(close, FromDIP(3));
        }
        const int inset = FromDIP(4);
        const int width = GetDPIScaleFactor() >= 1.5 ? FromDIP(1) + 1 : 1;
        dc.SetPen(wxPen(sign, width));
        dc.DrawLine(close.x + inset, close.y + inset, close.GetRight() - inset + 1, close.GetBottom() - inset + 1);
        dc.DrawLine(close.GetRight() - inset, close.y + inset, close.x + inset - 1, close.GetBottom() - inset + 1);

        const wxRect place = TextRectIn(tab);
        dc.SetFont(GetFont());
        dc.SetTextForeground(text);
        dc.SetBackgroundMode(wxBRUSHSTYLE_TRANSPARENT);
        const wxString label =
            wxControl::Ellipsize(GetPageText(index), dc, wxELLIPSIZE_END, place.width, wxELLIPSIZE_FLAGS_NONE);
        dc.SetClippingRegion(place);
        dc.DrawText(label, place.x, place.y + (place.height - dc.GetCharHeight()) / 2);
        dc.DestroyClippingRegion();
    }

    // In dark mode wxNotebook paints the whole control itself and never asks for the tabs' insides (the image at
    // the left, then the text), so the control is painted here in that mode as wxNotebook does (the tab row on the
    // button face colour, a line under it, each tab's frame, the selected one last and raised), with the insides as
    // above and the focus rectangle, which wxNotebook's painting leaves out. Elsewhere the system paints.
    void OnPaintDark(wxPaintEvent& e) {
        if (!wxMSWDarkMode::IsActive()) {
            e.Skip();
            return;
        }
        // The tab rectangles are asked for before the paint DC is made: asking can make the control paint.
        const int count = (int)GetPageCount();
        std::vector<wxRect> rects;
        for (int n = 0; n < count; ++n) rects.push_back(TabClientRect(n));
        wxPaintDC dc(this);
        dc.Clear();
        if (count == 0) return;
        const wxSize size = GetClientSize();
        const int selected = GetSelection();
        const int lift = FromDIP(2);
        int bottom = 0;
        for (const wxRect& r : rects) bottom = std::max(bottom, r.GetBottom());
        const wxRect row(0, 0, size.x, bottom + 1);
        dc.SetPen(*wxTRANSPARENT_PEN);
        dc.SetBrush(wxBrush(wxSystemSettings::GetColour(wxSYS_COLOUR_BTNFACE)));
        dc.DrawRectangle(row);
        dc.SetPen(wxPen(wxSystemSettings::GetColour(wxSYS_COLOUR_MENUBAR)));
        dc.DrawLine(0, bottom, size.x, bottom);
        const wxPoint pointer = ScreenToClient(wxGetMousePosition());
        for (int n = 0; n < count; ++n) {
            if (n == selected || rects[n].IsEmpty()) continue;
            wxRect r = rects[n];
            if (n > 0) {
                // Each frame overlaps the one before it, so two tabs share one line between them.
                r.x--;
                r.width++;
            }
            const bool current = r.Contains(pointer);
            dc.SetBrush(wxBrush(wxSystemSettings::GetColour(current ? wxSYS_COLOUR_HOTLIGHT : wxSYS_COLOUR_BTNSHADOW)));
            dc.DrawRectangle(r);
            DrawTabContent(dc, rects[n], n);
        }
        if (selected >= 0 && selected < count && !rects[selected].IsEmpty()) {
            wxRect r = rects[selected];
            r.Inflate(lift, 0);
            r.y -= lift;
            r.height += lift;
            dc.SetBrush(wxBrush(GetBackgroundColour()));
            dc.DrawRectangle(r);
            wxRect inside = rects[selected];
            inside.y -= lift;
            DrawTabContent(dc, inside, selected);
            const LRESULT cues = ::SendMessage(GetHwnd(), WM_QUERYUISTATE, 0, 0);
            if (HasFocus() && !(cues & UISF_HIDEFOCUS)) {
                wxRect focus = TextRectIn(inside);
                focus.Deflate(0, FromDIP(3));
                wxRendererNative::Get().DrawFocusRect(this, dc, focus);
            }
        }
    }

    // The context menu of the tab under the pointer, or of the selected tab from the keyboard (Shift+F10, the
    // Applications key), with the keys that do the same shown beside each item.
    void OnContextMenu(wxContextMenuEvent& e) {
        const int count = (int)GetPageCount();
        wxPoint at = e.GetPosition();
        int index = -1;
        if (at == wxDefaultPosition) {
            index = GetSelection();
            const wxRect tab = TabClientRect(index);
            at = wxPoint(tab.x + FromDIP(8), tab.GetBottom());
        } else {
            at = ScreenToClient(at);
            index = TabAt(at);
        }
        if (index < 0 || index >= count) {
            e.Skip();
            return;
        }
        enum { CloseItem = 1, LeftItem, RightItem };
        wxMenu menu;
        menu.Append(CloseItem, "Close Tab\tCtrl+W");
        menu.Append(LeftItem, "Move Tab Left\tCtrl+Shift+Page Up");
        menu.Append(RightItem, "Move Tab Right\tCtrl+Shift+Page Down");
        menu.Enable(LeftItem, index > 0);
        menu.Enable(RightItem, index + 1 < count);
        switch (GetPopupMenuSelectionFromUser(menu, at)) {
            case CloseItem:
                RequestClose(index);
                break;
            case LeftItem:
                Move(index, index - 1);
                break;
            case RightItem:
                Move(index, index + 1);
                break;
            default:
                break;
        }
    }
#endif

    void OnChanged(wxBookCtrlEvent& e) {
        e.Skip();
        if (m_adjusting) return;
        if (m_changed) m_changed(m_data, e.GetSelection());
    }

    // A tab under the pointer, or -1: the page area under the tabs does not count.
    int TabAt(const wxPoint& at) const {
        long flags = 0;
        const int index = HitTest(at, &flags);
        if (index == wxNOT_FOUND || (flags & wxBK_HITTEST_ONPAGE)) return -1;
        return index;
    }

    void OnMiddle(wxMouseEvent& e) {
        const int index = TabAt(e.GetPosition());
        if (index >= 0) {
            RequestClose(index);
        } else {
            e.Skip();
        }
    }

    void OnLeftDown(wxMouseEvent& e) {
#if defined(__WXMSW__)
        // A press on a close sign is kept (the tab is not selected); the tab closes when the button is let go on
        // the same sign, as in a browser.
        const int hit = CloseHit(e.GetPosition());
        if (hit >= 0) {
            m_pressClose = hit;
            m_pressInside = true;
            if (!HasCapture()) CaptureMouse();
            RefreshTab(hit);
            return;
        }
#endif
#if !defined(__WXGTK__)
        m_dragFrom = TabAt(e.GetPosition());
#endif
        e.Skip();
    }

    void OnLeftUp(wxMouseEvent& e) {
        m_dragFrom = -1;
#if defined(__WXMSW__)
        if (m_pressClose >= 0) {
            const int index = m_pressClose;
            const bool inside = CloseHit(e.GetPosition()) == index;
            ClearPress();
            if (inside) RequestClose(index);
            return;
        }
#endif
        e.Skip();
    }

    void ClearPress() {
#if defined(__WXMSW__)
        const int index = m_pressClose;
        m_pressClose = -1;
        m_pressInside = false;
        if (HasCapture()) ReleaseMouse();
        RefreshTab(index);
#endif
    }

    // Elsewhere than GTK: a tab dragged along the row moves as the pointer passes another tab.
    void OnMotion(wxMouseEvent& e) {
        e.Skip();
#if defined(__WXMSW__)
        const int hot = CloseHit(e.GetPosition());
        SetHotClose(hot);
        if (m_pressClose >= 0) {
            const bool inside = hot == m_pressClose;
            if (inside != m_pressInside) {
                m_pressInside = inside;
                RefreshTab(m_pressClose);
            }
            return;
        }
#endif
#if !defined(__WXGTK__)
        if (m_dragFrom < 0 || !e.LeftIsDown()) return;
        const int index = TabAt(e.GetPosition());
        if (index < 0 || index == m_dragFrom) return;
        const int from = m_dragFrom;
        m_dragFrom = index;
        Move(from, index);
#endif
    }

    // The keys while the tab bar itself has the focus (in a page, the editing window has its own handler for them):
    // Ctrl+Tab and Ctrl+Shift+Tab the next and previous tab (wrapping), Ctrl+W (Cmd+W on macOS) close, Ctrl+Shift+
    // Page Up and Page Down move the tab.
    void OnCharHook(wxKeyEvent& e) {
        if (wxWindow::FindFocus() != this) {
            e.Skip();
            return;
        }
        const int code = e.GetKeyCode();
        const bool control = e.RawControlDown();
        const bool command = e.ControlDown();
        const bool shift = e.ShiftDown();
        const int count = (int)GetPageCount();
        const int selected = GetSelection();
        if (control && !e.AltDown() && code == WXK_TAB && count > 0) {
            SetSelection((selected + (shift ? count - 1 : 1)) % count);
            return;
        }
        if (command && !shift && !e.AltDown() && (code == 'W' || code == 'w')) {
            RequestClose(selected);
            return;
        }
        if (control && shift && !e.AltDown() && (code == WXK_PAGEUP || code == WXK_PAGEDOWN) && selected >= 0) {
            Move(selected, code == WXK_PAGEUP ? selected - 1 : selected + 1);
            return;
        }
        e.Skip();
    }

#if defined(__WXGTK__)
    GtkWidget* TabBox(int index) const {
        GtkNotebook* notebook = GTK_NOTEBOOK(GetHandle());
        GtkWidget* child = gtk_notebook_get_nth_page(notebook, index);
        return child ? gtk_notebook_get_tab_label(notebook, child) : nullptr;
    }

    GtkWidget* CloseButton(int index) const {
        GtkWidget* box = TabBox(index);
        if (!box || !GTK_IS_CONTAINER(box)) return nullptr;
        GtkWidget* found = nullptr;
        GList* children = gtk_container_get_children(GTK_CONTAINER(box));
        for (GList* k = children; k; k = k->next) {
            if (g_object_get_data(G_OBJECT(k->data), "braillecraft-close")) found = GTK_WIDGET(k->data);
        }
        g_list_free(children);
        return found;
    }

    // A widget's screen rectangle: its allocation is in its GdkWindow's coordinates.
    static bool WidgetRect(GtkWidget* widget, wxRect& r) {
        GdkWindow* window = gtk_widget_get_window(widget);
        if (!window) return false;
        GtkAllocation a;
        gtk_widget_get_allocation(widget, &a);
        int ox = 0, oy = 0;
        gdk_window_get_origin(window, &ox, &oy);
        if (gtk_widget_get_has_window(widget)) {
            r = wxRect(ox, oy, a.width, a.height);
        } else {
            r = wxRect(ox + a.x, oy + a.y, a.width, a.height);
        }
        return true;
    }

    // The tab of page `index`: reorderable by dragging, with a close button after its text.
    void Decorate(int index) {
        GtkNotebook* notebook = GTK_NOTEBOOK(GetHandle());
        GtkWidget* child = gtk_notebook_get_nth_page(notebook, index);
        if (!child) return;
        gtk_notebook_set_tab_reorderable(notebook, child, TRUE);
        GtkWidget* box = gtk_notebook_get_tab_label(notebook, child);
        if (!box || !GTK_IS_BOX(box) || CloseButton(index)) return;
        GtkWidget* button = gtk_button_new_from_icon_name("window-close-symbolic", GTK_ICON_SIZE_MENU);
        gtk_button_set_relief(GTK_BUTTON(button), GTK_RELIEF_NONE);
        gtk_widget_set_focus_on_click(button, FALSE);
        g_object_set_data(G_OBJECT(button), "braillecraft-close", child);
        g_signal_connect(button, "clicked", G_CALLBACK(OnCloseClicked), this);
        gtk_box_pack_end(GTK_BOX(box), button, FALSE, FALSE, 0);
        gtk_widget_show(button);
        NameCloseButton(index);
    }

    // The button's name for screen readers and its tooltip: "Close" and the tab's text.
    void NameCloseButton(int index) {
        GtkWidget* button = CloseButton(index);
        if (!button) return;
        const wxString name = wxString("Close ") + GetPageText(index);
        gtk_widget_set_tooltip_text(button, name.utf8_str());
        atk_object_set_name(gtk_widget_get_accessible(button), name.utf8_str());
    }

    static void OnCloseClicked(GtkButton* button, gpointer self) {
        DocTabs* tabs = static_cast<DocTabs*>(self);
        GtkWidget* child = static_cast<GtkWidget*>(g_object_get_data(G_OBJECT(button), "braillecraft-close"));
        tabs->RequestClose(gtk_notebook_page_num(GTK_NOTEBOOK(tabs->GetHandle()), child));
    }

    // GTK moved a tab by a drag; the toolkit's own page list has not moved. The tab goes back where the toolkit
    // has it, and the move is made again through the toolkit once GTK has finished with the drag.
    static void OnPageReordered(GtkNotebook* notebook, GtkWidget* child, guint page_num, gpointer self) {
        DocTabs* tabs = static_cast<DocTabs*>(self);
        if (tabs->m_adjusting) return;
        int from = -1;
        for (size_t i = 0; i < tabs->GetPageCount(); ++i) {
            if (tabs->GetPage(i)->GetHandle() == child) from = (int)i;
        }
        const int to = (int)page_num;
        if (from < 0 || from == to) return;
        tabs->m_adjusting = true;
        gtk_notebook_reorder_child(notebook, child, from);
        tabs->m_adjusting = false;
        tabs->CallAfter([tabs, from, to] { tabs->Move(from, to); });
    }
#else
    void Decorate(int) {}
    void NameCloseButton(int) {}
#endif

    void* m_data;
    wxd_DocTabs_ChangedFn m_changed;
    wxd_DocTabs_CloseFn m_close;
    wxd_DocTabs_MovedFn m_moved;
    wxd_DocTabs_FreeFn m_free;
    // A change the tab bar is making itself: the selection events it causes are not reported.
    bool m_adjusting = false;
    int m_dragFrom = -1;
#if defined(__WXMSW__)
    // The tab whose close sign is under the pointer, and the one whose sign was pressed (and whether the pointer is
    // still on it), or -1.
    int m_hotClose = -1;
    int m_pressClose = -1;
    bool m_pressInside = false;
#endif
};

DocTabs* Tabs(wxd_Window_t* tabs) {
    return dynamic_cast<DocTabs*>(reinterpret_cast<wxWindow*>(tabs));
}

bool RectOut(bool ok, const wxRect& r, int* x, int* y, int* w, int* h) {
    if (!ok) return false;
    if (x) *x = r.x;
    if (y) *y = r.y;
    if (w) *w = r.width;
    if (h) *h = r.height;
    return true;
}

}  // namespace

extern "C" {

WXD_EXPORTED wxd_Window_t* wxd_DocTabs_Create(wxd_Window_t* parent, void* data, wxd_DocTabs_ChangedFn changed,
                                              wxd_DocTabs_CloseFn close, wxd_DocTabs_MovedFn moved,
                                              wxd_DocTabs_FreeFn free_data) {
    wxWindow* p = reinterpret_cast<wxWindow*>(parent);
    if (!p) return nullptr;
    return reinterpret_cast<wxd_Window_t*>(new DocTabs(p, data, changed, close, moved, free_data));
}

WXD_EXPORTED int wxd_DocTabs_AddPage(wxd_Window_t* tabs, wxd_Window_t* page, const char* text, bool select) {
    DocTabs* t = Tabs(tabs);
    wxWindow* p = reinterpret_cast<wxWindow*>(page);
    if (!t || !p) return -1;
    return t->Add(p, wxString::FromUTF8(text ? text : ""), select);
}

WXD_EXPORTED void wxd_DocTabs_SetPageText(wxd_Window_t* tabs, int index, const char* text) {
    DocTabs* t = Tabs(tabs);
    if (!t || index < 0 || index >= (int)t->GetPageCount()) return;
    t->SetPageText((size_t)index, wxString::FromUTF8(text ? text : ""));
}

WXD_EXPORTED int wxd_DocTabs_FindPage(wxd_Window_t* tabs, wxd_Window_t* page) {
    DocTabs* t = Tabs(tabs);
    wxWindow* p = reinterpret_cast<wxWindow*>(page);
    if (!t || !p) return -1;
    const int index = t->FindPage(p);
    return index == wxNOT_FOUND ? -1 : index;
}

WXD_EXPORTED int wxd_DocTabs_GetSelection(wxd_Window_t* tabs) {
    DocTabs* t = Tabs(tabs);
    if (!t) return -1;
    const int index = t->GetSelection();
    return index == wxNOT_FOUND ? -1 : index;
}

WXD_EXPORTED void wxd_DocTabs_SetSelection(wxd_Window_t* tabs, int index) {
    DocTabs* t = Tabs(tabs);
    if (!t || index < 0 || index >= (int)t->GetPageCount()) return;
    t->SetSelection((size_t)index);
}

WXD_EXPORTED void wxd_DocTabs_ChangeSelection(wxd_Window_t* tabs, int index) {
    DocTabs* t = Tabs(tabs);
    if (!t || index < 0 || index >= (int)t->GetPageCount()) return;
    t->ChangeSelection((size_t)index);
}

WXD_EXPORTED int wxd_DocTabs_GetPageCount(wxd_Window_t* tabs) {
    DocTabs* t = Tabs(tabs);
    return t ? (int)t->GetPageCount() : 0;
}

WXD_EXPORTED bool wxd_DocTabs_RemovePage(wxd_Window_t* tabs, int index) {
    DocTabs* t = Tabs(tabs);
    if (!t || index < 0 || index >= (int)t->GetPageCount()) return false;
    return t->RemovePage((size_t)index);
}

WXD_EXPORTED bool wxd_DocTabs_MovePage(wxd_Window_t* tabs, int from, int to) {
    DocTabs* t = Tabs(tabs);
    return t && t->Move(from, to);
}

WXD_EXPORTED bool wxd_DocTabs_TabRect(wxd_Window_t* tabs, int index, int* x, int* y, int* w, int* h) {
    DocTabs* t = Tabs(tabs);
    wxRect r;
    return t && RectOut(t->TabRect(index, r), r, x, y, w, h);
}

WXD_EXPORTED void wxd_DocTabs_ShowFocus(wxd_Window_t* tabs) {
    DocTabs* t = Tabs(tabs);
    if (t) t->ShowFocus();
}

WXD_EXPORTED bool wxd_DocTabs_CloseButtonRect(wxd_Window_t* tabs, int index, int* x, int* y, int* w, int* h) {
    DocTabs* t = Tabs(tabs);
    wxRect r;
    return t && RectOut(t->CloseRect(index, r), r, x, y, w, h);
}

}
