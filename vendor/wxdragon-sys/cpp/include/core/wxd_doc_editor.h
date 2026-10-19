#ifndef WXD_DOC_EDITOR_H
#define WXD_DOC_EDITOR_H

#include "../wxd_types.h"

// Added for BrailleCraft: the document editing window, drawn by the program (its
// formatted view and coded view), with its own text for screen readers on each system.
//
// The window keeps no document and no row text. The program tells it how many rows there are and the kind and
// length of each (text rows, and page break rows that hold no text) and where the caret and the selection are (row
// and column); the window asks the program for a row's text as it draws it, measures it or gives it to a screen
// reader, keeping a bounded number of rows (a book of thousands of pages is not held a second time in the window:
// plan line L1); it scrolls, keeps the system caret, and hands every key, typed character and click to the
// program. The text screen readers get is the text rows, each followed by a line feed (page break rows are not in
// it).

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
// A row asked of the program: `which` 0 is the row's text as drawn (in it, a code of the coded view stands between
// the characters U+E000 and U+E001: the window takes the markers out and draws the code in the code font inside a
// box, spec 27 4a; the markers do not count as columns), 1 the text screen readers get for the row (the same length,
// no markers: braille may be written in another form for screen readers than on screen), 2 the row's line number
// label ("" for none). The answer is NUL-terminated UTF-8 and stays valid until the next call. NULL means the program
// cannot answer now (it is busy giving the window its rows): the window then draws nothing for the row and keeps
// nothing, and asks again later.
typedef const char* (*wxd_DocEditor_RowFn)(void* data, int row, int which);

// Makes the window as a child of `parent`.
WXD_EXPORTED wxd_Window_t* wxd_DocEditor_Create(wxd_Window_t* parent, void* data, wxd_DocEditor_KeyFn key,
                                                wxd_DocEditor_CharFn chr, wxd_DocEditor_MouseFn mouse,
                                                wxd_DocEditor_FocusFn focus, wxd_DocEditor_FreeFn free_data,
                                                wxd_DocEditor_RowFn row);

// Sets the rows: `count` rows, with for each its kind (`kinds`: 0 a text row, 1 a page break row, drawn as a line
// across with no text) and the length in characters of its text (`lengths`: 0 for a page break row); `longest` is
// the longest row's length, for the window's width. The rows the window kept are let go; it asks the program for
// each row again as it needs it. Screen readers are told the text changed when `changed_at` is 0 or more: at that
// offset (in characters) of the text they get, `removed` went out and `inserted` came in (UTF-8; NULL for none).
WXD_EXPORTED void wxd_DocEditor_SetRows(wxd_Window_t* editor, int count, const unsigned char* kinds,
                                        const int* lengths, int longest, int changed_at, const char* removed,
                                        const char* inserted);

// Shows the line number column `width` wide (the labels come from the row function), or hides it (`width` 0).
WXD_EXPORTED void wxd_DocEditor_SetLineNumbers(wxd_Window_t* editor, int width);

// Puts the caret at `row`, `col` and the selection's other end at `anchor_row`, `anchor_col` (the same place for no
// selection); then scrolls so the caret row is shown (keeping it `keep_from_top` rows from the top when that is
// zero or more). `caret_row_text` is the caret row's text as drawn (as the row function gives it), kept for the row,
// since the program may be unable to answer for it while it places the caret. Screen readers are told the caret
// moved.
WXD_EXPORTED void wxd_DocEditor_SetCaret(wxd_Window_t* editor, int row, int col, int anchor_row, int anchor_col,
                                         int keep_from_top, const char* caret_row_text);

// The physical key of the key pressed or let go that is being handed to the program (six-key entry names keys by
// their place): on Windows its scan code (bits 16 to 23 of the key message's flags), on Linux the hardware key code
// less 8 (the evdev key code, the same number as the scan code for the main keys), on macOS the virtual key code.
WXD_EXPORTED int wxd_DocEditor_GetRawKey(wxd_Window_t* editor);

// The first row shown and the number of whole rows the window shows.
WXD_EXPORTED int wxd_DocEditor_GetTopRow(wxd_Window_t* editor);
WXD_EXPORTED int wxd_DocEditor_GetVisibleRows(wxd_Window_t* editor);
// Scrolls so `row` is the first row shown.
WXD_EXPORTED void wxd_DocEditor_SetTopRow(wxd_Window_t* editor, int row);

// The caret row's place from the top of the window, in rows (for a view switch that keeps it, spec 27 2c).
WXD_EXPORTED int wxd_DocEditor_GetCaretScreenRow(wxd_Window_t* editor);

// The caret's rectangle in screen coordinates (x, y, width, height), for the Cursor Locator.
WXD_EXPORTED void wxd_DocEditor_GetCaretRect(wxd_Window_t* editor, int* x, int* y, int* w, int* h);

// Handles the events waiting (input included), for a window that stays open while the program works, such as the
// embossing progress window with its Cancel button.
WXD_EXPORTED void wxd_App_HandlePendingEvents(void);

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
