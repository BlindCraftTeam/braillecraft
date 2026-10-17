// Added for BrailleCraft: Save As with the file's encoding chosen in the dialog itself, as the original program
// shows its encoding box inside its Save As dialog for formatted braille and formatted text.
//
// The label and the choice are added through wxWidgets' file dialog customisation, which every system supports.
// Windows and macOS tell the customisation each time the file type changes; GTK does not, so on Linux the file
// chooser's own "filter" property is watched and the type found from its list of filters.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/filedlg.h>
#include <wx/filedlgcustomize.h>
#include <wx/tokenzr.h>
#include <algorithm>
#include <cstring>
#include <vector>
#include "../include/wxdragon.h"

#if defined(__WXGTK__)
#include <gtk/gtk.h>
#endif

namespace {

class ChoiceHook : public wxFileDialogCustomizeHook {
public:
    ChoiceHook(const wxString& label, const wxArrayString& items, std::vector<int> shown, int first)
        : m_labelText(label), m_items(items), m_shown(std::move(shown)), m_type(first) {}

    void AddCustomControls(wxFileDialogCustomize& customizer) override {
        m_label = customizer.AddStaticText(m_labelText);
        m_choice = customizer.AddChoice(m_items.size(), m_items.empty() ? nullptr : &m_items[0]);
        if (!m_items.empty()) m_choice->SetSelection(0);
        Show(m_type);
    }


    void UpdateCustomControls() override {
        // Windows and macOS: the type now chosen (GTK reports none here and is told through `Show`).
        if (m_dialog) {
            const int type = m_dialog->GetCurrentlySelectedFilterIndex();
            if (type != wxNOT_FOUND) Show(type);
        }
    }

    void TransferDataFromCustomControls() override {
        m_item = m_choice ? m_choice->GetSelection() : 0;
    }

    // The label and the choice shown for the types that have them, hidden for the others.
    void Show(int type) {
        m_type = type;
        const bool on = std::find(m_shown.begin(), m_shown.end(), type) != m_shown.end();
        if (m_label) m_label->Show(on);
        if (m_choice) m_choice->Show(on);
#if defined(__WXGTK__)
        // wxWidgets sizes the panel holding the controls only when it makes it, so on GTK (where the controls are
        // shown and hidden after that) it is fitted to them again and GTK lays it out again.
        if (wxWindow* panel = m_dialog ? m_dialog->GetExtraControl() : nullptr) {
            if (wxSizer* sizer = panel->GetSizer()) {
                panel->SetClientSize(sizer->ComputeFittingClientSize(panel));
                sizer->Layout();
            }
            if (panel->m_widget) gtk_widget_queue_resize(panel->m_widget);
        }
#endif
    }

    wxFileDialog* m_dialog = nullptr;
    int m_item = 0;

private:
    wxString m_labelText;
    wxArrayString m_items;
    std::vector<int> m_shown;
    int m_type;
    wxFileDialogStaticText* m_label = nullptr;
    wxFileDialogChoice* m_choice = nullptr;
};

#if defined(__WXGTK__)
// The position of the chooser's filter in its list of filters: the type chosen.
void OnFilterChanged(GObject* object, GParamSpec*, gpointer data) {
    GtkFileChooser* chooser = GTK_FILE_CHOOSER(object);
    GtkFileFilter* filter = gtk_file_chooser_get_filter(chooser);
    GSList* filters = gtk_file_chooser_list_filters(chooser);
    const int type = filter ? g_slist_index(filters, filter) : -1;
    g_slist_free(filters);
    if (type >= 0) static_cast<ChoiceHook*>(data)->Show(type);
}

#endif

} // namespace

int wxd_SaveDialog_WithChoice(wxd_Window_t* parent, const char* title, const char* wildcard, const char* default_dir,
                              const char* default_file, int type_index, const char* label, const char* items,
                              const int* shown_for, int shown_count, char* path, int path_size, int* type_out,
                              int* item_out) {
    wxArrayString list = wxStringTokenize(wxString::FromUTF8(items ? items : ""), "\n", wxTOKEN_RET_EMPTY_ALL);
    if (!list.empty() && list.back().empty()) list.pop_back();
    std::vector<int> shown(shown_for, shown_for + std::max(shown_count, 0));
    wxFileDialog dialog(reinterpret_cast<wxWindow*>(parent), wxString::FromUTF8(title ? title : ""),
                        wxString::FromUTF8(default_dir ? default_dir : ""),
                        wxString::FromUTF8(default_file ? default_file : ""),
                        wxString::FromUTF8(wildcard ? wildcard : ""), wxFD_SAVE | wxFD_OVERWRITE_PROMPT);
    dialog.SetFilterIndex(type_index);
    ChoiceHook hook(wxString::FromUTF8(label ? label : ""), list, shown, type_index);
    hook.m_dialog = &dialog;
    dialog.SetCustomizeHook(hook);
#if defined(__WXGTK__)
    if (dialog.m_widget && GTK_IS_FILE_CHOOSER(dialog.m_widget)) {
        g_signal_connect(G_OBJECT(dialog.m_widget), "notify::filter", G_CALLBACK(OnFilterChanged), &hook);
    }
#endif
    const int answer = dialog.ShowModal();
#if defined(__WXGTK__)
    if (dialog.m_widget) {
        g_signal_handlers_disconnect_by_data(G_OBJECT(dialog.m_widget), &hook);
    }
#endif
    if (answer != wxID_OK) return 0;
    // The path is kept in a string of its own: in a UTF-8 build of wxWidgets the buffer points into that string.
    const wxString file = dialog.GetPath();
    const wxScopedCharBuffer chosen = file.utf8_str();
    if (path && path_size > 0) {
        std::strncpy(path, chosen.data(), static_cast<size_t>(path_size) - 1);
        path[path_size - 1] = '\0';
    }
    if (type_out) *type_out = dialog.GetFilterIndex();
    if (item_out) *item_out = hook.m_item;
    return 1;
}
