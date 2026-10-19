#ifndef WXD_DOC_TABS_H
#define WXD_DOC_TABS_H

#include "../wxd_types.h"

// Added for BrailleCraft: the tab bar of the main window on Windows and Linux, one tab per open document (a
// wxNotebook with the behaviour of a browser's tab bar): a close button on each tab (a real button in the tab on GTK;
// on Windows the tabs are drawn by the program with a close sign at the right of the text, which closes the tab when
// the click is let go on it, and a tab has a context menu: Close Tab, Move Tab Left, Move Tab Right; a middle click
// closes a tab everywhere), tabs dragged to reorder (GTK's own reorderable tabs, a drag over the tab row elsewhere),
// and the keys Ctrl+Tab and Ctrl+Shift+Tab (next and previous tab), Ctrl+W (close) and Ctrl+Shift+Page Up and Page
// Down (move the tab) while the tab bar itself has the focus. The pages are the program's windows; the notebook is
// read by screen readers as the system's tab control (a page tab list on AT-SPI, a tab control on UI Automation).
// On macOS the documents are window tabs instead (wxd_window_tabs.h).

#ifdef __cplusplus
extern "C" {
#endif

// The selected tab changed (by the user, or by wxd_DocTabs_SetSelection): the index now selected.
typedef void (*wxd_DocTabs_ChangedFn)(void* data, int index);
// The user asked for tab `index` to close (its close button, a middle click on it, Ctrl+W).
typedef void (*wxd_DocTabs_CloseFn)(void* data, int index);
// The tab at `from` was moved to `to` (a drag, Ctrl+Shift+Page Up or Page Down, or wxd_DocTabs_MovePage); the
// other tabs shift to make room, as a list element moved does.
typedef void (*wxd_DocTabs_MovedFn)(void* data, int from, int to);
// The tab bar is being destroyed: the program frees `data`.
typedef void (*wxd_DocTabs_FreeFn)(void* data);

// Makes the tab bar as a child of `parent`.
WXD_EXPORTED wxd_Window_t* wxd_DocTabs_Create(wxd_Window_t* parent, void* data, wxd_DocTabs_ChangedFn changed,
                                              wxd_DocTabs_CloseFn close, wxd_DocTabs_MovedFn moved,
                                              wxd_DocTabs_FreeFn free_data);

// Adds `page` (a window made as a child of the tab bar) as the last tab with the tab text `text` (UTF-8), selected
// when `select`. Returns its index.
WXD_EXPORTED int wxd_DocTabs_AddPage(wxd_Window_t* tabs, wxd_Window_t* page, const char* text, bool select);

// Sets the tab text of page `index`.
WXD_EXPORTED void wxd_DocTabs_SetPageText(wxd_Window_t* tabs, int index, const char* text);

// The index of the page window `page`, or -1.
WXD_EXPORTED int wxd_DocTabs_FindPage(wxd_Window_t* tabs, wxd_Window_t* page);

// The selected tab, or -1 when there is none.
WXD_EXPORTED int wxd_DocTabs_GetSelection(wxd_Window_t* tabs);
// Selects tab `index`, reporting it through the changed callback.
WXD_EXPORTED void wxd_DocTabs_SetSelection(wxd_Window_t* tabs, int index);
// Selects tab `index` without the callback.
WXD_EXPORTED void wxd_DocTabs_ChangeSelection(wxd_Window_t* tabs, int index);
// How many tabs there are.
WXD_EXPORTED int wxd_DocTabs_GetPageCount(wxd_Window_t* tabs);
// Takes tab `index` out; its page window is kept (the program destroys it). Another tab is selected when the
// selected one went.
WXD_EXPORTED bool wxd_DocTabs_RemovePage(wxd_Window_t* tabs, int index);
// Moves tab `from` to `to`, keeping the selected document selected; reported through the moved callback.
WXD_EXPORTED bool wxd_DocTabs_MovePage(wxd_Window_t* tabs, int from, int to);

// The screen rectangle of tab `index` (its label), for a program that drives the window; false where the system
// does not tell it.
WXD_EXPORTED bool wxd_DocTabs_TabRect(wxd_Window_t* tabs, int index, int* x, int* y, int* w, int* h);
// The screen rectangle of the close button of tab `index` (on Windows the drawn close sign at the right of the
// text); false where the tabs have none.
WXD_EXPORTED bool wxd_DocTabs_CloseButtonRect(wxd_Window_t* tabs, int index, int* x, int* y, int* w, int* h);
// Puts the keyboard focus on the tab bar with the keyboard cues on (Windows: the focus rectangle shows), for a
// program that drives the window.
WXD_EXPORTED void wxd_DocTabs_ShowFocus(wxd_Window_t* tabs);

#ifdef __cplusplus
}
#endif

#endif // WXD_DOC_TABS_H
