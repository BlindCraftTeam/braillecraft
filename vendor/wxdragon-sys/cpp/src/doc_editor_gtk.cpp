// Added for BrailleCraft: the drawn document editing window's text for screen readers on Linux (ATK, read by Orca
// through AT-SPI).
//
// wxWidgets has no accessibility layer on GTK (wxUSE_ACCESSIBILITY is Windows only). The window's GTK widget gets an
// accessible object of our own: a GtkWidgetAccessible that also has the ATK text interface, over the window's text
// for screen readers (each text row followed by a line feed, page break rows not in it: the reference's own text for
// its editing window, spec 27 section 6). It tells screen readers when the text changes (only the stretch that
// changed), when the caret moves and when the selection changes. A screen reader that moves the caret or selects
// through the interface does so as a click would.
//
// The widget's class is the one every wxWidgets window shares (wxPizza); its get_accessible is replaced once by one
// that gives the editing window its own object and every other window the one it had.

#if defined(__WXGTK__)

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <gtk/gtk.h>
#include <gtk/gtk-a11y.h>
#include <atk/atk.h>
#include <cstring>
#include "doc_editor_internal.h"

namespace {

const char* const kEditorKey = "bc-doc-editor";
const char* const kAccessibleKey = "bc-doc-editor-accessible";

struct BcEditorAccessible {
    GtkWidgetAccessible parent;
};

struct BcEditorAccessibleClass {
    GtkWidgetAccessibleClass parent_class;
};

void bc_text_init(AtkTextIface* iface);

G_DEFINE_TYPE_WITH_CODE(BcEditorAccessible, bc_editor_accessible, GTK_TYPE_WIDGET_ACCESSIBLE,
                        G_IMPLEMENT_INTERFACE(ATK_TYPE_TEXT, bc_text_init))

DocEditor* EditorOf(gpointer accessible)
{
    GtkWidget* w = gtk_accessible_get_widget(GTK_ACCESSIBLE(accessible));
    return w ? static_cast<DocEditor*>(g_object_get_data(G_OBJECT(w), kEditorKey)) : nullptr;
}

AtkStateSet* bc_ref_state_set(AtkObject* obj)
{
    AtkStateSet* set = ATK_OBJECT_CLASS(bc_editor_accessible_parent_class)->ref_state_set(obj);
    if (EditorOf(obj)) {
        atk_state_set_add_state(set, ATK_STATE_EDITABLE);
        atk_state_set_add_state(set, ATK_STATE_MULTI_LINE);
        atk_state_set_add_state(set, ATK_STATE_FOCUSABLE);
    }
    return set;
}

void bc_initialize(AtkObject* obj, gpointer data)
{
    ATK_OBJECT_CLASS(bc_editor_accessible_parent_class)->initialize(obj, data);
    obj->role = ATK_ROLE_TEXT;
}

void bc_editor_accessible_class_init(BcEditorAccessibleClass* klass)
{
    AtkObjectClass* atk = ATK_OBJECT_CLASS(klass);
    atk->ref_state_set = bc_ref_state_set;
    atk->initialize = bc_initialize;
}

void bc_editor_accessible_init(BcEditorAccessible*) {}

gchar* Utf8(const wxString& s)
{
    const wxScopedCharBuffer b = s.utf8_str();
    return g_strdup(b.data() ? b.data() : "");
}

// ---- AtkText ----

gchar* bc_get_text(AtkText* t, gint start, gint end)
{
    DocEditor* e = EditorOf(t);
    if (!e) return g_strdup("");
    const wxString& text = e->AccessibleText();
    const int n = (int)text.length();
    if (end < 0 || end > n) end = n;
    start = wxClip(start, 0, n);
    if (end < start) end = start;
    return Utf8(text.Mid(start, end - start));
}

gint bc_get_character_count(AtkText* t)
{
    DocEditor* e = EditorOf(t);
    return e ? (gint)e->AccessibleText().length() : 0;
}

gunichar bc_get_character_at_offset(AtkText* t, gint offset)
{
    DocEditor* e = EditorOf(t);
    if (!e) return 0;
    const wxString& text = e->AccessibleText();
    if (offset < 0 || offset >= (int)text.length()) return 0;
    return (gunichar)text[offset].GetValue();
}

gint bc_get_caret_offset(AtkText* t)
{
    DocEditor* e = EditorOf(t);
    return e ? e->OffsetOf(e->CaretRow(), e->CaretCol()) : 0;
}

gboolean bc_set_caret_offset(AtkText* t, gint offset)
{
    DocEditor* e = EditorOf(t);
    if (!e) return FALSE;
    int row = 0, col = 0;
    e->PlaceOf(offset, &row, &col);
    e->Click(0, row, col);
    return TRUE;
}

bool IsWordChar(wxUniChar c)
{
    return !(c == ' ' || c == '\n' || c == '\t');
}

// The start and end of the text unit of kind `g` at `offset`.
void UnitAt(const wxString& text, gint offset, AtkTextGranularity g, gint* start, gint* end)
{
    const int n = (int)text.length();
    offset = wxClip(offset, 0, n);
    int s = offset, en = offset;
    switch (g) {
    case ATK_TEXT_GRANULARITY_CHAR:
        en = wxMin(offset + 1, n);
        break;
    case ATK_TEXT_GRANULARITY_WORD:
        // The word the offset is in, with the spaces after it (as GTK's text view gives it).
        while (s > 0 && IsWordChar(text[s - 1])) --s;
        while (en < n && IsWordChar(text[en])) ++en;
        while (en < n && text[en] == ' ') ++en;
        break;
    case ATK_TEXT_GRANULARITY_LINE:
    case ATK_TEXT_GRANULARITY_SENTENCE:
    case ATK_TEXT_GRANULARITY_PARAGRAPH:
    default:
        // A row, with its line feed (each row is one line of the editing window).
        while (s > 0 && text[s - 1] != '\n') --s;
        while (en < n && text[en] != '\n') ++en;
        if (en < n) ++en;
        break;
    }
    *start = s;
    *end = en;
}

gchar* bc_get_string_at_offset(AtkText* t, gint offset, AtkTextGranularity g, gint* start, gint* end)
{
    *start = *end = 0;
    DocEditor* e = EditorOf(t);
    if (!e) return g_strdup("");
    const wxString& text = e->AccessibleText();
    UnitAt(text, offset, g, start, end);
    return Utf8(text.Mid(*start, *end - *start));
}

AtkTextGranularity GranularityOf(AtkTextBoundary b)
{
    switch (b) {
    case ATK_TEXT_BOUNDARY_CHAR: return ATK_TEXT_GRANULARITY_CHAR;
    case ATK_TEXT_BOUNDARY_WORD_START:
    case ATK_TEXT_BOUNDARY_WORD_END: return ATK_TEXT_GRANULARITY_WORD;
    case ATK_TEXT_BOUNDARY_SENTENCE_START:
    case ATK_TEXT_BOUNDARY_SENTENCE_END: return ATK_TEXT_GRANULARITY_SENTENCE;
    default: return ATK_TEXT_GRANULARITY_LINE;
    }
}

gchar* bc_get_text_at_offset(AtkText* t, gint offset, AtkTextBoundary b, gint* start, gint* end)
{
    return bc_get_string_at_offset(t, offset, GranularityOf(b), start, end);
}

gchar* bc_get_text_before_offset(AtkText* t, gint offset, AtkTextBoundary b, gint* start, gint* end)
{
    *start = *end = 0;
    DocEditor* e = EditorOf(t);
    if (!e) return g_strdup("");
    const wxString& text = e->AccessibleText();
    gint s = 0, en = 0;
    UnitAt(text, offset, GranularityOf(b), &s, &en);
    if (s == 0) return g_strdup("");
    return bc_get_string_at_offset(t, s - 1, GranularityOf(b), start, end);
}

gchar* bc_get_text_after_offset(AtkText* t, gint offset, AtkTextBoundary b, gint* start, gint* end)
{
    *start = *end = 0;
    DocEditor* e = EditorOf(t);
    if (!e) return g_strdup("");
    const wxString& text = e->AccessibleText();
    gint s = 0, en = 0;
    UnitAt(text, offset, GranularityOf(b), &s, &en);
    if (en >= (int)text.length()) {
        *start = *end = (gint)text.length();
        return g_strdup("");
    }
    return bc_get_string_at_offset(t, en, GranularityOf(b), start, end);
}

// A rectangle of the window (client coordinates) in the coordinates ATK asks for.
void ToCoords(DocEditor* e, wxRect r, AtkCoordType coords, gint* x, gint* y)
{
    wxPoint p = e->ClientToScreen(r.GetPosition());
    if (coords == ATK_XY_WINDOW) {
        wxWindow* top = wxGetTopLevelParent(e);
        if (top) p -= top->GetScreenPosition();
    }
    *x = p.x;
    *y = p.y;
}

void bc_get_character_extents(AtkText* t, gint offset, gint* x, gint* y, gint* w, gint* h, AtkCoordType coords)
{
    *x = *y = *w = *h = 0;
    DocEditor* e = EditorOf(t);
    if (!e) return;
    int row = 0, col = 0;
    e->PlaceOf(offset, &row, &col);
    const wxRect r = e->RowRect(row, col, col + 1);
    ToCoords(e, r, coords, x, y);
    *w = r.width;
    *h = r.height;
}

gint bc_get_offset_at_point(AtkText* t, gint x, gint y, AtkCoordType coords)
{
    DocEditor* e = EditorOf(t);
    if (!e) return -1;
    wxPoint p(x, y);
    if (coords == ATK_XY_WINDOW) {
        wxWindow* top = wxGetTopLevelParent(e);
        if (top) p += top->GetScreenPosition();
    }
    p = e->ScreenToClient(p);
    int vx = 0, vy = 0;
    e->CalcUnscrolledPosition(p.x, p.y, &vx, &vy);
    const int row = wxClip(vy / wxMax(e->RowHeight(), 1), 0, wxMax(e->RowCount() - 1, 0));
    return e->OffsetOf(row, e->ColumnAt(row, vx));
}

// The selection, earlier end first, as offsets.
bool SelectionOf(DocEditor* e, gint* start, gint* end)
{
    const int a = e->OffsetOf(e->AnchorRow(), e->AnchorCol());
    const int c = e->OffsetOf(e->CaretRow(), e->CaretCol());
    *start = wxMin(a, c);
    *end = wxMax(a, c);
    return a != c;
}

gint bc_get_n_selections(AtkText* t)
{
    DocEditor* e = EditorOf(t);
    gint s = 0, en = 0;
    return e && SelectionOf(e, &s, &en) ? 1 : 0;
}

gchar* bc_get_selection(AtkText* t, gint n, gint* start, gint* end)
{
    *start = *end = 0;
    DocEditor* e = EditorOf(t);
    if (!e || n != 0 || !SelectionOf(e, start, end)) return nullptr;
    return Utf8(e->AccessibleText().Mid(*start, *end - *start));
}

gboolean bc_set_selection(AtkText* t, gint n, gint start, gint end)
{
    DocEditor* e = EditorOf(t);
    if (!e || n != 0) return FALSE;
    int r1 = 0, c1 = 0, r2 = 0, c2 = 0;
    e->PlaceOf(start, &r1, &c1);
    e->PlaceOf(end, &r2, &c2);
    e->Click(0, r1, c1);
    e->Click(1, r2, c2);
    e->Click(2, r2, c2);
    return TRUE;
}

gboolean bc_add_selection(AtkText* t, gint start, gint end)
{
    return bc_set_selection(t, 0, start, end);
}

gboolean bc_remove_selection(AtkText* t, gint n)
{
    DocEditor* e = EditorOf(t);
    if (!e || n != 0) return FALSE;
    e->Click(0, e->CaretRow(), e->CaretCol());
    return TRUE;
}

AtkAttributeSet* bc_get_run_attributes(AtkText*, gint offset, gint* start, gint* end)
{
    *start = offset;
    *end = offset + 1;
    return nullptr;
}

void bc_text_init(AtkTextIface* iface)
{
    iface->get_text = bc_get_text;
    iface->get_character_count = bc_get_character_count;
    iface->get_character_at_offset = bc_get_character_at_offset;
    iface->get_caret_offset = bc_get_caret_offset;
    iface->set_caret_offset = bc_set_caret_offset;
    iface->get_string_at_offset = bc_get_string_at_offset;
    iface->get_text_at_offset = bc_get_text_at_offset;
    iface->get_text_before_offset = bc_get_text_before_offset;
    iface->get_text_after_offset = bc_get_text_after_offset;
    iface->get_character_extents = bc_get_character_extents;
    iface->get_offset_at_point = bc_get_offset_at_point;
    iface->get_n_selections = bc_get_n_selections;
    iface->get_selection = bc_get_selection;
    iface->set_selection = bc_set_selection;
    iface->add_selection = bc_add_selection;
    iface->remove_selection = bc_remove_selection;
    iface->get_run_attributes = bc_get_run_attributes;
}

// The get_accessible every wxWidgets window's widget class had before ours.
AtkObject* (*g_widgetGetAccessible)(GtkWidget*) = nullptr;

AtkObject* bc_get_accessible(GtkWidget* widget)
{
    if (!g_object_get_data(G_OBJECT(widget), kEditorKey) && !g_object_get_data(G_OBJECT(widget), kAccessibleKey)) {
        return g_widgetGetAccessible(widget);
    }
    AtkObject* acc = static_cast<AtkObject*>(g_object_get_data(G_OBJECT(widget), kAccessibleKey));
    if (!acc) {
        // As GTK makes a widget's own accessible: the widget given when it is made, then initialised with it.
        acc = ATK_OBJECT(g_object_new(bc_editor_accessible_get_type(), "widget", widget, nullptr));
        atk_object_initialize(acc, widget);
        g_object_set_data_full(G_OBJECT(widget), kAccessibleKey, acc, g_object_unref);
    }
    return acc;
}

AtkObject* AccessibleOf(DocEditor* e)
{
    GtkWidget* w = static_cast<GtkWidget*>(e->m_accessible);
    return w ? static_cast<AtkObject*>(g_object_get_data(G_OBJECT(w), kAccessibleKey)) : nullptr;
}

} // namespace

