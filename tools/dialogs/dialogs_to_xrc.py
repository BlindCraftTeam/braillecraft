"""Turns decoded dialog layouts (dialogs-layout.json, from tools/pe_dialog_layout.py) into one wxWidgets XRC file of
native dialogs, positioned exactly as the layouts give them.

Usage: python3 -I dialogs_to_xrc.py <dialogs-layout.json>[,<more-layouts.json>...] <out.xrc> [<out dialog_table.rs>]
          [--xrc <program.xrc> <xrc-dialogs.json>]

With --xrc, dialogs the program defines in its own XRC file (sizer layouts, not dialog templates) are added as they
are. xrc-dialogs.json (kept with the decoded resource data, outside the repository) gives {"prefix": the prefix of the
program's control names, "dialogs": {name in the XRC file: menu command id}}: each dialog takes the name
dlg<menu command id>, and the prefix is dropped from its control names (controls are then found by name).

Several layout files (the program's and the embosser setup's) are read in order; from each only the English
dialogs are taken (language 1033 or 9), and a resource id already taken from an earlier file is skipped.

Every dialog becomes a wxDialog named dlg<resource id>; every control keeps its position and size in dialog units
(XRC "d" units, converted by wxWidgets with the dialog font as Windows does), its label with its access key, its
tab order (document order), and its visible, enabled, default-button, group and style flags. Controls are named
c<id>; ids 1 and 2 are the standard OK and Cancel (wxID_OK, wxID_CANCEL), as Windows treats them. The sizes are
client-area sizes (DLGTEMPLATE cx, cy); the program sets each dialog's client size from them when it opens one.
"""
import json
import sys
import xml.etree.ElementTree as ET
from xml.sax.saxutils import escape

XRC_NS = "{http://www.wxwidgets.org/wxxrc}"


def xrc_dialogs(path, mapping_path):
    """The mapped dialogs of the program's XRC file as XRC text, renamed."""
    mapping = json.load(open(mapping_path, encoding="utf-8"))
    prefix, wanted = mapping["prefix"], mapping["dialogs"]
    root = ET.parse(path).getroot()
    out = []
    for obj in root:
        name = obj.get("name")
        if obj.tag != XRC_NS + "object" or name not in wanted:
            continue
        for e in obj.iter():
            e.tag = e.tag.replace(XRC_NS, "")
            n = e.get("name")
            if n and n.startswith(prefix):
                e.set("name", n[len(prefix):])
        obj.set("name", "dlg%d" % wanted[name])
        ET.indent(obj, space="  ", level=1)
        out.append("  " + ET.tostring(obj, encoding="unicode").rstrip())
    if len(out) != len(wanted):
        raise SystemExit("XRC dialogs not found in %s" % path)
    return out

# Licensing, activation, registration and vendor-specific prompts are not part of BrailleCraft.
SKIP = {1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015,
        1056, 1057, 1059, 1070}
# The embosser setup's printer-driver installation and first-run wizard pages (10005 to 10016) are a later queue line
# (BrailleCraft queue line 10, M3b); the setup sheet itself is 10000 to 10004.
SKIP |= set(range(10005, 10017))
# The embosser setup sheet's pages, shown as tabs of the sheet (10000): written as panels, not dialogs.
PANELS = {10001, 10002, 10003, 10004}

# About (1016) keeps its layout with BrailleCraft's own text; the activation buttons and the original credits are
# left out (None). Controls 10 to 14 are filled at run time.
ABOUT = 1016
ABOUT_TEXT = {
    5: None, 6: None, 9: None, 20: None, 21: None, 22: None,
    7: "BrailleCraft for Windows, Linux and Mac",
    8: "Version %s",
    15: "Designed, implemented, and tested by:",
    16: "the BlindCraft Team",
    17: "Braille translation:",
    18: "the BrailleCraft translation engine and its tables",
    19: "Copyright 2026 BlindCraft Team and BrailleCraft contributors",
}
# About's texts 7 to 14 are reached by Tab while a screen reader runs (re/spec/04f): they are written as the static
# text class that accepts focus then (vendor/wxdragon-sys, WxdReaderStaticText).
READER_TAB_STOPS = {(ABOUT, n) for n in range(7, 15)}


