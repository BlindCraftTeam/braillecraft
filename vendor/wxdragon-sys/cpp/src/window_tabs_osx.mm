// Added for BrailleCraft: the open documents as the system's own window tabs on macOS (wxd_window_tabs.h).
//
// wxWidgets turns window tabbing off for the whole program at launch and has nothing for tab groups, so this file
// does it with AppKit: each document frame's window gets the tabbing identifier the document windows share and is
// added to the tab group of the window in front; the tab bar, its close buttons, the Window menu's tab items and
// VoiceOver's reading of the tab group are then AppKit's own. The plus button of the tab bar appears when something
// in the window's responder chain answers newWindowForTab:, which wxWidgets' window class does not; the method is
// added to that class at run time and does nothing for a window that is not a document window.
//
// Selecting a tab makes its window key, which wxWidgets reports as the frame's activation; closing a tab (its close
// button, Cmd+W) asks the frame to close, which wxWidgets reports as the frame's close event. So the program hears
// of both through wxWidgets. A tab dragged along the bar is reported by nothing: the program reads the tab order
// (wxd_WindowTabs_GroupOrder) when it needs it.

#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#include <algorithm>
#include <cstring>
#include <string>
#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/nonownedwnd.h>
#include <wx/toplevel.h>
#include <wx/frame.h>
#include <wx/menu.h>
#include "../include/wxdragon.h"

// The program runs on macOS 11 and later (packaging/macos/Info.plist), and every AppKit call here is of macOS 10.13
// or earlier (tab groups); the library is compiled for an older minimum (wxWidgets' default), so the availability
// warnings are off for this file.
#pragma clang diagnostic ignored "-Wunguarded-availability"
#pragma clang diagnostic ignored "-Wunguarded-availability-new"

static NSString* const kIdentifier = @"BrailleCraftDocument";
static char kCallbackKey;

// What the plus button calls, kept on the document window.
@interface BCWindowTabCallback : NSObject
@property (nonatomic) void* data;
@property (nonatomic) wxd_WindowTabs_NewTabFn fn;
@end

@implementation BCWindowTabCallback
@end

static NSWindow* WindowOf(wxd_Window_t* w)
{
    wxNonOwnedWindow* tlw = wxDynamicCast(reinterpret_cast<wxWindow*>(w), wxNonOwnedWindow);
    return tlw ? tlw->GetWXWindow() : nil;
}

static bool IsDocumentWindow(NSWindow* w)
{
    return w && [w.tabbingIdentifier isEqualToString:kIdentifier]
        && objc_getAssociatedObject(w, &kCallbackKey) != nil;
}

// newWindowForTab: as added to wxWidgets' window class.
static void NewWindowForTab(id self, SEL, id)
{
    BCWindowTabCallback* cb = objc_getAssociatedObject(self, &kCallbackKey);
    if (cb && cb.fn) cb.fn(cb.data);
}

