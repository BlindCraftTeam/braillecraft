#ifndef WXD_WINDOW_TABS_H
#define WXD_WINDOW_TABS_H

#include "../wxd_types.h"

// Added for BrailleCraft: the open documents as the system's own window tabs on macOS. Each document is a frame of
// its own; the document frames share one tabbing identifier, so AppKit shows them as the tabs of one window (its tab
// bar, the close button on each tab, the plus button, Show Next Tab and the other Window menu items) and VoiceOver
// reads the tab bar as a tab group with the selected tab marked. The program's own frame, which holds the menu bar,
// is never shown.
//
// The functions do their work on macOS only (cpp/src/window_tabs_osx.mm); elsewhere they do nothing and answer
// false or 0 (cpp/src/window_tabs.cpp), so the program can call them on every system.

#ifdef __cplusplus
extern "C" {
#endif

// The plus button of a document window's tab bar was pressed.
typedef void (*wxd_WindowTabs_NewTabFn)(void* data);

// Window tabbing on for the program (wxWidgets turns it off at launch); the program's own default that keeps the
// system from tabbing its other windows (dialogs) together; the program frame never tabbed, and its menu bar kept as
// the menu bar shown while no document window is open.
WXD_EXPORTED void wxd_WindowTabs_Enable(wxd_Window_t* program_frame);

// Makes `frame` a document window: the tabbing identifier the document windows share, and the plus button
// (newWindowForTab:) calling `new_tab(data)`. Called before the frame is first shown.
WXD_EXPORTED void wxd_WindowTabs_Prepare(wxd_Window_t* frame, void* data, wxd_WindowTabs_NewTabFn new_tab);

// Adds `frame` as the last tab of the tab group of `into` (a shown document window); false when `into` is null or
// not a document window (the frame then opens as a window of its own).
WXD_EXPORTED bool wxd_WindowTabs_Add(wxd_Window_t* into, wxd_Window_t* frame);

// Shows the tab bar of `frame`'s window when it is hidden (once, for the first document).
WXD_EXPORTED void wxd_WindowTabs_ShowTabBar(wxd_Window_t* frame);

// Selects the tab of `frame` and makes its window key (the activation is reported by wxWidgets).
WXD_EXPORTED void wxd_WindowTabs_Select(wxd_Window_t* frame);

// Selects the next (forward) or previous tab of `frame`'s tab group, wrapping; false when the group has fewer than
// two tabs (nothing was done).
WXD_EXPORTED bool wxd_WindowTabs_Step(wxd_Window_t* frame, bool forward);

// Moves `frame` to place `to` in its tab group, keeping the selected tab selected; false when out of range.
WXD_EXPORTED bool wxd_WindowTabs_Move(wxd_Window_t* frame, int to);

// The document frames of `frame`'s tab group in tab order: fills up to `max` of `out` and answers how many there are
// (1 when the window has no tab group of its own, 0 when `frame` is not a window).
WXD_EXPORTED int wxd_WindowTabs_GroupOrder(wxd_Window_t* frame, wxd_Window_t** out, int max);

// The edited mark in the close button of the window and of its tab (wxTopLevelWindow::OSXSetModified).
WXD_EXPORTED void wxd_WindowTabs_SetEdited(wxd_Window_t* frame, bool edited);

// Whether `frame`'s window is the key window.
WXD_EXPORTED bool wxd_WindowTabs_IsKey(wxd_Window_t* frame);

// For the build checks: every item of the main menu, one "Menu > Item" line each (nested menus as "Menu > Sub >
// Item"), as UTF-8 written into `out` (at most `size` bytes with the closing zero); answers the length the whole
// text needs (0 elsewhere than macOS).
WXD_EXPORTED int wxd_WindowTabs_MenuDump(char* out, int size);

#ifdef __cplusplus
}
#endif

#endif // WXD_WINDOW_TABS_H
