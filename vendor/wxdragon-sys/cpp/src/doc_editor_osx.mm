// Added for BrailleCraft: the drawn document editing window's text for screen readers on macOS (VoiceOver).
//
// wxWidgets has no accessibility layer on macOS (wxUSE_ACCESSIBILITY is Windows only). The window's own view becomes
// an accessibility text area over the window's text for screen readers (each text row followed by a line feed, page
// break rows not in it, as the reference gives its editing window, spec 27 section 6): its value, the selected range
// (the caret), lines, the string and screen frame of a range; it posts the value changed notification when the text
// changes and the selected text changed notification when the caret moves. VoiceOver moving the caret or selecting
// does it as a click would.
//
// The view's class is wxWidgets' own; the view is given a subclass of it, made at run time, that adds the
// accessibility methods. Their code is written as methods of a helper class (so the compiler gives each its types)
// and copied onto that subclass, where `self` is the view.

#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#include <wx/wxprec.h>
#include <wx/wx.h>
#include "doc_editor_internal.h"

static char kEditorKey;

static DocEditor* EditorOf(id view)
{
    NSValue* v = objc_getAssociatedObject(view, &kEditorKey);
    return v ? static_cast<DocEditor*>([v pointerValue]) : nullptr;
}

static NSString* NS(const wxString& s)
{
    const wxScopedCharBuffer b = s.utf8_str();
    return [NSString stringWithUTF8String:b.data() ? b.data() : ""];
}

// The line (0-based) holding offset `offset` of the text.
static NSInteger LineOf(const wxString& text, int offset)
{
    NSInteger line = 0;
    const int n = wxMin(offset, (int)text.length());
    for (int k = 0; k < n; ++k) {
        if (text[k] == '\n') ++line;
    }
    return line;
}

// A rectangle of the window (client coordinates) in screen coordinates as Cocoa counts them (from the bottom of the
// main screen).
static NSRect ScreenRect(DocEditor* e, const wxRect& r)
{
    const wxPoint p = e->ClientToScreen(r.GetPosition());
    const CGFloat height = [[[NSScreen screens] firstObject] frame].size.height;
    return NSMakeRect(p.x, height - p.y - r.height, r.width, r.height);
}

@interface BCDocEditorAccess : NSObject
@end

@implementation BCDocEditorAccess

- (BOOL)isAccessibilityElement
{
    return EditorOf(self) != nullptr;
}

- (NSAccessibilityRole)accessibilityRole
{
    return NSAccessibilityTextAreaRole;
}

- (NSString*)accessibilityLabel
{
    DocEditor* e = EditorOf(self);
    return e ? NS(e->GetName()) : @"";
}

- (id)accessibilityValue
{
    DocEditor* e = EditorOf(self);
    return e ? NS(e->AccessibleText()) : @"";
}

- (NSInteger)accessibilityNumberOfCharacters
{
    DocEditor* e = EditorOf(self);
    return e ? (NSInteger)e->AccessibleText().length() : 0;
}

- (NSRange)accessibilitySelectedTextRange
{
    DocEditor* e = EditorOf(self);
    if (!e) return NSMakeRange(0, 0);
    const int a = e->OffsetOf(e->AnchorRow(), e->AnchorCol());
    const int c = e->OffsetOf(e->CaretRow(), e->CaretCol());
    return NSMakeRange((NSUInteger)wxMin(a, c), (NSUInteger)abs(a - c));
}

- (void)setAccessibilitySelectedTextRange:(NSRange)range
{
    DocEditor* e = EditorOf(self);
    if (!e) return;
    int r1 = 0, c1 = 0, r2 = 0, c2 = 0;
    e->PlaceOf((int)range.location, &r1, &c1);
    e->PlaceOf((int)(range.location + range.length), &r2, &c2);
    e->Click(0, r1, c1);
    if (range.length > 0) {
        e->Click(1, r2, c2);
        e->Click(2, r2, c2);
    }
}

- (NSString*)accessibilitySelectedText
{
    DocEditor* e = EditorOf(self);
    if (!e) return @"";
    const int a = e->OffsetOf(e->AnchorRow(), e->AnchorCol());
    const int c = e->OffsetOf(e->CaretRow(), e->CaretCol());
    return NS(e->AccessibleText().Mid(wxMin(a, c), abs(a - c)));
}

- (NSRange)accessibilityVisibleCharacterRange
{
    DocEditor* e = EditorOf(self);
    if (!e) return NSMakeRange(0, 0);
    const int top = e->GetTopRow();
    const int first = e->OffsetOf(top, 0);
    const int last = e->OffsetOf(top + e->GetVisibleRows(), 0);
    return NSMakeRange((NSUInteger)first, (NSUInteger)wxMax(last - first, 0));
}

- (NSInteger)accessibilityInsertionPointLineNumber
{
    DocEditor* e = EditorOf(self);
    return e ? LineOf(e->AccessibleText(), e->OffsetOf(e->CaretRow(), e->CaretCol())) : 0;
}

- (NSInteger)accessibilityLineForIndex:(NSInteger)index
{
    DocEditor* e = EditorOf(self);
    return e ? LineOf(e->AccessibleText(), (int)index) : 0;
}

