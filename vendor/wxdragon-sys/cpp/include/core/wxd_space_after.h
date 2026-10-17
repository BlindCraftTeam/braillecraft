#ifndef WXD_SPACE_AFTER_H
#define WXD_SPACE_AFTER_H

#include "../wxd_types.h"

// Added for BrailleCraft: leaves `pixels` of empty space after the line (paragraph) holding positions `from` to
// `to` of a multi-line text control, with no text in it: through the text view's own spacing on GTK and macOS, and
// through the paragraph spacing wxWidgets sets on Windows. Does nothing for any other window.
WXD_EXPORTED void wxd_TextCtrl_SetSpaceAfter(wxd_Window_t* window, long from, long to, int pixels);

#endif // WXD_SPACE_AFTER_H