# Titles and texts that name another program are replaced.
TITLES = {1052: "Selective Contractions", 1054: "BrailleCraft", 1061: "BrailleCraft Autosave Recovery Wizard"}
TEXTS = {
    (1036, 12): "Allow embedded codes, eg. [[*pg*]]",
    (1037, 16): '<a href="www.example.com">www.example.com</a>',
    (1060, 12): "",
    # Install JAWS Scripts: the same texts as crates/bc-desktop/src/jaws_install.rs.
    (1058, 6): "You have one or more JAWS® installations without the most up-to-date script files for "
               "BrailleCraft.  Installing the most recent script files (included with BrailleCraft) will help "
               "BrailleCraft to work better with JAWS.",
    (1058, 7): "Would you like to install the script files?  (Any existing BrailleCraft script files will be backed "
               "up.)",
    # Embosser setup sheet.
    (10000, 6): "BrailleCraft",
    (10001, 19): "(BrailleCraft will prompt if this is blank)",
    (10003, 19): "If you want to add interline Print and Braille capabilities to BrailleCraft using your Enabling "
                 "Technologies Braille Embosser please contact:",
}


# Lists whose resource asks for sorting but which the reference fills by inserting each item at a set place
# (CB_INSERTSTRING), which keeps the order given: the TranSend SE printer choice keeps Supported, Supported or
# Compatible, any printer, the order of the values it stands for (re/spec/25-emboss.md section 14.7 item 6).
UNSORTED = {(10003, 5)}


def name_of(c):
    return {1: "wxID_OK", 2: "wxID_CANCEL"}.get(c["id"], "c%d" % c["id"])


def label(text):
    # Win32 and wxWidgets both mark the access key with "&"; text needs XML escaping only.
    return escape(text or "")


def is_box(c):
    return c["class"] == "Button" and "BS_GROUPBOX" in c["style_flags"]


def is_radio(c):
    return c["class"] == "Button" and any(f in c["style_flags"] for f in ("BS_RADIOBUTTON", "BS_AUTORADIOBUTTON"))


def kind_of(c):
    """The control's kind for the program: how its value is read and kept."""
    f = c["style_flags"]
    if c["class"] == "Button":
        if "BS_GROUPBOX" in f:
            return "Box"
        if is_radio(c):
            return "Radio"
        if any(k in f for k in ("BS_CHECKBOX", "BS_AUTOCHECKBOX", "BS_3STATE", "BS_AUTO3STATE")):
            return "Check"
        return "Button"
    # The original's codes edit field is a text field (shown as one by `control`).
    if c["class"].endswith("_CODESEDIT"):
        return "Text"
    return {"Static": "Static", "Edit": "Text", "ListBox": "List",
            "ComboBox": "Combo"}.get(c["class"], "Static")


def common(c, out, indent):
    f = c["style_flags"]
    out.append('%s<pos>%d,%dd</pos>' % (indent, c["x"], c["y"]))
    out.append('%s<size>%d,%dd</size>' % (indent, c["width"], c["height"]))
    if "WS_VISIBLE" not in f:
        out.append('%s<hidden>1</hidden>' % indent)
    if "WS_DISABLED" in f:
        out.append('%s<enabled>0</enabled>' % indent)


