// Added for BrailleCraft: the drawn document editing window's UI Automation provider on Windows (spec 15, spec 27
// section 6, spec 30 section on UI Automation).
//
// The reference program gives its own drawn editing window a UI Automation provider: the window answers
// WM_GETOBJECT for UiaRootObjectId with an element that has the Text pattern over the window's text (the formatted
// page lines or the coded rows, each followed by a line feed), with text ranges by character, word (ending at white
// space), line and paragraph (one row), page (one formatted page) and document; it raises Text_TextChanged and
// Text_TextSelectionChanged when the window is refreshed and the caret moves; all of it only while Global,
// Internationalization, Support UI Automation is on. GetVisibleRanges is not offered (as in the reference) and
// ScrollIntoView does nothing (as in the reference). The system caret the window keeps (wxCaret, the Win32 caret)
// gives screen readers and magnifiers the caret through MSAA as well.

#if defined(__WXMSW__)

#include <wx/wxprec.h>
#include <wx/wx.h>
#include <wx/msw/wrapwin.h>
#include <ole2.h>
#include <UIAutomation.h>
#include <vector>
#include "doc_editor_internal.h"

namespace {

// Global, Internationalization, Support UI Automation.
bool g_support = true;

class TextRange;

class Provider : public IRawElementProviderSimple,
                 public IRawElementProviderFragment,
                 public IRawElementProviderFragmentRoot,
                 public ITextProvider {
public:
    explicit Provider(DocEditor* editor) : m_editor(editor) {}

    DocEditor* Editor() const { return m_editor; }
    void Disconnect() { m_editor = nullptr; }

    // IUnknown
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&m_refs); }
    ULONG STDMETHODCALLTYPE Release() override
    {
        const LONG n = InterlockedDecrement(&m_refs);
        if (n == 0) delete this;
        return n;
    }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        if (riid == __uuidof(IUnknown) || riid == __uuidof(IRawElementProviderSimple))
            *out = static_cast<IRawElementProviderSimple*>(this);
        else if (riid == __uuidof(IRawElementProviderFragment))
            *out = static_cast<IRawElementProviderFragment*>(this);
        else if (riid == __uuidof(IRawElementProviderFragmentRoot))
            *out = static_cast<IRawElementProviderFragmentRoot*>(this);
        else if (riid == __uuidof(ITextProvider))
            *out = static_cast<ITextProvider*>(this);
        else
            return E_NOINTERFACE;
        AddRef();
        return S_OK;
    }

    // IRawElementProviderSimple
    HRESULT STDMETHODCALLTYPE get_ProviderOptions(ProviderOptions* out) override
    {
        if (!out) return E_INVALIDARG;
        *out = ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetPatternProvider(PATTERNID id, IUnknown** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        if (!m_editor) return UIA_E_ELEMENTNOTAVAILABLE;
        if (id == UIA_TextPatternId) {
            *out = static_cast<ITextProvider*>(this);
            AddRef();
        }
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetPropertyValue(PROPERTYID id, VARIANT* out) override
    {
        if (!out) return E_INVALIDARG;
        VariantInit(out);
        if (!m_editor) return UIA_E_ELEMENTNOTAVAILABLE;
        switch (id) {
        case UIA_ControlTypePropertyId:
            // INFERENCE (spec 30 unknown: the reference's property values): an edit area.
            out->vt = VT_I4;
            out->lVal = UIA_EditControlTypeId;
            break;
        case UIA_NamePropertyId:
            out->vt = VT_BSTR;
            out->bstrVal = SysAllocString(m_editor->GetName().wc_str());
            break;
        case UIA_IsKeyboardFocusablePropertyId:
            out->vt = VT_BOOL;
            out->boolVal = VARIANT_TRUE;
            break;
        case UIA_HasKeyboardFocusPropertyId:
            out->vt = VT_BOOL;
            out->boolVal = wxWindow::FindFocus() == m_editor ? VARIANT_TRUE : VARIANT_FALSE;
            break;
        case UIA_IsTextPatternAvailablePropertyId:
            out->vt = VT_BOOL;
            out->boolVal = VARIANT_TRUE;
            break;
        case UIA_IsControlElementPropertyId:
        case UIA_IsContentElementPropertyId:
        case UIA_IsEnabledPropertyId:
            out->vt = VT_BOOL;
            out->boolVal = VARIANT_TRUE;
            break;
        default:
            break;
        }
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE get_HostRawElementProvider(IRawElementProviderSimple** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        if (!m_editor) return UIA_E_ELEMENTNOTAVAILABLE;
        return UiaHostProviderFromHwnd((HWND)m_editor->GetHWND(), out);
    }

    // IRawElementProviderFragment
    HRESULT STDMETHODCALLTYPE Navigate(NavigateDirection, IRawElementProviderFragment** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetRuntimeId(SAFEARRAY** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr; // The host window gives the root its identity.
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE get_BoundingRectangle(UiaRect* out) override
    {
        if (!out) return E_INVALIDARG;
        *out = UiaRect{0, 0, 0, 0}; // The host window gives the root its rectangle.
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetEmbeddedFragmentRoots(SAFEARRAY** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetFocus() override
    {
        if (!m_editor) return UIA_E_ELEMENTNOTAVAILABLE;
        m_editor->SetFocus();
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE get_FragmentRoot(IRawElementProviderFragmentRoot** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = static_cast<IRawElementProviderFragmentRoot*>(this);
        AddRef();
        return S_OK;
    }

    // IRawElementProviderFragmentRoot
    HRESULT STDMETHODCALLTYPE ElementProviderFromPoint(double, double, IRawElementProviderFragment** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = static_cast<IRawElementProviderFragment*>(this);
        AddRef();
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetFocus(IRawElementProviderFragment** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        return S_OK;
    }

    // ITextProvider
    HRESULT STDMETHODCALLTYPE GetSelection(SAFEARRAY** out) override;
    HRESULT STDMETHODCALLTYPE GetVisibleRanges(SAFEARRAY** out) override
    {
        // Not offered by the reference (spec 30).
        if (out) *out = nullptr;
        return E_NOTIMPL;
    }
    HRESULT STDMETHODCALLTYPE RangeFromChild(IRawElementProviderSimple*, ITextRangeProvider** out) override
    {
        if (out) *out = nullptr;
        return E_INVALIDARG;
    }
    HRESULT STDMETHODCALLTYPE RangeFromPoint(UiaPoint point, ITextRangeProvider** out) override;
    HRESULT STDMETHODCALLTYPE get_DocumentRange(ITextRangeProvider** out) override;
    HRESULT STDMETHODCALLTYPE get_SupportedTextSelection(SupportedTextSelection* out) override
    {
        if (!out) return E_INVALIDARG;
        *out = SupportedTextSelection_Single;
        return S_OK;
    }

private:
    ~Provider() = default;
    LONG m_refs = 1;
    DocEditor* m_editor;
};

// The units of the window's text (spec 27 section 6): a character; a word, ending at white space; a line and a
// paragraph, one row; a page, one formatted page (the rows between page break rows); the document.
int UnitStart(DocEditor* e, int offset, TextUnit unit)
{
    const wxString& text = e->AccessibleText();
    const int n = (int)text.length();
    offset = wxClip(offset, 0, n);
    switch (unit) {
    case TextUnit_Character:
        return offset;
    case TextUnit_Format:
    case TextUnit_Word: {
        int s = offset;
        // Back over the spaces before the word to its start.
        while (s > 0 && (text[s - 1] == ' ' || text[s - 1] == '\t') && !(s < n && text[s] != ' ' && text[s] != '\n'))
            --s;
        while (s > 0 && text[s - 1] != ' ' && text[s - 1] != '\n' && text[s - 1] != '\t') --s;
        return s;
    }
    case TextUnit_Line:
    case TextUnit_Paragraph: {
        int s = offset;
        while (s > 0 && text[s - 1] != '\n') --s;
        return s;
    }
    case TextUnit_Page: {
        int row = 0, col = 0;
        e->PlaceOf(offset, &row, &col);
        while (row > 0 && !e->IsBreakRow(row - 1)) --row;
        while (row < e->RowCount() && e->IsBreakRow(row)) ++row;
        return e->OffsetOf(row, 0);
    }
    case TextUnit_Document:
    default:
        return 0;
    }
}

int UnitEnd(DocEditor* e, int offset, TextUnit unit)
{
    const wxString& text = e->AccessibleText();
    const int n = (int)text.length();
    offset = wxClip(offset, 0, n);
    switch (unit) {
    case TextUnit_Character:
        return wxMin(offset + 1, n);
    case TextUnit_Format:
    case TextUnit_Word: {
        int en = offset;
        while (en < n && text[en] != ' ' && text[en] != '\n' && text[en] != '\t') ++en;
        while (en < n && (text[en] == ' ' || text[en] == '\t')) ++en;
        if (en < n && text[en] == '\n' && en == offset) ++en;
        return en;
    }
    case TextUnit_Line:
    case TextUnit_Paragraph: {
        int en = offset;
        while (en < n && text[en] != '\n') ++en;
        return wxMin(en + 1, n);
    }
    case TextUnit_Page: {
        int row = 0, col = 0;
        e->PlaceOf(offset, &row, &col);
        while (row < e->RowCount() && !e->IsBreakRow(row)) ++row;
        return row >= e->RowCount() ? n : e->OffsetOf(row, 0);
    }
    case TextUnit_Document:
    default:
        return n;
    }
}

class TextRange : public ITextRangeProvider {
public:
    TextRange(Provider* provider, int start, int end) : m_provider(provider), m_start(start), m_end(end)
    {
        m_provider->AddRef();
    }

    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&m_refs); }
    ULONG STDMETHODCALLTYPE Release() override
    {
        const LONG n = InterlockedDecrement(&m_refs);
        if (n == 0) delete this;
        return n;
    }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        if (riid == __uuidof(IUnknown) || riid == __uuidof(ITextRangeProvider)) {
            *out = static_cast<ITextRangeProvider*>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    HRESULT STDMETHODCALLTYPE Clone(ITextRangeProvider** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = new TextRange(m_provider, m_start, m_end);
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE Compare(ITextRangeProvider* other, BOOL* out) override
    {
        if (!out || !other) return E_INVALIDARG;
        TextRange* o = static_cast<TextRange*>(other);
        *out = o->m_start == m_start && o->m_end == m_end;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE CompareEndpoints(TextPatternRangeEndpoint which, ITextRangeProvider* other,
                                               TextPatternRangeEndpoint otherWhich, int* out) override
    {
        if (!out || !other) return E_INVALIDARG;
        TextRange* o = static_cast<TextRange*>(other);
        const int a = which == TextPatternRangeEndpoint_Start ? m_start : m_end;
        const int b = otherWhich == TextPatternRangeEndpoint_Start ? o->m_start : o->m_end;
        *out = a - b;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE ExpandToEnclosingUnit(TextUnit unit) override
    {
        DocEditor* e = Ed();
        if (!e) return UIA_E_ELEMENTNOTAVAILABLE;
        m_start = UnitStart(e, m_start, unit);
        m_end = UnitEnd(e, m_start, unit);
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE FindAttribute(TEXTATTRIBUTEID, VARIANT, BOOL, ITextRangeProvider** out) override
    {
        if (out) *out = nullptr;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE FindText(BSTR text, BOOL backward, BOOL ignoreCase, ITextRangeProvider** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        DocEditor* e = Ed();
        if (!e) return UIA_E_ELEMENTNOTAVAILABLE;
        wxString hay = e->AccessibleText().Mid(m_start, m_end - m_start);
        wxString needle(text);
        if (ignoreCase) {
            hay.MakeLower();
            needle.MakeLower();
        }
        const size_t at = backward ? hay.rfind(needle) : hay.find(needle);
        if (at != wxString::npos) *out = new TextRange(m_provider, m_start + (int)at, m_start + (int)at + (int)needle.length());
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetAttributeValue(TEXTATTRIBUTEID id, VARIANT* out) override
    {
        if (!out) return E_INVALIDARG;
        VariantInit(out);
        if (id == UIA_IsReadOnlyAttributeId) {
            out->vt = VT_BOOL;
            out->boolVal = VARIANT_FALSE;
            return S_OK;
        }
        IUnknown* none = nullptr;
        UiaGetReservedNotSupportedValue(&none);
        out->vt = VT_UNKNOWN;
        out->punkVal = none;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetBoundingRectangles(SAFEARRAY** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = nullptr;
        DocEditor* e = Ed();
        if (!e) return UIA_E_ELEMENTNOTAVAILABLE;
        int r1 = 0, c1 = 0, r2 = 0, c2 = 0;
        e->PlaceOf(m_start, &r1, &c1);
        e->PlaceOf(m_end, &r2, &c2);
        std::vector<double> boxes;
        const wxRect client = e->GetClientRect();
        for (int row = r1; row <= r2 && row < e->RowCount(); ++row) {
            if (e->IsBreakRow(row)) continue;
            const int from = row == r1 ? c1 : 0;
            const int to = row == r2 ? c2 : (int)e->RowText(row).length();
            wxRect r = e->RowRect(row, from, wxMax(to, from));
            if (!r.Intersects(client)) continue;
            const wxPoint p = e->ClientToScreen(r.GetPosition());
            boxes.insert(boxes.end(), {(double)p.x, (double)p.y, (double)r.width, (double)r.height});
        }
        SAFEARRAY* sa = SafeArrayCreateVector(VT_R8, 0, (ULONG)boxes.size());
        for (LONG i = 0; i < (LONG)boxes.size(); ++i) SafeArrayPutElement(sa, &i, &boxes[i]);
        *out = sa;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetEnclosingElement(IRawElementProviderSimple** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = static_cast<IRawElementProviderSimple*>(m_provider);
        m_provider->AddRef();
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetText(int maxLength, BSTR* out) override
    {
        if (!out) return E_INVALIDARG;
        DocEditor* e = Ed();
        if (!e) return UIA_E_ELEMENTNOTAVAILABLE;
        int len = m_end - m_start;
        if (maxLength >= 0 && maxLength < len) len = maxLength;
        *out = SysAllocString(e->AccessibleText().Mid(m_start, len).wc_str());
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE Move(TextUnit unit, int count, int* moved) override
    {
        if (!moved) return E_INVALIDARG;
        *moved = 0;
        DocEditor* e = Ed();
        if (!e) return UIA_E_ELEMENTNOTAVAILABLE;
        const bool degenerate = m_start == m_end;
        const int n = (int)e->AccessibleText().length();
        int at = UnitStart(e, m_start, unit);
        while (count > 0 && at < n) {
            const int next = UnitEnd(e, at, unit);
            if (next >= n && unit != TextUnit_Document) {
                if (next == at) break;
            }
            if (next == at) break;
            at = next;
            if (at >= n) break;
            --count;
            ++*moved;
        }
        while (count < 0 && at > 0) {
            at = UnitStart(e, at - 1, unit);
            ++count;
            --*moved;
        }
        m_start = at;
        m_end = degenerate ? at : UnitEnd(e, at, unit);
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE MoveEndpointByUnit(TextPatternRangeEndpoint which, TextUnit unit, int count,
                                                 int* moved) override
    {
        if (!moved) return E_INVALIDARG;
        *moved = 0;
        DocEditor* e = Ed();
        if (!e) return UIA_E_ELEMENTNOTAVAILABLE;
        const int n = (int)e->AccessibleText().length();
        int at = which == TextPatternRangeEndpoint_Start ? m_start : m_end;
        while (count > 0 && at < n) {
            const int start = UnitStart(e, at, unit);
            int next = UnitEnd(e, start, unit);
            if (next <= at) next = UnitEnd(e, at, unit);
            if (next <= at) break;
            at = next;
            --count;
            ++*moved;
        }
        while (count < 0 && at > 0) {
            int prev = UnitStart(e, at, unit);
            if (prev >= at) prev = UnitStart(e, at - 1, unit);
            at = prev;
            ++count;
            --*moved;
        }
        if (which == TextPatternRangeEndpoint_Start) {
            m_start = at;
            if (m_end < m_start) m_end = m_start;
        } else {
            m_end = at;
            if (m_start > m_end) m_start = m_end;
        }
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE MoveEndpointByRange(TextPatternRangeEndpoint which, ITextRangeProvider* other,
                                                  TextPatternRangeEndpoint otherWhich) override
    {
        if (!other) return E_INVALIDARG;
        TextRange* o = static_cast<TextRange*>(other);
        const int at = otherWhich == TextPatternRangeEndpoint_Start ? o->m_start : o->m_end;
        if (which == TextPatternRangeEndpoint_Start) {
            m_start = at;
            if (m_end < m_start) m_end = m_start;
        } else {
            m_end = at;
            if (m_start > m_end) m_start = m_end;
        }
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE Select() override
    {
        DocEditor* e = Ed();
        if (!e) return UIA_E_ELEMENTNOTAVAILABLE;
        int r1 = 0, c1 = 0, r2 = 0, c2 = 0;
        e->PlaceOf(m_start, &r1, &c1);
        e->PlaceOf(m_end, &r2, &c2);
        e->Click(0, r1, c1);
        if (m_end != m_start) {
            e->Click(1, r2, c2);
            e->Click(2, r2, c2);
        }
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE AddToSelection() override { return Select(); }
    HRESULT STDMETHODCALLTYPE RemoveFromSelection() override { return S_OK; }
    // The reference does nothing here (spec 30).
    HRESULT STDMETHODCALLTYPE ScrollIntoView(BOOL) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE GetChildren(SAFEARRAY** out) override
    {
        if (!out) return E_INVALIDARG;
        *out = SafeArrayCreateVector(VT_UNKNOWN, 0, 0);
        return S_OK;
    }

private:
    ~TextRange() { m_provider->Release(); }
    DocEditor* Ed() const { return m_provider->Editor(); }
    LONG m_refs = 1;
    Provider* m_provider;
    int m_start, m_end;
};

SAFEARRAY* OneRange(ITextRangeProvider* range)
{
    SAFEARRAY* sa = SafeArrayCreateVector(VT_UNKNOWN, 0, 1);
    LONG i = 0;
    SafeArrayPutElement(sa, &i, range);
    range->Release();
    return sa;
}

HRESULT Provider::GetSelection(SAFEARRAY** out)
{
    if (!out) return E_INVALIDARG;
    *out = nullptr;
    if (!m_editor) return UIA_E_ELEMENTNOTAVAILABLE;
    const int a = m_editor->OffsetOf(m_editor->AnchorRow(), m_editor->AnchorCol());
    const int c = m_editor->OffsetOf(m_editor->CaretRow(), m_editor->CaretCol());
    *out = OneRange(new TextRange(this, wxMin(a, c), wxMax(a, c)));
    return S_OK;
}

HRESULT Provider::RangeFromPoint(UiaPoint point, ITextRangeProvider** out)
{
    if (!out) return E_INVALIDARG;
    *out = nullptr;
    if (!m_editor) return UIA_E_ELEMENTNOTAVAILABLE;
    // The point is on the screen; the reference used it as a window point (spec 30), which places it wrongly.
    const wxPoint p = m_editor->ScreenToClient(wxPoint((int)point.x, (int)point.y));
    int vx = 0, vy = 0;
    m_editor->CalcUnscrolledPosition(p.x, p.y, &vx, &vy);
    const int row = wxClip(vy / wxMax(m_editor->RowHeight(), 1), 0, wxMax(m_editor->RowCount() - 1, 0));
    const int offset = m_editor->OffsetOf(row, m_editor->ColumnAt(row, vx));
    *out = new TextRange(this, offset, offset);
    return S_OK;
}

HRESULT Provider::get_DocumentRange(ITextRangeProvider** out)
{
    if (!out) return E_INVALIDARG;
    *out = nullptr;
    if (!m_editor) return UIA_E_ELEMENTNOTAVAILABLE;
    *out = new TextRange(this, 0, (int)m_editor->AccessibleText().length());
    return S_OK;
}

Provider* ProviderOf(DocEditor* e)
{
    return static_cast<Provider*>(e->m_accessible);
}

void RaiseUiaEvent(DocEditor* e, EVENTID id)
{
    Provider* p = ProviderOf(e);
    if (p && g_support && UiaClientsAreListening()) UiaRaiseAutomationEvent(p, id);
}

} // namespace

void DocEditor::AccessibleInit()
{
    m_accessible = new Provider(this);
}

void DocEditor::AccessibleDone()
{
    if (Provider* p = ProviderOf(this)) {
        p->Disconnect();
        UiaReturnRawElementProvider((HWND)GetHWND(), 0, 0, nullptr);
        p->Release();
    }
    m_accessible = nullptr;
}

void DocEditor::AccessibleTextChanged(int)
{
    RaiseUiaEvent(this, UIA_Text_TextChangedEventId);
}

void DocEditor::AccessibleCaretMoved()
{
    RaiseUiaEvent(this, UIA_Text_TextSelectionChangedEventId);
}

void DocEditor::AccessibleNameChanged()
{
    Provider* p = ProviderOf(this);
    if (!p || !g_support || !UiaClientsAreListening()) return;
    VARIANT none, name;
    VariantInit(&none);
    VariantInit(&name);
    name.vt = VT_BSTR;
    name.bstrVal = SysAllocString(GetName().wc_str());
    UiaRaiseAutomationPropertyChangedEvent(p, UIA_NamePropertyId, none, name);
    VariantClear(&name);
}

WXLRESULT DocEditor::MSWWindowProc(WXUINT message, WXWPARAM wParam, WXLPARAM lParam)
{
    if (message == WM_GETOBJECT && (LONG)lParam == UiaRootObjectId && g_support && m_accessible) {
        return UiaReturnRawElementProvider((HWND)GetHWND(), wParam, lParam, ProviderOf(this));
    }
    return wxScrolledCanvas::MSWWindowProc(message, wParam, lParam);
}

void wxd_DocEditor_SetSupportUIAutomation(bool on)
{
    g_support = on;
}

#else

#include "../include/wxdragon.h"

// Global, Internationalization, Support UI Automation is used on Windows only.
void wxd_DocEditor_SetSupportUIAutomation(bool) {}

#endif // __WXMSW__
