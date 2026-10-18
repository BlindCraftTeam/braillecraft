// Added for BrailleCraft: the tab bar of the main window (wxd_doc_tabs.h).
//
// A wxNotebook, so the system's own tab control carries the tabs and screen readers read them as a tab list. What is
// added: a close button in each tab (GTK: a real button packed into the tab's label box; Windows: the tab's image,
// which the tab control hit-tests on a click; macOS has no room for one in its tabs), a middle click to close,
// reordering (GTK's own reorderable tabs, put back and redone through the toolkit so its page list stays in step;
// elsewhere a drag across the tab row moves the tab as the pointer passes other tabs), and the keys while the tab bar
// has the focus. The program hears of every change through three callbacks.

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
#include <wx/msw/private.h>
#include <wx/msw/wrapcctl.h>
#endif

namespace {

class DocTabs : public wxNotebook {
public:
    DocTabs(wxWindow* parent, void* data, wxd_DocTabs_ChangedFn changed, wxd_DocTabs_CloseFn close,
            wxd_DocTabs_MovedFn moved, wxd_DocTabs_FreeFn free_data)
        : wxNotebook(parent, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxNB_TOP),
          m_data(data), m_changed(changed), m_close(close), m_moved(moved), m_free(free_data) {
#if defined(__WXMSW__)
        // The close image drawn beside each tab's text; the tab control tells a click on it apart from one on the
        // text (HitTest, wxBK_HITTEST_ONICON).
        wxImageList* images = new wxImageList(16, 16, true);
        images->Add(CloseBitmap());
        AssignImageList(images);
#endif
#if defined(__WXGTK__)
        g_signal_connect(GetHandle(), "page-reordered", G_CALLBACK(OnPageReordered), this);
#endif
        Bind(wxEVT_NOTEBOOK_PAGE_CHANGED, &DocTabs::OnChanged, this);
        Bind(wxEVT_MIDDLE_DOWN, &DocTabs::OnMiddle, this);
        Bind(wxEVT_LEFT_DOWN, &DocTabs::OnLeftDown, this);
        Bind(wxEVT_MOTION, &DocTabs::OnMotion, this);
        Bind(wxEVT_LEFT_UP, [this](wxMouseEvent& e) {
            m_dragFrom = -1;
            e.Skip();
        });
        Bind(wxEVT_MOUSE_CAPTURE_LOST, [this](wxMouseCaptureLostEvent&) { m_dragFrom = -1; });
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
        // The image sits at the left of the tab, before the text.
        wxRect tab;
        if (!TabRect(index, tab)) return false;
        r = wxRect(tab.x + 2, tab.y, wxMin(20, tab.width), tab.height);
        return true;
#else
        return false;
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
    // A 16 by 16 close sign: two diagonals in the window text colour on a masked background.
    static wxBitmap CloseBitmap() {
        wxBitmap bitmap(16, 16);
        const wxColour mask(255, 0, 255);
        {
            wxMemoryDC dc(bitmap);
            dc.SetBackground(wxBrush(mask));
            dc.Clear();
            dc.SetPen(wxPen(wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOWTEXT), 2));
            dc.DrawLine(4, 4, 12, 12);
            dc.DrawLine(12, 4, 4, 12);
        }
        bitmap.SetMask(new wxMask(bitmap, mask));
        return bitmap;
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
        long flags = 0;
        const int hit = HitTest(e.GetPosition(), &flags);
        if (hit != wxNOT_FOUND && (flags & wxBK_HITTEST_ONICON)) {
            RequestClose(hit);
            return;
        }
#endif
#if !defined(__WXGTK__)
        m_dragFrom = TabAt(e.GetPosition());
#endif
        e.Skip();
    }

    // Elsewhere than GTK: a tab dragged along the row moves as the pointer passes another tab.
    void OnMotion(wxMouseEvent& e) {
        e.Skip();
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

WXD_EXPORTED bool wxd_DocTabs_CloseButtonRect(wxd_Window_t* tabs, int index, int* x, int* y, int* w, int* h) {
    DocTabs* t = Tabs(tabs);
    wxRect r;
    return t && RectOut(t->CloseRect(index, r), r, x, y, w, h);
}

}