def control(c, out, group_start, rid=None):
    f = c["style_flags"]
    cls = c["class"]
    ind = "      "
    styles = []
    if cls == "Button":
        kind = next((x for x in f if x.startswith("BS_") and x in (
            "BS_PUSHBUTTON", "BS_DEFPUSHBUTTON", "BS_CHECKBOX", "BS_AUTOCHECKBOX", "BS_RADIOBUTTON",
            "BS_AUTORADIOBUTTON", "BS_GROUPBOX", "BS_3STATE", "BS_AUTO3STATE")), "BS_PUSHBUTTON")
        if kind == "BS_GROUPBOX":
            wx = "wxStaticBox"
        elif kind in ("BS_CHECKBOX", "BS_AUTOCHECKBOX"):
            wx = "wxCheckBox"
        elif kind in ("BS_3STATE", "BS_AUTO3STATE"):
            wx, styles = "wxCheckBox", ["wxCHK_3STATE", "wxCHK_ALLOW_3RD_STATE_FOR_USER"]
        elif kind in ("BS_RADIOBUTTON", "BS_AUTORADIOBUTTON"):
            wx = "wxRadioButton"
            # A group starts where the layout marks one (WS_GROUP) or where a run of radio buttons begins after another
            # kind of control (dialogs 1028, 1037 and 1051 rely on that).
            if group_start:
                styles.append("wxRB_GROUP")
        else:
            wx = "wxButton"
    elif cls == "Static":
        wx = "wxStaticText"
        if "SS_CENTER" in f:
            styles.append("wxALIGN_CENTRE_HORIZONTAL")
        elif "SS_RIGHT" in f:
            styles.append("wxALIGN_RIGHT")
        if "SS_LEFTNOWORDWRAP" in f or "SS_SIMPLE" in f:
            styles.append("wxST_NO_AUTORESIZE")
    elif cls == "Edit" or cls.endswith("_CODESEDIT"):
        wx = "wxTextCtrl"
        if "ES_MULTILINE" in f:
            styles.append("wxTE_MULTILINE")
        if "ES_READONLY" in f:
            styles.append("wxTE_READONLY")
        if "ES_PASSWORD" in f:
            styles.append("wxTE_PASSWORD")
        if "ES_WANTRETURN" in f:
            styles.append("wxTE_PROCESS_ENTER")
    elif cls == "ListBox":
        wx = "wxListBox"
        if "LBS_EXTENDEDSEL" in f:
            styles.append("wxLB_EXTENDED")
        elif "LBS_MULTIPLESEL" in f:
            styles.append("wxLB_MULTIPLE")
        else:
            styles.append("wxLB_SINGLE")
        if "LBS_SORT" in f:
            styles.append("wxLB_SORT")
        if "LBS_DISABLENOSCROLL" in f:
            styles.append("wxLB_ALWAYS_SB")
    elif cls == "ComboBox" and "CBS_DROPDOWNLIST" in f:
        # A drop-down list with no typing is a choice control: on Windows the same native drop-down list, on macOS
        # the system's pop-up button, as a Mac program shows it.
        wx = "wxChoice"
        if "CBS_SORT" in f:
            styles.append("wxCB_SORT")
    elif cls == "ComboBox":
        wx = "wxComboBox"
        if "CBS_SIMPLE" in f:
            styles.append("wxCB_SIMPLE")
        else:
            styles.append("wxCB_DROPDOWN")
        if "CBS_SORT" in f:
            styles.append("wxCB_SORT")
    else:
        wx = "wxStaticText"  # an unknown class is shown as its text so nothing is silently dropped
    if wx == "wxStaticText" and (rid, c["id"]) in READER_TAB_STOPS:
        out.append('    <object class="%s" name="%s" subclass="WxdReaderStaticText">' % (wx, name_of(c)))
    else:
        out.append('    <object class="%s" name="%s">' % (wx, name_of(c)))
    if wx in ("wxButton", "wxCheckBox", "wxRadioButton", "wxStaticBox", "wxStaticText"):
        out.append('%s<label>%s</label>' % (ind, label(c["text"] if isinstance(c["text"], str) else "")))
    elif wx == "wxTextCtrl" and isinstance(c["text"], str) and c["text"]:
        out.append('%s<value>%s</value>' % (ind, label(c["text"])))
    if wx == "wxButton" and "BS_DEFPUSHBUTTON" in f:
        out.append('%s<default>1</default>' % ind)
    if styles:
        out.append('%s<style>%s</style>' % (ind, "|".join(styles)))
    common(c, out, ind)
    out.append('    </object>')


