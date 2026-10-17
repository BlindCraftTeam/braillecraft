// Added for BrailleCraft: empty space after a line of a multi-line text control (wxd_space_after.h). wxWidgets
// applies a paragraph's spacing after only on Windows; on GTK the text view's "pixels-below-lines" tag gives the
// same, and on macOS the text view's paragraph spacing (space_after_osx.mm).

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/textctrl.h>
#include "../include/wxdragon.h"

#ifdef __WXGTK__
#include <gtk/gtk.h>
#endif

#ifdef __WXOSX__
void wxd_osx_SetSpaceAfter(wxTextCtrl* text, long from, long to, int pixels);
#endif

void wxd_TextCtrl_SetSpaceAfter(wxd_Window_t* window, long from, long to, int pixels)
{
    wxTextCtrl* text = wxDynamicCast(reinterpret_cast<wxWindow*>(window), wxTextCtrl);
    if (!text || !text->IsMultiLine() || to < from) return;
#if defined(__WXGTK__)
    GtkTextBuffer* buffer = text->GTKGetTextBuffer();
    if (!buffer) return;
    GtkTextTag* tag = gtk_text_buffer_create_tag(buffer, nullptr, "pixels-below-lines", pixels, nullptr);
    GtkTextIter start, end;
    gtk_text_buffer_get_iter_at_offset(buffer, &start, (gint)from);
    gtk_text_buffer_get_iter_at_offset(buffer, &end, (gint)to);
    gtk_text_buffer_apply_tag(buffer, tag, &start, &end);
#elif defined(__WXOSX__)
    wxd_osx_SetSpaceAfter(text, from, to, pixels);
#else
    // Paragraph spacing in tenths of a millimetre, from pixels at the screen's resolution.
    const int dpi = wxMax(text->GetDPI().y, 1);
    wxTextAttr attr;
    attr.SetParagraphSpacingAfter(pixels * 254 / dpi);
    text->SetStyle(from, to, attr);
#endif
}
