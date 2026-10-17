#ifndef WXD_DOC_EDITOR_H
#define WXD_DOC_EDITOR_H

#include "../wxd_types.h"

// Added for BrailleCraft: the document editing window, drawn by the program as the reference program's is (its
// formatted view and coded view), with its own text for screen readers on each system.
//
// The window keeps no document. The program gives it the rows to draw (text rows, and page break rows that hold no
// text), where the caret and the selection are (row and column), and the line number labels; the window draws them,
// scrolls, keeps the system caret, and hands every key, typed character and click to the program. The text screen
// readers get is the text rows, each followed by a line feed (page break rows are not in it).

#ifdef __cplusplus
extern "C" {
#endif

// A key pressed: the wxWidgets key code, the character it gives (0 for none) and the modifiers (wxMOD_ bits).
// Returns true when the program has carried it out; otherwise the window lets it go on (to menus, Tab and so on).
// A key let go comes the same way with its key code negated (the return value is not used then).
typedef bool (*wxd_DocEditor_KeyFn)(void* data, int key_code, int unicode, int modifiers);
// A character typed (after the key handler let the key go on): the character and the modifiers.
typedef void (*wxd_DocEditor_CharFn)(void* data, int unicode, int modifiers);
// The mouse: kind 0 button down, 1 drag, 2 button up, 3 double click; the row and column under it (the column can be
// past a row's end); the modifiers.
typedef void (*wxd_DocEditor_MouseFn)(void* data, int kind, int row, int col, int modifiers);
// The window got (true) or lost (false) the keyboard focus.
typedef void (*wxd_DocEditor_FocusFn)(void* data, bool focused);
// The window is being destroyed: the program frees `data`.
typedef void (*wxd_DocEditor_FreeFn)(void* data);

// Makes the window as a child of `parent`.
WXD_EXPORTED wxd_Window_t* wxd_DocEditor_Create(wxd_Window_t* parent, void* data, wxd_DocEditor_KeyFn key,
                                                wxd_DocEditor_CharFn chr, wxd_DocEditor_MouseFn mouse,
                                                wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data);

// Sets the rows: `count` NUL-terminated UTF-8 texts, and for each its kind: 0 a text row, 1 a page break row (drawn
// as a line across, no text). Replaces the earlier rows; `changed_from` is the first row that differs from before
// (screen readers are told the text changed from there), or -1 when nothing changed.
WXD_EXPORTED void wxd_DocEditor_SetRows(wxd_Window_t* editor, const char* const* rows, const int* kinds, int count,
                                        int changed_from);

// Sets the line number column: `count` labels, one per row (NULL or "" for none). `width` 0 hides the column.
WXD_EXPORTED void wxd_DocEditor_SetLineNumbers(wxd_Window_t* editor, const char* const* labels, int count, int width);

// Puts the caret at `row`, `col` and the selection's other end at `anchor_row`, `anchor_col` (the same place for no
// selection); then scrolls so the caret row is shown (keeping it `keep_from_top` rows from the top when that is
// zero or more). Screen readers are told the caret moved.
WXD_EXPORTED void wxd_DocEditor_SetCaret(wxd_Window_t* editor, int row, int col, int anchor_row, int anchor_col,
                                         int keep_from_top);

// The first row shown and the number of whole rows the window shows.
WXD_EXPORTED int wxd_DocEditor_GetTopRow(wxd_Window_t* editor);
WXD_EXPORTED int wxd_DocEditor_GetVisibleRows(wxd_Window_t* editor);
// Scrolls so `row` is the first row shown.
WXD_EXPORTED void wxd_DocEditor_SetTopRow(wxd_Window_t* editor, int row);

// The caret row's place from the top of the window, in rows (for a view switch that keeps it, spec 27 2c).
WXD_EXPORTED int wxd_DocEditor_GetCaretScreenRow(wxd_Window_t* editor);

// The caret's rectangle in screen coordinates (x, y, width, height), for the Cursor Locator.
WXD_EXPORTED void wxd_DocEditor_GetCaretRect(wxd_Window_t* editor, int* x, int* y, int* w, int* h);

// Shows the Cursor Locator: a rectangle closing in on the caret, drawn in the window (spec 24 2d.2).
WXD_EXPORTED void wxd_DocEditor_ShowCursorLocator(wxd_Window_t* editor);

// The screen reader name of the window (the document name).
WXD_EXPORTED void wxd_DocEditor_SetAccessibleName(wxd_Window_t* editor, const char* name);

// Global, Internationalization, Support UI Automation: on Windows the editing windows answer UI Automation clients
// and tell them of changes only while it is on (spec 15). Elsewhere it does nothing.
WXD_EXPORTED void wxd_DocEditor_SetSupportUIAutomation(bool on);

#ifdef __cplusplus
}
#endif

#endif // WXD_DOC_EDITOR_H