- (NSRange)accessibilityRangeForLine:(NSInteger)line
{
    DocEditor* e = EditorOf(self);
    if (!e) return NSMakeRange(0, 0);
    const wxString& text = e->AccessibleText();
    const int n = (int)text.length();
    int start = 0;
    for (NSInteger l = 0; l < line && start < n; ++start) {
        if (text[start] == '\n') ++l;
    }
    int end = start;
    while (end < n && text[end] != '\n') ++end;
    if (end < n) ++end;
    return NSMakeRange((NSUInteger)start, (NSUInteger)(end - start));
}

- (NSString*)accessibilityStringForRange:(NSRange)range
{
    DocEditor* e = EditorOf(self);
    return e ? NS(e->AccessibleText().Mid((int)range.location, (int)range.length)) : @"";
}

- (NSRect)accessibilityFrameForRange:(NSRange)range
{
    DocEditor* e = EditorOf(self);
    if (!e) return NSZeroRect;
    int r1 = 0, c1 = 0, r2 = 0, c2 = 0;
    e->PlaceOf((int)range.location, &r1, &c1);
    e->PlaceOf((int)(range.location + range.length), &r2, &c2);
    if (r2 != r1) c2 = (int)e->RowText(r1).length();
    return ScreenRect(e, e->RowRect(r1, c1, wxMax(c2, c1 + 1)));
}

- (NSRange)accessibilityRangeForPosition:(NSPoint)point
{
    DocEditor* e = EditorOf(self);
    if (!e) return NSMakeRange(0, 0);
    const CGFloat height = [[[NSScreen screens] firstObject] frame].size.height;
    const wxPoint p = e->ScreenToClient(wxPoint((int)point.x, (int)(height - point.y)));
    int vx = 0, vy = 0;
    e->CalcUnscrolledPosition(p.x, p.y, &vx, &vy);
    const int row = wxClip(vy / wxMax(e->RowHeight(), 1), 0, wxMax(e->RowCount() - 1, 0));
    return NSMakeRange((NSUInteger)e->OffsetOf(row, e->ColumnAt(row, vx)), 1);
}

- (BOOL)isAccessibilityFocused
{
    DocEditor* e = EditorOf(self);
    return e && wxWindow::FindFocus() == e;
}

@end

static NSView* ViewOf(DocEditor* e)
{
    NSView* view = (NSView*)e->GetHandle();
    if ([view isKindOfClass:[NSScrollView class]]) view = [(NSScrollView*)view documentView];
    return view;
}

// The subclass of `base` with the accessibility methods, made once per base class.
static Class SubclassOf(Class base)
{
    NSString* name = [NSString stringWithFormat:@"BCDocEditor_%s", class_getName(base)];
    Class c = NSClassFromString(name);
    if (c) return c;
    c = objc_allocateClassPair(base, [name UTF8String], 0);
    if (!c) return nil;
    const SEL selectors[] = {
        @selector(isAccessibilityElement),
        @selector(accessibilityRole),
        @selector(accessibilityLabel),
        @selector(accessibilityValue),
        @selector(accessibilityNumberOfCharacters),
        @selector(accessibilitySelectedTextRange),
        @selector(setAccessibilitySelectedTextRange:),
        @selector(accessibilitySelectedText),
        @selector(accessibilityVisibleCharacterRange),
        @selector(accessibilityInsertionPointLineNumber),
        @selector(accessibilityLineForIndex:),
        @selector(accessibilityRangeForLine:),
        @selector(accessibilityStringForRange:),
        @selector(accessibilityFrameForRange:),
        @selector(accessibilityRangeForPosition:),
        @selector(isAccessibilityFocused),
    };
    for (SEL s : selectors) {
        Method m = class_getInstanceMethod([BCDocEditorAccess class], s);
        if (m) class_addMethod(c, s, method_getImplementation(m), method_getTypeEncoding(m));
    }
    objc_registerClassPair(c);
    return c;
}

void DocEditor::AccessibleInit()
{
    NSView* view = ViewOf(this);
    if (!view) return;
    objc_setAssociatedObject(view, &kEditorKey, [NSValue valueWithPointer:this], OBJC_ASSOCIATION_RETAIN);
    if (Class c = SubclassOf(object_getClass(view))) object_setClass(view, c);
    m_accessible = (__bridge void*)view;
}

void DocEditor::AccessibleDone()
{
    if (m_accessible) {
        NSView* view = (__bridge NSView*)m_accessible;
        objc_setAssociatedObject(view, &kEditorKey, nil, OBJC_ASSOCIATION_RETAIN);
    }
    m_accessible = nullptr;
}

void DocEditor::AccessibleTextChanged(int)
{
    if (m_accessible) NSAccessibilityPostNotification((__bridge id)m_accessible, NSAccessibilityValueChangedNotification);
}

void DocEditor::AccessibleCaretMoved()
{
    if (m_accessible) {
        NSAccessibilityPostNotification((__bridge id)m_accessible, NSAccessibilitySelectedTextChangedNotification);
    }
}

void DocEditor::AccessibleNameChanged()
{
    if (m_accessible) NSAccessibilityPostNotification((__bridge id)m_accessible, NSAccessibilityTitleChangedNotification);
}
