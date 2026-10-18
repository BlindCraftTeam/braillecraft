;script header file for BrailleCraft for Windows

const

;window classes
; the reference layout stacks document windows in an MDI client window; BrailleCraft shows its documents as the
; tabs of one tab control (class SysTabControl32, read by JAWS as a tab control: the active document's name is the
; selected tab), so no window has the MDI client class and the tests against it never match
	wcMDIClient = "MDIClient",
	wcTabControl = "SysTabControl32",
; the editing area of a document window: the program's own drawn editing window, registered under its own class name
; ("Document Editing Window" to UI Automation), as the reference program's editing window has a class of its own
	wcEditor = "BrailleCraftEditor",
; the dialog edit fields that take codes (Find, Replace, Shortcut Preferences, styles, header, footer, table of
; contents) have a class of their own in the reference layout; BrailleCraft's are ordinary edit fields, so no window
; has this class
	wcCodesEdit = "BrailleCraft_CODESEDIT",

; other constants
	Editor_Control=0,
wtUnknown = "unknown"
