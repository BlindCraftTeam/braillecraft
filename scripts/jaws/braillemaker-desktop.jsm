;message file for BrailleMaker

const

;string compares
	scBrailleDocument = "Braille Document",
scBrailleDocumentNew = "Braille Document",
		scPrintDocument = "Print Document",
	scDocument = "Document",
	scPage = "Page:",
	scLine = "Line:",
	scColumn = "Column:",
	scUnicode = "U+",
	scCurrentStyle = "Current Style:",
	scMDI = "MDI",
	scRow = "Row",
	scCol = "Col",
	scSlash = "/",
	scEllipsis = "...",
	scKeys = "Keys:",
	scKeys6 = ":6",
	scKeys1 = "Keys:1",
	
	scErrorMessage0 = "Error message 0",
	scOK = "OK",

;Keystrokes:
;These are the same as their script names in braillemaker-desktop.jkm
;Prefix ks is added so they're known as keystrokes.
	ksSpeakTransLine = "Alt+7",
	ksControlF8Press = "Control+F8",
	ksHighlighting = "Control+H",
	ksBrailleDocInputEchoToggle = "Alt+P",
ksSpeakSpellCheckWord="Alt+Shift+L",
;window names
	wnFileNew = "File: New...",
	wnFind = "Edit: Find...",
	wnReplace = "Edit: Replace...",
	wnSpellCheck = "Spell Check...",
	wnSuggestedReplacements = "Suggested replacements:",
	wnConfirmReplacement = "Confirm Replacement",
	wnOK = "OK",
	wnApp = "BrailleMaker",
	wnDialog = "dialog",
	UO_BrailleLineSpeakingLevel = "Braille line speaking level",
	UO_EchoPC6KeyEntry = "Echo PC 6-key entry", 
	sHKey_PREFERRED_BRL_PROFILE_LIST="PreferredBrailleProfiles",
	c1msgNoPreferredBrailleProfileListConfigured_L = "no braille profile list configured",
	c1msgNoPreferredBrailleProfileListConfigured_S = "no braille profile list configured",
csBrailleDocInputEcho = " same as JAWS typing echo.; silent.; characters.; words.; characters and words.",
csSpace = " "
; xxx
; The lines below for speaking characters as braille cells can be localized.
; Each line tells JAWS how to speak the next 16 Unicode characters as braille cells.
; using semicolon is the delimiter
; The first four lines are for ASCII characters spoken in English context.
; the next three lines are for characters in the Russian braille encoding.
; The next four lines are for characters on Unicode page 28.

; first four below for North American braille encoding
; They give English-based speaking of braille cells.
; You can edit these four lines to speak the braille cells in the North American braille encoding differently.
; After editing these lines, save this file and recompile braillemaker-desktop.jss.

const

	csBrailleChars32 = "space;t h e sign;dot 5;number sign;e d sign;s h sign;a n d sign;dot 3;o f sign;w i t h sign;c h sign;i n g sign;dot 6;dots 3 6;dots 4 6;s t sign",
	csBrailleChars48 = "low j;low a;low b;low c;low d;low e;low f;low g;low h;low i;w h sign;dots 5 6;g h sign;f o r sign;a r sign;t h sign; dot 4",
	csBrailleChars91 = "o w sign;o u sign;e r sign;dots 4 5;dots 4 5 6;dot 4;a;b;c;d;e;f;g;h;i;j",
	csBrailleChars107 = "k;l;m;n;o;p;q;r;s;t;u;v;w;x;y;z;o w sign;o u sign;e r sign;dots 4 5;dots 4 5 6",
	csBrailleChars123 = "dots 4 6;dots 4 5 6;dot 6;dots 1 2 4 5 6;hard space",

; three lines below for Russian braille encoding
	csBrailleChars1072 = "dot 1;dots 1 2;dots 2 4 5 6;dots 1 2 4 5;dots 1 4 5;dots 1 5;dots 2 4 5;dots 1 3 5 6;dots 2 4;dots 1 2 3 4 6;dots 1 3;dots 1 2 3;dots 1 3 4;dots 1 3 4 5;dots 1 3 5;dots 1 2 3 4",
	csBrailleChars1088 = "dots 1 2 3 5;dots 2 3 4;dots 2 3 4 5;dots 1 3 6;dots 1 2 4;dots 1 2 5;dots 1 4;dots 1 2 3 4 5;dots 1 5 6;dots 1 3 4 6;dots 1 2 3 5 6;dots 2 3 4 6;dots 2 3 4 5 6;dots 2 4 6;dots 1 2 5 6;dots 1 2 4 6;dots 1 6",

