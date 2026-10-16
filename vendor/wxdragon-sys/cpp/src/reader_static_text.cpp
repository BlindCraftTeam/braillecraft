// Added for BrailleCraft: a static text that Tab reaches while the system reports a screen reader running.
//
// On Windows a dialog's static texts are normally passed over by Tab; with a screen reader running, a text that
// holds something is made a tab stop so it can be reached and read. wxWidgets moves focus only to windows that
// accept it, so the text says it accepts focus exactly then: on Windows, while SPI_GETSCREENREADER is on and the
// text is not empty. Other systems have no such flag, and there the text behaves as any static text.
//
// Dialog resources ask for it by name, with subclass="WxdReaderStaticText" on a wxStaticText object.

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/stattext.h>
#ifdef __WXMSW__
#include <wx/msw/wrapwin.h>
#endif
#include "../include/wxdragon.h"

class WxdReaderStaticText : public wxStaticText {
public:
    WxdReaderStaticText() {}

    bool AcceptsFocus() const override { return Reachable(); }
    bool AcceptsFocusFromKeyboard() const override { return Reachable(); }

private:
    bool Reachable() const {
#ifdef __WXMSW__
        BOOL on = FALSE;
        if (!::SystemParametersInfoW(SPI_GETSCREENREADER, 0, &on, 0))
            on = FALSE;
        return on && !GetLabel().empty();
#else
        return false;
#endif
    }

    wxDECLARE_DYNAMIC_CLASS(WxdReaderStaticText);
};

wxIMPLEMENT_DYNAMIC_CLASS(WxdReaderStaticText, wxStaticText);

extern "C" bool wxd_ReaderStaticText_Registered() {
    return wxClassInfo::FindClass("WxdReaderStaticText") != nullptr;
}