void DocEditor::AccessibleInit()
{
    GtkWidget* w = m_wxwindow ? m_wxwindow : m_widget;
    if (!w) return;
    m_accessible = w;
    g_object_set_data(G_OBJECT(w), kEditorKey, this);
    GtkWidgetClass* klass = GTK_WIDGET_GET_CLASS(w);
    if (klass->get_accessible != bc_get_accessible) {
        g_widgetGetAccessible = klass->get_accessible;
        klass->get_accessible = bc_get_accessible;
    }
    // Made now, so screen readers find this object from the start.
    gtk_widget_get_accessible(w);
    m_accessiblePrevious = AccessibleText();
}

void DocEditor::AccessibleDone()
{
    if (GtkWidget* w = static_cast<GtkWidget*>(m_accessible)) g_object_set_data(G_OBJECT(w), kEditorKey, nullptr);
    m_accessible = nullptr;
}

void DocEditor::AccessibleTextChanged(int)
{
    AtkObject* acc = AccessibleOf(this);
    const wxString now = AccessibleText();
    const wxString before = m_accessiblePrevious;
    m_accessiblePrevious = now;
    if (!acc) return;
    // Only the stretch that changed: what is before it and after it is the same in both.
    const size_t a = before.length(), b = now.length();
    size_t p = 0;
    while (p < a && p < b && before[p] == now[p]) ++p;
    size_t s = 0;
    while (s < a - p && s < b - p && before[a - 1 - s] == now[b - 1 - s]) ++s;
    if (a - s > p) {
        const wxScopedCharBuffer gone = before.Mid(p, a - s - p).utf8_str();
        g_signal_emit_by_name(acc, "text-remove", (gint)p, (gint)(a - s - p), gone.data());
    }
    if (b - s > p) {
        const wxScopedCharBuffer put = now.Mid(p, b - s - p).utf8_str();
        g_signal_emit_by_name(acc, "text-insert", (gint)p, (gint)(b - s - p), put.data());
    }
}

void DocEditor::AccessibleCaretMoved()
{
    AtkObject* acc = AccessibleOf(this);
    if (!acc) return;
    const int caret = OffsetOf(m_caretRow, m_caretCol);
    const int anchor = OffsetOf(m_anchorRow, m_anchorCol);
    if (caret != m_accessibleCaret) {
        m_accessibleCaret = caret;
        g_signal_emit_by_name(acc, "text-caret-moved", (gint)caret);
    }
    if (anchor != m_accessibleAnchor || (anchor != caret && caret != m_accessibleCaretSelection)) {
        const bool had = m_accessibleAnchor != m_accessibleCaretSelection;
        m_accessibleAnchor = anchor;
        m_accessibleCaretSelection = caret;
        if (had || anchor != caret) g_signal_emit_by_name(acc, "text-selection-changed");
    }
}

void DocEditor::AccessibleNameChanged()
{
    if (AtkObject* acc = AccessibleOf(this)) atk_object_set_name(acc, GetName().utf8_str());
}

#endif // __WXGTK__
