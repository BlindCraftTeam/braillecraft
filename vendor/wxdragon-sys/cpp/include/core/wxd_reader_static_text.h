#ifndef WXD_READER_STATIC_TEXT_H
#define WXD_READER_STATIC_TEXT_H

#include "../wxd_types.h"

// Added for BrailleCraft: the static text class dialog resources name with subclass="WxdReaderStaticText" (a text
// Tab reaches while the system reports a screen reader running). Returns whether the class is registered; calling it
// once at start-up also keeps the class in the program when it is linked from a static library.
WXD_EXPORTED bool wxd_ReaderStaticText_Registered();

#endif // WXD_READER_STATIC_TEXT_H
