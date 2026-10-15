;script header file for BrailleMaker for Windows

const

;window classes
	wcMDIClient = "MDIClient",
; the editing area of a document window: a native rich edit control ("Document Editing Window")
	wcEditor = "RICHEDIT50W",
; the dialog edit fields that take codes (Find, Replace, Shortcut Preferences, styles, header, footer, table of
; contents) have a class of their own in the reference layout; BrailleMaker's are ordinary edit fields, so no window
; has this class
	wcCodesEdit = "BrailleMaker_CODESEDIT",

; other constants
	Editor_Control=0,
wtUnknown = "unknown"
