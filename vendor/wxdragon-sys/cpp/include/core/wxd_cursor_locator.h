#ifndef WXD_CURSOR_LOCATOR_H
#define WXD_CURSOR_LOCATOR_H

#include "../wxd_types.h"

// Added for BrailleCraft: shows the Cursor Locator (a rectangle closing in on the caret) over a multi-line text
// control, scrolling the caret into view first; does nothing for any other window or when the caret has no place
// on screen.
WXD_EXPORTED void wxd_TextCtrl_ShowCursorLocator(wxd_Window_t* window);

#endif // WXD_CURSOR_LOCATOR_H
