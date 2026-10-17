#ifndef WXD_LINE_GUTTER_H
#define WXD_LINE_GUTTER_H

#include "../wxd_types.h"

// Added for BrailleCraft: a narrow drawn column beside a multi-line text control that shows a label (a line number)
// at the height of given text positions, following the control's scrolling. It draws only; it has no text of its own
// for screen readers, as a drawn line number has none.

// Makes the column as a child of `parent`, for the text control `text`, `width` pixels wide. Returns the window.
WXD_EXPORTED wxd_Window_t* wxd_LineGutter_Create(wxd_Window_t* parent, wxd_Window_t* text, int width);

// Sets the labels: `count` text positions and, for each, a NUL-terminated UTF-8 label. Replaces the earlier ones.
WXD_EXPORTED void wxd_LineGutter_SetMarks(wxd_Window_t* gutter, const long* positions, const char* const* labels, int count);

#endif // WXD_LINE_GUTTER_H