extern "C" {

WXD_EXPORTED void wxd_WindowTabs_Enable(wxd_Window_t* program_frame)
{
    wxTheApp->OSXEnableAutomaticTabbing(true);
    // Only the windows the program tabs itself are tabbed: for a user whose system setting is to prefer tabs
    // always, the program's dialogs would otherwise be tabbed together.
    [[NSUserDefaults standardUserDefaults] setObject:@"manual" forKey:@"AppleWindowTabbingMode"];
    NSWindow* w = WindowOf(program_frame);
    if (w) w.tabbingMode = NSWindowTabbingModeDisallowed;
    wxFrame* f = wxDynamicCast(reinterpret_cast<wxWindow*>(program_frame), wxFrame);
    if (f && f->GetMenuBar()) wxMenuBar::MacSetCommonMenuBar(f->GetMenuBar());
}

WXD_EXPORTED void wxd_WindowTabs_Prepare(wxd_Window_t* frame, void* data, wxd_WindowTabs_NewTabFn new_tab)
{
    NSWindow* w = WindowOf(frame);
    if (!w) return;
    w.tabbingIdentifier = kIdentifier;
    w.tabbingMode = NSWindowTabbingModePreferred;
    BCWindowTabCallback* cb = [BCWindowTabCallback new];
    cb.data = data;
    cb.fn = new_tab;
    objc_setAssociatedObject(w, &kCallbackKey, cb, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    Class c = object_getClass(w);
    if (![c instancesRespondToSelector:@selector(newWindowForTab:)])
        class_addMethod(c, @selector(newWindowForTab:), (IMP)NewWindowForTab, "v@:@");
}

WXD_EXPORTED bool wxd_WindowTabs_Add(wxd_Window_t* into, wxd_Window_t* frame)
{
    NSWindow* a = WindowOf(into);
    NSWindow* b = WindowOf(frame);
    if (!a || !b || a == b || !IsDocumentWindow(a) || ![a isVisible]) return false;
    // After the last tab of the group, as the program's list puts a new document last.
    NSWindow* last = a.tabGroup.windows.lastObject;
    if (!last) last = a;
    [last addTabbedWindow:b ordered:NSWindowAbove];
    return true;
}

WXD_EXPORTED void wxd_WindowTabs_ShowTabBar(wxd_Window_t* frame)
{
    NSWindow* w = WindowOf(frame);
    if (w && w.tabGroup && ![w.tabGroup isTabBarVisible]) [w toggleTabBar:nil];
}

WXD_EXPORTED void wxd_WindowTabs_Select(wxd_Window_t* frame)
{
    NSWindow* w = WindowOf(frame);
    if (!w) return;
    if (w.tabGroup) w.tabGroup.selectedWindow = w;
    [w makeKeyAndOrderFront:nil];
}

WXD_EXPORTED bool wxd_WindowTabs_Step(wxd_Window_t* frame, bool forward)
{
    NSWindow* w = WindowOf(frame);
    if (!w || !w.tabGroup || w.tabGroup.windows.count < 2) return false;
    if (forward) {
        [w selectNextTab:nil];
    } else {
        [w selectPreviousTab:nil];
    }
    return true;
}

WXD_EXPORTED bool wxd_WindowTabs_Move(wxd_Window_t* frame, int to)
{
    NSWindow* w = WindowOf(frame);
    if (!w) return false;
    NSWindowTabGroup* g = w.tabGroup;
    if (!g || to < 0 || to >= (int)g.windows.count) return false;
    const NSUInteger at = [g.windows indexOfObject:w];
    if (at == NSNotFound || (int)at == to) return false;
    NSWindow* selected = g.selectedWindow;
    [g removeWindow:w];
    [g insertWindow:w atIndex:(NSInteger)to];
    if (selected) {
        g.selectedWindow = selected;
        [selected makeKeyAndOrderFront:nil];
    }
    return true;
}

WXD_EXPORTED int wxd_WindowTabs_GroupOrder(wxd_Window_t* frame, wxd_Window_t** out, int max)
{
    NSWindow* w = WindowOf(frame);
    if (!w) return 0;
    NSArray<NSWindow*>* list = w.tabGroup ? w.tabGroup.windows : nil;
    if (!list || list.count == 0) list = @[ w ];
    int n = 0;
    for (NSWindow* x in list) {
        wxNonOwnedWindow* peer = wxNonOwnedWindow::GetFromWXWindow(x);
        if (!peer) continue;
        if (out && n < max) out[n] = reinterpret_cast<wxd_Window_t*>(static_cast<wxWindow*>(peer));
        ++n;
    }
    return n;
}

WXD_EXPORTED void wxd_WindowTabs_SetEdited(wxd_Window_t* frame, bool edited)
{
    wxTopLevelWindow* t = wxDynamicCast(reinterpret_cast<wxWindow*>(frame), wxTopLevelWindow);
    if (t) t->OSXSetModified(edited);
}

WXD_EXPORTED bool wxd_WindowTabs_IsKey(wxd_Window_t* frame)
{
    NSWindow* w = WindowOf(frame);
    return w && [w isKeyWindow];
}

}

static void DumpMenu(NSMenu* menu, const std::string& path, std::string& out)
{
    for (NSMenuItem* item in menu.itemArray) {
        if ([item isSeparatorItem]) continue;
        const char* t = item.title.UTF8String;
        const std::string name = path + " > " + (t ? t : "");
        if (item.submenu) {
            DumpMenu(item.submenu, name, out);
        } else {
            out += name;
            out += "\n";
        }
    }
}

extern "C" WXD_EXPORTED int wxd_WindowTabs_MenuDump(char* out, int size)
{
    std::string text;
    NSMenu* main = [NSApp mainMenu];
    for (NSMenuItem* top in main.itemArray) {
        if (!top.submenu) continue;
        // The menu's own title: the top item's title is empty for the application menu.
        const char* t = top.submenu.title.UTF8String;
        DumpMenu(top.submenu, t ? t : "", text);
    }
    if (out && size > 0) {
        const size_t n = std::min(text.size(), (size_t)size - 1);
        std::memcpy(out, text.data(), n);
        out[n] = '\0';
    }
    return (int)text.size();
}
