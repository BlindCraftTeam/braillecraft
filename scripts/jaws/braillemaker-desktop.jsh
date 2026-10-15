;script header file for BrailleMaker for Windows

const

;window classes
	wcMDIClient = "MDIClient",
; the editing area of a document window: a native rich edit control ("Document Editing Window")
	wcEditor = "RICHEDIT50W",
; the code editing window of the reference layout; BrailleMaker has no window of this class
	wcCodesEdit = "BrailleMaker_CODESEDIT",
; the status line: the status bar of the main window
	wcStatusBar = "msctls_statusbar32",

; other constants
	Editor_Control=0,
wtUnknown = "unknown"
