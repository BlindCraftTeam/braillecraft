// Added for BrailleCraft: empty space after a line of a multi-line text control on macOS (space_after.cpp): the
// text view's paragraph spacing over those characters.

#import <AppKit/AppKit.h>
#include <wx/textctrl.h>

void wxd_osx_SetSpaceAfter(wxTextCtrl* text, long from, long to, int pixels)
{
    NSView* view = text->GetHandle();
    NSTextView* tv = nil;
    if ([view isKindOfClass:[NSScrollView class]]) {
        id doc = [(NSScrollView*)view documentView];
        if ([doc isKindOfClass:[NSTextView class]]) tv = (NSTextView*)doc;
    } else if ([view isKindOfClass:[NSTextView class]]) {
        tv = (NSTextView*)view;
    }
    if (!tv) return;
    NSTextStorage* storage = [tv textStorage];
    NSUInteger length = [storage length];
    if ((NSUInteger)from > length) return;
    NSUInteger end = MIN((NSUInteger)to, length);
    NSMutableParagraphStyle* style = [[NSMutableParagraphStyle alloc] init];
    [style setParagraphSpacing:(CGFloat)pixels];
    [storage addAttribute:NSParagraphStyleAttributeName value:style range:NSMakeRange((NSUInteger)from, end - (NSUInteger)from)];
}
