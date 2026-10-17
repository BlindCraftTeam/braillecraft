#ifndef WXD_SAVE_DIALOG_H
#define WXD_SAVE_DIALOG_H

#include "../wxd_types.h"

// Added for BrailleCraft: a Save As file dialog holding a labelled choice (an encoding list) of its own, shown only
// while one of the types listed in `shown_for` (type indexes, `shown_count` of them) is chosen; the choice's first
// item is chosen when the dialog opens. `items` holds the choice's items, one per line. Returns 1 on OK, with the
// file's path (UTF-8, at most `path_size` bytes with its ending zero) in `path`, the type chosen in `type_index`
// and the choice's item in `item_index`; 0 when the dialog is cancelled.
WXD_EXPORTED int wxd_SaveDialog_WithChoice(wxd_Window_t* parent, const char* title, const char* wildcard,
                                           const char* default_dir, const char* default_file, int type_index,
                                           const char* label, const char* items, const int* shown_for,
                                           int shown_count, char* path, int path_size, int* type_out,
                                           int* item_out);

#endif // WXD_SAVE_DIALOG_H