; four lines below for Unicode braille patterns (Unicode page 28)
	csBrailleChars10240 = "space;a;low a;b;dot 3;k;low b;l;dot 4;c;i;f;s t sign;m;s;p",
	csBrailleChars10256 = "dot 5;e;low c;h;low i;o;low f;r;dots 4 5;d;j;g;a r sign;n;t;q",
	csBrailleChars10272 = "dot 6;c h sign;low e;g h sign;hyphen;u;low h;v;dots 4 6;s h sign;o w sign;e d sign;i n g sign;x;t h e sign;a n d sign",
	csBrailleChars10288 = "dots 5 6;w h sign;low d;o u sign;low j;z;low g;o f sign;dots 4 5 6;t h sign;w;e r sign;number sign;y;w i t h sign;f o r sign"

;end const

messages
@msgLocalBlank
blank
@@
@msgLocalSpace
space
@@
@msgLocalHardSpace
hard space
@@
@msgLocalAssistedHyphen
assisted hyphen
@@
@msgLocalHardHyphen
hard hyphen
@@
@msgUnicode
U+
@@
@msgHex
 Hex
 @@
@msgDecimal
 Decimal
 @@
@msgAppName
braillemaker-desktop
@@
@msgAppStart
for JAWS hot keys in BrailleMaker press %KeyFor(HotKeyHelp).
for troubleshooting problems with BrailleMaker and JAWS press %KeyFor(ScriptFileName).
@@
@msgJHotKeyHelp_L
To troubleshoot problems with BrailleMaker and JAWS press %KeyFor(ScriptFileName).
To hear the translated line press  %KeyFor(SpeakTransLine).
To hear the current style  press %KeyFor(SayStyleName).
To hear the current row and column numbers in a table  press (Alt+8) twice quickly.
To hear the current location press  %KeyFor(SayLocation).
To hear the current document name and status line press  %KeyFor(SayDocNameAndStatusLine).
To toggle use of translated line on or off for up and down in braille, press %KeyFor(ReadLineInBrailleDocToggle) or use JAWS Quick Settings.
To toggle reading of braille characters as English braille signs on or off, use JAWS QuickSettings.
To toggle style change and line skip announcements, press %KeyFor(ReadStyleChangeToggle), or use JAWS Quick Settings.
To toggle on or off showing requested status info, like the current location, on your braille display press %KeyFor(ShowRequestedStatusInfoOnBrailleDisplayToggle) or use JAWS Quick Settings.
To toggle on or off reading lines by word in print coded view, use JAWS Quick Settings.
To toggle the keyboard echo for entry in a braille document press %KeyFor(BraillingEchoToggle) or use Quick Settings; some choices now include reading the completed braille word when you press space.
To toggle on or off the reading of the just-completed braille word when you press space, press %KeyFor(SpeakCompletedBrailleWordsToggle).
To read the current line of text showing in spell check or replace, press %KeyFor(ReadWordInContext).
To hear the current unknown word in the spell checker press %KeyFor (SpeakSpellCheckWord).
To toggle between JAWS cursor on translated line and PC cursor press (Alt+Semicolon).
To hear the current line position press %KeyFor (SayLinePosition) (Alt+Comma).
To hear the number of characters to the end of the line press (Alt+Comma) twice quickly.
To hear the number of characters on the line press %KeyFor (SayTotalAndLeftOffset) (Alt+Period).
To hear the number of left spaces press (Alt+Period) twice quickly.
To hear the last word on the line press (Alt+End).
To move down one row in a table and speak the entry, press %KeyFor (NextRowEntry).
To move up one row in a table and speak the entry, press %KeyFor (PriorRowEntry).
To move right one column in a table and speak the entry, press %KeyFor (NextColumnEntry).
To move left one column in a table and speak the entry, press %KeyFor (PriorColumnEntry).
To speak the current table entry press %KeyFor(SayTableEntry).
@@
@msgJHotkeyHelp_S
Troubleshoot problems with BrailleMaker and JAWS: %KeyFor(ScriptFileName).
Speak the translated line: %KeyFor(SpeakTransLine).
Say the current style: %KeyFor(SayStyleName).
Say the table row and column numbers: Alt+8 twice quickly.+Period).
Say the current location: %KeyFor(SayLocation).
Hhear the document name and status line: %KeyFor(SayDocNameAndStatusLine).
Ttoggle use of translated line on or off for up and down in braille: %KeyFor(ReadLineInBrailleDocToggle) or JAWS QuickSettings.
Toggle echo for entry in braille documents: %KeyFor(BraillingEchoToggle); some choices include reading the completed braille word when you press space.
Toggle reading of completed braille words when you press space: %KeyFor(SpeakCompletedBrailleWordsToggle).
Toggle reading of braille characters as English braille signs: JAWS QuickSettings.
Toggle announcing of style change and skipped lines: press %KeyFor(ReadStyleChangeToggle) or JAWS Quick Settings.
Toggle on or off showing requested status info on braille display: %KeyFor(ShowRequestedStatusInfoOnBrailleDisplayToggle) or JAWS Quick Settings.
Ttoggle on or off reading lines by word in print coded view: JAWS Quick Settings. 
Rread the current line of text shown in spell check or replace: %KeyFor(ReadWordInContext).
Toggle between JAWS cursor on translated line and PC cursor: Alt+Semicolon.
Hear the current line position: %KeyFor (SayLinePosition) (Alt+Comma).
Hear the number of left spaces: %KeyFor (SayLeftOffsetAndTotal) (Alt+Period).
Hear the number of characters on the line: (Alt+Period) twice quickly.
Hear the last word on the line: (Alt+End).
Move down one row in a table and speak the entry: %KeyFor (NextRowEntry).
Move up one row in a table and speak the entry: %KeyFor (PriorRowEntry).
Move right one column in a table and speak the entry: %KeyFor (NextColumnEntry).
Move left one column in a table and speak the entry: %KeyFor (PriorColumnEntry).
Speak the current table entry: %KeyFor(SayTableEntry).
@@
@msgAppTrouble
You are using JAWS scripts for BrailleMaker dated October 15, 2026.
1. In BrailleMaker's Global Menu -> Internationalization, the item called UI Automation determines how BrailleMaker communicates with JAWS and other screen readers. Many improvements have been made in BrailleMaker's UI Automation.
2. To view multiple lines of a BrailleMaker document on a multiline braille display, you need to turn on UI Automation in BrailleMaker's Global -> Internationalization.
3. If UI Automation is off and Global -> Internationalization has the setting Prefer Unicode Braille Patterns, only Print font works properly in brraille documents.
When JAWS views braille shown as Unicode braille patterns in Braille or SimBraille font, the characters it speaks and shows on the braille display are wrong; the braille seems like garbage.
To fix this, change Global -> Default Views -> Default font for braille documents to Print.
4. If JAWS has started saying blank whenever you try to read text in BrailleMaker, try turning on UI Automation. Alternatively press Alt+Space and then R to restore the screen. Pressing Alt+Space followed by X after that to maximize the screen is fine.
Note that if the JAWS task list includes the item "Microsoft on top" or "Windows widgets on top," these fixes may not work; you may need to restart your computer.
5. If you get garbage when entering braille from your braille display keyboard, press F2 to turn off PC 6-key entry.
@@
@msgSpelled			
spelled
@@
@msgPage
Page 
@@
@msgTableRow
R
@@
@msgTableCol
C
@@
@msgRow
Row 
@@
@msgCol
Column 
@@
@msgNewLine
new line
@@
@msgOneSpace
one space
@@
@msgMultipleSpaces
 spaces
 @@