def main():
    args = sys.argv[1:]
    xrc = None
    if "--xrc" in args:
        i = args.index("--xrc")
        xrc = (args[i + 1], args[i + 2])
        del args[i:i + 3]
    sys.argv[1:] = args
    dialogs, seen = [], set()
    for path in sys.argv[1].split(","):
        for d in json.load(open(path, encoding="utf-8")):
            if d.get("language") not in (1033, 9) or d["resource"] in seen:
                continue
            seen.add(d["resource"])
            dialogs.append(d)
    out = ['<?xml version="1.0" encoding="UTF-8"?>',
           '<!-- Generated by tools/dialogs/dialogs_to_xrc.py from the dialog layouts; do not edit by hand. -->',
           '<resource xmlns="http://www.wxwidgets.org/wxxrc" version="2.5.3.0">']
    written = 0
    table = []
    for d in dialogs:
        rid = d["resource"]
        if rid in SKIP or "controls" not in d:
            continue
        controls = [dict(c, text=TEXTS[(rid, c["id"])]) if (rid, c["id"]) in TEXTS else c for c in d["controls"]]
        controls = [dict(c, style_flags=[f for f in c["style_flags"] if f not in ("CBS_SORT", "LBS_SORT")])
                    if (rid, c["id"]) in UNSORTED else c for c in controls]
        title = TITLES.get(rid, d["title"] or "BrailleCraft")
        if rid == ABOUT:
            title = "About BrailleCraft"
            kept = []
            for c in controls:
                if c["id"] in ABOUT_TEXT:
                    if ABOUT_TEXT[c["id"]] is None:
                        continue
                    c = dict(c, text=ABOUT_TEXT[c["id"]])
                kept.append(c)
            controls = kept
        if rid in PANELS:
            out.append('  <object class="wxPanel" name="dlg%s">' % rid)
            out.append('    <size>%d,%dd</size>' % (d["width"], d["height"]))
            out.append('    <style>wxTAB_TRAVERSAL</style>')
        else:
            out.append('  <object class="wxDialog" name="dlg%s">' % rid)
            out.append('    <title>%s</title>' % escape(title))
            out.append('    <size>%d,%dd</size>' % (d["width"], d["height"]))
            out.append('    <style>wxDEFAULT_DIALOG_STYLE</style>')
        # Group boxes first, so they sit behind the controls they frame (Windows draws them that way).
        for c in controls:
            if is_box(c):
                control(c, out, False)
        previous = None
        for c in controls:
            if is_box(c):
                continue
            start = is_radio(c) and ("WS_GROUP" in c["style_flags"] or previous is None or not is_radio(previous))
            control(c, out, start, rid)
            previous = c
        out.append('  </object>')
        written += 1
        table.append((rid, d["width"], d["height"], [(c["id"], kind_of(c)) for c in controls]))
    added = xrc_dialogs(*xrc) if xrc else []
    out.extend(added)
    out.append('</resource>')
    open(sys.argv[2], "w", encoding="utf-8").write("\n".join(out) + "\n")
    if len(sys.argv) > 3:
        rs = ["// Generated by tools/dialogs/dialogs_to_xrc.py from the dialog layouts; do not edit by hand.",
              "// Each dialog: resource id, client width and height in dialog units (DLGTEMPLATE cx, cy), and its",
              "// controls in tab order with their kind.", "",
              "use crate::dlg::Ctl;", "",
              "pub const DIALOGS: &[(i32, i32, i32, &[(i32, Ctl)])] = &["]
        for rid, w, h, ctls in table:
            rs.append("    (%d, %d, %d, &[%s])," % (rid, w, h, ", ".join("(%d, Ctl::%s)" % c for c in ctls)))
        rs.append("];")
        open(sys.argv[3], "w", encoding="utf-8").write("\n".join(rs) + "\n")
    print(written, "dialogs written,", len(dialogs) - written, "left out (licensing, activation, vendor prompts),",
          len(added), "added from the program's XRC file")


if __name__ == "__main__":
    main()
