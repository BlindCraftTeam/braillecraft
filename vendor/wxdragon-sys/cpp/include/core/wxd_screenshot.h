#ifndef WXD_SCREENSHOT_H
#define WXD_SCREENSHOT_H

#include "../wxd_types.h"

// Added for BrailleCraft: a picture of the program's windows, for the control channel's screenshot tool
// (braillecraft-cli mcp --connect). Every shown top-level window of the program is taken as one picture of the
// screen area they cover, written as a PNG file at `path` (UTF-8), after the program has carried out its pending
// events for `settle_ms` milliseconds. `width` and `height` get the picture's size. False when no window is shown
// or the screen cannot be read (a display server that does not let a program read the screen, such as Wayland
// without a portal).
WXD_EXPORTED bool wxd_App_SaveScreenshot(const char* path, int settle_ms, int* width, int* height);

#endif // WXD_SCREENSHOT_H