@msgSpaceAtEndOfLine
 space/endd of line
@@
@msgEndOfLine
end of line
@@
@msgTurningOnTransLine
turning on translated line
@@
@msgTransLineNotFound
translated line not available
@@
@msgTransTableError
translation not available
@@
@msgJawsCursorOnTransLine
JAWS cursor at start of translated line
@@
@msgTransLineOn
translated line on
@@
@msgTransLineOff
translated line off
@@
@msgEndLine
end of translated line
@@
@msgStartLine
start of translated line
@@
@msgNotInDocument
not in document window
@@
@msgCode
code
@@
@msgStyle
style
@@
@msgBeginStyle
begin style
@@
@msgEndStyle
end style
@@
@msgReadBackTrans
use translated line for up and down in braille
@@
@msgReadBrailleLinesAsBrailleChars
read line as braille characters for up and down in braille
@@
@msgSilentBrailleLines
silent for up and down in braille
@@
@msgSpeakStyleChange
Announcements: Change of style on.
@@
@msgStyleChangeOff
Announcements: change of style off.
@@
@msgSpeakSkips
Skipped lines on.
@@
@msgSkipsOff
Skipped lines off.
@@
@msgBrailleDocInputEcho
Echo for Braille entry is 
@@
@msgBraillingEchoCharacters
Echo for Braille entry is characters
@@
@msgBraillingEchoWords
Echo for braille entry is words.
@@
@msgBraillingEchoCharactersAndWords
Echo for braille entry is characters and words.
@@
@msgBraillingEchoSameAsTypingEcho
Use typing Echo for echoing braille entry.
@@
@msgShowRequestedStatusInfoOnBrailleDisplayOn
show requested status info on braille display
@@
@msgShowRequestedStatusInfoOnBrailleDisplayOff
do not show requested status info on braille display
@@
@msgNoLocation_l
location not available in coded view
press Alt+F3 to turn codes off
@@
@msgNoLocation_s
in coded view
@@
@msgCodesOn_l
coded view
@@
@msgCodesOn_s
coded
@@
@msgCodesOff_l
formatted view
@@
@msgCodesOff_s
formatted
@@
@msgHighlightingOn
highlighting on
@@
@msgHighlightingOff
highlighting off
@@
@msgCosmoConnected
Cosmo connected
@@
@msgCosmoNotConnected
Cosmo not connected
@@
@msgPerkinsOn
6 key on
@@
@msgPerkinsOff
6 key off
@@
@msgFind
Find
@@
@msgReplace
Replace
@@
@msgShortcutPreferences
Shortcut Preferences
@@
@msgInsertForGrade0
Insert for grade 0
@@
@msgMdiClient
MDI client
@@
@msgLineSkip
line skip
@@
@msgBlankLines
blank lines
@@
@msgPageId
P
@@
@msgLineId
 L
