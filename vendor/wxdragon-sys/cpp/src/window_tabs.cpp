// Added for BrailleCraft: the window tab functions (wxd_window_tabs.h) on the systems other than macOS, where the
// documents are the pages of the tab bar (doc_tabs.cpp) and there are no window tabs: each does nothing. On macOS
// window_tabs_osx.mm has them.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include "../include/wxdragon.h"

#if !defined(__WXOSX__)

extern "C" {

WXD_EXPORTED void wxd_WindowTabs_Enable(wxd_Window_t*) {}

WXD_EXPORTED void wxd_WindowTabs_Prepare(wxd_Window_t*, void*, wxd_WindowTabs_NewTabFn) {}

WXD_EXPORTED bool wxd_WindowTabs_Add(wxd_Window_t*, wxd_Window_t*) { return false; }

WXD_EXPORTED void wxd_WindowTabs_ShowTabBar(wxd_Window_t*) {}

WXD_EXPORTED void wxd_WindowTabs_Select(wxd_Window_t*) {}

WXD_EXPORTED bool wxd_WindowTabs_Step(wxd_Window_t*, bool) { return false; }

WXD_EXPORTED bool wxd_WindowTabs_Move(wxd_Window_t*, int) { return false; }

WXD_EXPORTED int wxd_WindowTabs_GroupOrder(wxd_Window_t*, wxd_Window_t**, int) { return 0; }

WXD_EXPORTED void wxd_WindowTabs_SetEdited(wxd_Window_t*, bool) {}

WXD_EXPORTED bool wxd_WindowTabs_IsKey(wxd_Window_t*) { return false; }

WXD_EXPORTED int wxd_WindowTabs_MenuDump(char* out, int size)
{
    if (out && size > 0) out[0] = '\0';
    return 0;
}

}

#endif
