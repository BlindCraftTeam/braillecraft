// Added for BrailleCraft: a picture of the program's windows (wxd_screenshot.h).
//
// The area is the union of the program's windows on the screen, clipped to the screen. How the pixels are read
// depends on the system. On Linux under X (GTK with the X11 backend) the program's top-level X windows are found
// through the X server itself, so the native file chooser the toolkit does not list is seen too; the screen region
// is read from the root window and each of the program's windows is then read by itself and laid over it (a window
// with an alpha channel comes out black in the root image, and a window under a dialog is read stale through GDK).
// On Linux without X each toolkit window is read through GDK and composed on a background. On Windows the screen
// is read through wxScreenDC. On macOS the system's screencapture tool writes the region, as the system's own
// screenshot keys do.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/dcscreen.h>
#include <wx/dcmemory.h>
#include <wx/image.h>
#include <wx/toplevel.h>
#include <wx/utils.h>
#include "../include/wxdragon.h"

#if defined(__WXGTK__)
#include <gtk/gtk.h>
#include <gdk/gdk.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <X11/Xutil.h>
#include <unistd.h>
#include <vector>
#endif
#endif

namespace {

// Whether a top-level window is on the screen. On GTK the native message box runs without the library's own shown
// flag being set, so the widget's mapped state is what counts there.
bool Visible(wxTopLevelWindow* w) {
    if (!w || w->IsIconized()) return false;
#if defined(__WXGTK__)
    GtkWidget* widget = w->GetHandle();
    return widget && gtk_widget_get_mapped(widget);
#else
    return w->IsShown();
#endif
}

wxRect ScreenRect() {
    return wxRect(0, 0, wxSystemSettings::GetMetric(wxSYS_SCREEN_X), wxSystemSettings::GetMetric(wxSYS_SCREEN_Y));
}

// The shown toolkit windows and the screen area they cover together.
bool WindowsBox(wxRect& box) {
    bool any = false;
    for (wxWindowList::const_iterator it = wxTopLevelWindows.begin(); it != wxTopLevelWindows.end(); ++it) {
        wxTopLevelWindow* w = wxDynamicCast(*it, wxTopLevelWindow);
        if (!Visible(w)) continue;
        const wxRect r = w->GetScreenRect();
        if (r.width <= 0 || r.height <= 0) continue;
        if (!any) {
            box = r;
            any = true;
        } else {
            box.Union(r);
        }
    }
    if (!any) return false;
    box.Intersect(ScreenRect());
    return !box.IsEmpty();
}

void EnsurePng() {
    if (!wxImage::FindHandler(wxBITMAP_TYPE_PNG)) wxImage::AddHandler(new wxPNGHandler);
}

#if defined(__WXGTK__) && defined(GDK_WINDOWING_X11)

// A channel of a pixel under a visual's mask, as 8 bits.
unsigned char Channel(unsigned long pixel, unsigned long mask) {
    if (!mask) return 0;
    int shift = 0, bits = 0;
    unsigned long m = mask;
    while (!(m & 1)) {
        m >>= 1;
        ++shift;
    }
    while (m & 1) {
        m >>= 1;
        ++bits;
    }
    unsigned long v = (pixel & mask) >> shift;
    if (bits >= 8) return (unsigned char)(v >> (bits - 8));
    return (unsigned char)(v << (8 - bits));
}

struct XWin {
    Window id;
    int x, y, width, height;
};

// The viewable top-level X windows of this process (the ones carrying its _NET_WM_PID), bottom to top as the X
// server stacks them; the window manager's frames are walked through to the client windows inside them.
void CollectWindows(Display* dpy, Window win, Atom pidAtom, unsigned long pid, int depth, std::vector<XWin>& out) {
    if (depth > 4) return;
    Window root, parent, *children = NULL;
    unsigned int count = 0;
    if (!XQueryTree(dpy, win, &root, &parent, &children, &count)) return;
    for (unsigned int i = 0; i < count; ++i) {
        const Window c = children[i];
        XWindowAttributes a;
        if (!XGetWindowAttributes(dpy, c, &a) || a.map_state != IsViewable || a.c_class != InputOutput) continue;
        bool mine = false;
        if (pidAtom != None) {
            Atom type = None;
            int format = 0;
            unsigned long n = 0, after = 0;
            unsigned char* data = NULL;
            if (XGetWindowProperty(dpy, c, pidAtom, 0, 1, False, XA_CARDINAL, &type, &format, &n, &after, &data) == Success && data) {
                if (type == XA_CARDINAL && n >= 1 && *(unsigned long*)data == pid) mine = true;
                XFree(data);
            }
        }
        if (mine && a.width > 1 && a.height > 1) {
            int rx = 0, ry = 0;
            Window child;
            XTranslateCoordinates(dpy, c, root, 0, 0, &rx, &ry, &child);
            XWin w = {c, rx, ry, a.width, a.height};
            out.push_back(w);
        } else {
            CollectWindows(dpy, c, pidAtom, pid, depth + 1, out);
        }
    }
    if (children) XFree(children);
}

// Lays an X image over `out` at (ox, oy): a 32-bit image's transparent pixels are left out.
void Lay(XImage* img, Visual* visual, int depth, int ox, int oy, wxImage& out) {
    const unsigned long rm = visual->red_mask, gm = visual->green_mask, bm = visual->blue_mask;
    for (int y = 0; y < img->height; ++y) {
        const int ty = oy + y;
        if (ty < 0 || ty >= out.GetHeight()) continue;
        for (int x = 0; x < img->width; ++x) {
            const int tx = ox + x;
            if (tx < 0 || tx >= out.GetWidth()) continue;
            const unsigned long p = XGetPixel(img, x, y);
            if (depth == 32 && ((p >> 24) & 0xff) < 128) continue;
            out.SetRGB(tx, ty, Channel(p, rm), Channel(p, gm), Channel(p, bm));
        }
    }
}

// True when the picture was taken through X; false to try another way.
bool CaptureX(const wxString& path, int* width, int* height) {
    GdkDisplay* gd = gdk_display_get_default();
    if (!gd || !GDK_IS_X11_DISPLAY(gd)) return false;
    Display* dpy = GDK_DISPLAY_XDISPLAY(gd);
    const int screen = DefaultScreen(dpy);
    const Window root = RootWindow(dpy, screen);
    XSync(dpy, False);
    std::vector<XWin> wins;
    CollectWindows(dpy, root, XInternAtom(dpy, "_NET_WM_PID", True), (unsigned long)getpid(), 0, wins);
    wxRect box;
    bool any = false;
    for (size_t i = 0; i < wins.size(); ++i) {
        const wxRect r(wins[i].x, wins[i].y, wins[i].width, wins[i].height);
        if (!any) {
            box = r;
            any = true;
        } else {
            box.Union(r);
        }
    }
    if (!any && !WindowsBox(box)) return false;
    box.Intersect(wxRect(0, 0, DisplayWidth(dpy, screen), DisplayHeight(dpy, screen)));
    if (box.IsEmpty()) return false;
    wxImage out(box.width, box.height);
    XImage* img = XGetImage(dpy, root, box.x, box.y, box.width, box.height, AllPlanes, ZPixmap);
    if (!img) return false;
    Lay(img, DefaultVisual(dpy, screen), DefaultDepth(dpy, screen), 0, 0, out);
    XDestroyImage(img);
    for (size_t i = 0; i < wins.size(); ++i) {
        XWindowAttributes a;
        if (!XGetWindowAttributes(dpy, wins[i].id, &a)) continue;
        XImage* wi = XGetImage(dpy, wins[i].id, 0, 0, wins[i].width, wins[i].height, AllPlanes, ZPixmap);
        if (!wi) continue;
        Lay(wi, a.visual, a.depth, wins[i].x - box.x, wins[i].y - box.y, out);
        XDestroyImage(wi);
    }
    EnsurePng();
    if (!out.SaveFile(path, wxBITMAP_TYPE_PNG)) return false;
    if (width) *width = box.width;
    if (height) *height = box.height;
    return true;
}
#endif

#if defined(__WXGTK__)
bool Capture(const wxRect& box, const wxString& path) {
    wxImage out(box.width, box.height);
    // The window frames the window manager draws are not read; their area shows as the button face colour.
    const wxColour back = wxSystemSettings::GetColour(wxSYS_COLOUR_BTNFACE);
    out.SetRGB(wxRect(0, 0, box.width, box.height), back.Red(), back.Green(), back.Blue());
    // The windows in their stacking order as the list has them: the newest (a dialog over the frame) last, on top.
    for (wxWindowList::const_iterator it = wxTopLevelWindows.begin(); it != wxTopLevelWindows.end(); ++it) {
        wxTopLevelWindow* w = wxDynamicCast(*it, wxTopLevelWindow);
        if (!Visible(w)) continue;
        GtkWidget* widget = w->GetHandle();
        GdkWindow* win = gtk_widget_get_window(widget);
        if (!win) continue;
        const int ww = gdk_window_get_width(win), wh = gdk_window_get_height(win);
        if (ww <= 0 || wh <= 0) continue;
        GdkPixbuf* pb = gdk_pixbuf_get_from_window(win, 0, 0, ww, wh);
        if (!pb) continue;
        int ox = 0, oy = 0;
        gdk_window_get_origin(win, &ox, &oy);
        const int rs = gdk_pixbuf_get_rowstride(pb), nc = gdk_pixbuf_get_n_channels(pb);
        const guchar* px = gdk_pixbuf_get_pixels(pb);
        for (int y = 0; y < wh; ++y) {
            const int ty = oy - box.y + y;
            if (ty < 0 || ty >= box.height) continue;
            for (int x = 0; x < ww; ++x) {
                const int tx = ox - box.x + x;
                if (tx < 0 || tx >= box.width) continue;
                const guchar* p = px + y * rs + x * nc;
                out.SetRGB(tx, ty, p[0], p[1], p[2]);
            }
        }
        g_object_unref(pb);
    }
    EnsurePng();
    return out.SaveFile(path, wxBITMAP_TYPE_PNG);
}
#elif defined(__WXOSX__)
bool Capture(const wxRect& box, const wxString& path) {
    // screencapture -x (no sound) -R x,y,w,h: the region of the screen, with the system's Screen Recording
    // permission as the system's own screenshot keys need it.
    wxString quoted = path;
    quoted.Replace("'", "'\\''");
    const wxString command = wxString::Format("/usr/sbin/screencapture -x -R %d,%d,%d,%d '%s'", box.x, box.y, box.width, box.height, quoted);
    return wxExecute(command, wxEXEC_SYNC) == 0 && wxFileExists(path);
}
#else
bool Capture(const wxRect& box, const wxString& path) {
    wxScreenDC screenDC;
    wxBitmap bitmap(box.width, box.height);
    {
        wxMemoryDC memory(bitmap);
        if (!memory.Blit(0, 0, box.width, box.height, &screenDC, box.x, box.y)) return false;
    }
    EnsurePng();
    return bitmap.SaveFile(path, wxBITMAP_TYPE_PNG);
}
#endif

}  // namespace

extern "C" {

WXD_EXPORTED bool wxd_App_SaveScreenshot(const char* path, int settle_ms, int* width, int* height) {
    if (!path) return false;
    // The program carries out what it has pending first (a dialog being opened, a window brought to the front,
    // paints), for `settle_ms`, so the picture shows the state the last request led to.
    wxStopWatch watch;
    do {
        wxYieldIfNeeded();
        wxMilliSleep(10);
    } while (watch.Time() < settle_ms);
    wxYieldIfNeeded();
    const wxString file = wxString::FromUTF8(path);
#if defined(__WXGTK__) && defined(GDK_WINDOWING_X11)
    if (CaptureX(file, width, height)) return true;
#endif
    wxRect box;
    if (!WindowsBox(box)) return false;
    if (!Capture(box, file)) return false;
    if (width) *width = box.width;
    if (height) *height = box.height;
    return true;
}

}