@@
@msgColId
 C
@@
@msgCatchingUp
catching up
@@
@msgLineLength
Line length 
@@
@msgTotalCharacters
 total characters.
@@
@msgLeftSpaces
 left spaces
@@
@msgLinePosition
position
@@
@msgToEndOfLine
  to last nonspace character
@@
@msgBackspace
backspace
@@
@msgAppOptionsHlp
balloon
@@
@msgSelected
selected
@@
@msgUnselected
unselected
@@
@msgScrollingTime
scrolling time
@@
@msgScrollingDown
scrolling down
@@
@msgScrollingUp
scrolling up
@@
@msgEnteringTable
entering table
@@
@msgExitingTable
exiting table
@@
@msgLinear
Linear
@@
@msgListed
Listed
@@
@msgTwoPage
Two-Page
@@
@msgMatrix
Matrix
@@
@msgOf
of
@@
@msgNoTab
Pressing tab here will exit the table.
@@
@msgNoShiftTab
Pressing Shift+Tab here will exit the table.
@@
@msgNone
None
@@
@msgSpellCompletedBrailleWordsOn_l
echo completed braille words on
@@
@msgSpellCompletedBrailleWordsOff_l
echo completed braille words off
@@
@msgOn
on
@@
@msgOff
off
@@
@msgLineDown
   
@@
@msgEchoBrailleCharsOn
on
@@
@msgEchoBrailleCharsOff
off
@@
@msgUnicodeValueNotOnStatusLine
The Unicode value does not appear on the status line. Turn off the use of status line for Unicode values.
@@
@msgUnicodePrefix
U+
@@

endMessages

