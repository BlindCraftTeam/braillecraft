-- BrailleCraft VoiceOver scripts: the part every key script shares. tools/voiceover/build.py puts this text in front
-- of each key script in scripts/voiceover/keys, so each compiled .scpt stands alone (VoiceOver runs one file per
-- key). The desktop program only; the web build has no scripts.
--
-- The scripts read the front BrailleCraft document window through System Events: the editing area (an accessibility
-- text area named Document Editing Window; its text is one row per line, the caret is the selected range and the
-- row is the insertion point line number), the status line (a static text: Current Style: x  Page:n  Line:n
-- Column:n  c  Keys:1, or the table text with Row r/n Col c/n in a table; Page, Line and Column are left out in the
-- coded view; Keys:6 for six-key entry, H after it for highlighting, + for a connected braille keyboard), and the
-- translated line (a read-only text field, Error message 0 when the document has no translation, absent while View,
-- Translated Line is off). They speak through VoiceOver (VoiceOver Utility, General, Allow VoiceOver to be controlled
-- with AppleScript), and through the system voice when that is off.

property bundleId : "dev.blindcraft.braillecraft"
property editorName : "Document Editing Window"
property twiceSeconds : 0.7
property stampFile : "/tmp/braillecraft-voiceover-stamp"

-- Speaks `t`: VoiceOver when it takes AppleScript, else the system voice.
on speak(t)
	try
		tell application "VoiceOver" to output t
	on error
		say t
	end try
end speak

-- The BrailleCraft process, or missing value when the program is not running.
on bcProcess()
	tell application "System Events"
		try
			return first application process whose bundle identifier is bundleId
		on error
			return missing value
		end try
	end tell
end bcProcess

-- Every element of a window, front to back of the tree, with the role each has.
on elementsOf(w)
	tell application "System Events" to return entire contents of w
end elementsOf

-- The front document window of the program and its three parts: {window, editing area, status line, translated line}.
-- The translated line is missing value while it is hidden; the whole record is missing value when no document window
-- is in front. Each document is a window of its own, the open documents tabs of one window (or of several): the main
-- window (the window of the selected tab of the front window) is tried first, then the others as the system lists
-- them. The program's own window, which holds the menu bar, is never shown and has no editing area.
on docParts()
	set proc to bcProcess()
	if proc is missing value then return missing value
	tell application "System Events" to set ws to windows of proc
	set mains to {}
	set others to {}
	repeat with w in ws
		set isMain to false
		try
			tell application "System Events" to set isMain to ((value of attribute "AXMain" of w) is true)
		end try
		if isMain then
			set end of mains to (contents of w)
		else
			set end of others to (contents of w)
		end if
	end repeat
	repeat with w in (mains & others)
		set parts to my partsOf(contents of w)
		if parts is not missing value then return parts
	end repeat
	return missing value
end docParts

-- A window's three parts as docParts gives them, or missing value when it is no document window.
on partsOf(w)
	set area to missing value
	set statusLine to missing value
	set trans to missing value
	tell application "System Events"
		repeat with e in (my elementsOf(w))
			set r to ""
			try
				set r to role of e
			end try
			if r is "AXTextArea" and area is missing value then
				set area to e
			else if r is "AXStaticText" and statusLine is missing value then
				set v to ""
				try
					set v to value of e
				end try
				if v starts with "Current Style" or v contains "Row " then set statusLine to e
			else if r is "AXTextField" and trans is missing value then
				set trans to e
			end if
		end repeat
	end tell
	if area is missing value then return missing value
	return {w, area, statusLine, trans}
end partsOf

-- Where the document window's tab is among the tabs of its window: "tab 2 of 3"; "" when the window has one tab or
-- its tab bar cannot be read. The tab bar is the system's tab group, a radio button per tab, the selected one on.
on tabPlace(w)
	try
		tell application "System Events"
			set bars to (every UI element of w whose role is "AXTabGroup")
			if bars is {} then
				set bars to {}
				repeat with e in (my elementsOf(w))
					try
						if role of e is "AXTabGroup" then set end of bars to (contents of e)
					end try
				end repeat
			end if
			repeat with b in bars
				set tabList to (every radio button of b)
				set n to count of tabList
				if n > 1 then
					repeat with i from 1 to n
						set v to 0
						try
							set v to (value of item i of tabList) as integer
						end try
						if v is 1 then return "tab " & i & " of " & n
					end repeat
				end if
			end repeat
		end tell
	end try
	return ""
end tabPlace

-- The title of the document window.
on docTitle(w)
	tell application "System Events" to return name of w
end docTitle

on isBrailleDoc(w)
	return my docTitle(w) contains "Braille Document"
end isBrailleDoc

-- The status line text, "" when there is none.
on statusText(statusLine)
	if statusLine is missing value then return ""
	tell application "System Events" to return value of statusLine
end statusText

-- The text after `lab` in the status line, up to the next double space.
on afterLabel(s, lab)
	set p to offset of lab in s
	if p is 0 then return ""
	set rest to text (p + (length of lab)) thru -1 of s
	repeat while rest starts with " "
		if (length of rest) is 1 then return ""
		set rest to text 2 thru -1 of rest
	end repeat
	set q to offset of "  " in rest
	if q is 0 then return rest
	if q is 1 then return ""
	return text 1 thru (q - 1) of rest
end afterLabel

-- The coded view leaves Page, Line and Column off the status line.
on isCodedView(s)
	return (s is not "") and (s does not contain "Page:")
end isCodedView

on inTable(s)
	return s contains "Row "
end inTable

-- The translated line text, "" while hidden.
on transText(trans)
	if trans is missing value then return ""
	tell application "System Events" to return value of trans
end transText

-- Turns View, Translated Line on through the menu and finds the field again.
on showTranslatedLine()
	set proc to bcProcess()
	tell application "System Events" to click menu item "Translated Line" of menu "View" of menu bar 1 of proc
	delay 0.4
	set parts to my docParts()
	if parts is missing value then return missing value
	return item 4 of parts
end showTranslatedLine

-- The caret: {row number from 1, column from 1, the text of the row}.
on caretOf(area)
	tell application "System Events"
		set t to value of area
		set n to value of attribute "AXInsertionPointLineNumber" of area
		set r to value of attribute "AXSelectedTextRange" of area
	end tell
	set loc to 0
	try
		set loc to item 1 of r
	on error
		try
			set loc to r
		end try
	end try
	set rowList to paragraphs of t
	if n < 0 then set n to 0
	if n > ((count of rowList) - 1) then set n to (count of rowList) - 1
	set startOfRow to 0
	repeat with i from 1 to n
		set startOfRow to startOfRow + (length of (item i of rowList)) + 1
	end repeat
	set rowText to ""
	if (count of rowList) > 0 then set rowText to item (n + 1) of rowList
	return {n + 1, loc - startOfRow + 1, rowText}
end caretOf

-- True when this key was pressed again within twiceSeconds (the second form of a key).
on pressedTwice(keyName)
	-- Milliseconds as an integer, so no decimal separator is read.
	set now to (do shell script "perl -MTime::HiRes=time -e 'printf \"%d\", time * 1000'") as integer
	set again to false
	try
		set prev to paragraphs of (do shell script "cat " & quoted form of stampFile)
		if (item 1 of prev) is keyName and (now - ((item 2 of prev) as integer)) < (twiceSeconds * 1000) then set again to true
	end try
	do shell script "printf '%s\\n%s\\n' " & quoted form of keyName & " " & quoted form of (now as text) & " > " & quoted form of stampFile
	return again
end pressedTwice

on trimRight(s)
	repeat while s ends with " "
		if (length of s) is 1 then return ""
		set s to text 1 thru -2 of s
	end repeat
	return s
end trimRight

on trimLeft(s)
	repeat while s starts with " "
		if (length of s) is 1 then return ""
		set s to text 2 thru -1 of s
	end repeat
	return s
end trimLeft

on splitList(s)
	set tid to AppleScript's text item delimiters
	set AppleScript's text item delimiters to ";"
	set l to text items of s
	set AppleScript's text item delimiters to tid
	return l
end splitList

-- Braille cell names (the tables of the BrailleCraft JAWS scripts, one name per character value).
property na32 : "space;t h e sign;dot 5;number sign;e d sign;s h sign;a n d sign;dot 3;o f sign;w i t h sign;c h sign;i n g sign;dot 6;dots 3 6;dots 4 6;s t sign"
property na48 : "low j;low a;low b;low c;low d;low e;low f;low g;low h;low i;w h sign;dots 5 6;g h sign;f o r sign;a r sign;t h sign;dot 4"
property na91 : "o w sign;o u sign;e r sign;dots 4 5;dots 4 5 6;dot 4;a;b;c;d;e;f;g;h;i;j"
property na107 : "k;l;m;n;o;p;q;r;s;t;u;v;w;x;y;z"
property na123 : "dots 4 6;dots 4 5 6;dot 6;dots 1 2 4 5 6;hard space"
property ru1072 : "dot 1;dots 1 2;dots 2 4 5 6;dots 1 2 4 5;dots 1 4 5;dots 1 5;dots 2 4 5;dots 1 3 5 6;dots 2 4;dots 1 2 3 4 6;dots 1 3;dots 1 2 3;dots 1 3 4;dots 1 3 4 5;dots 1 3 5;dots 1 2 3 4"
property ru1088 : "dots 1 2 3 5;dots 2 3 4;dots 2 3 4 5;dots 1 3 6;dots 1 2 4;dots 1 2 5;dots 1 4;dots 1 2 3 4 5;dots 1 5 6;dots 1 3 4 6;dots 1 2 3 5 6;dots 2 3 4 6;dots 2 3 4 5 6;dots 2 4 6;dots 1 2 5 6;dots 1 2 4 6;dots 1 6"
property uni : "space;a;low a;b;dot 3;k;low b;l;dot 4;c;i;f;s t sign;m;s;p;dot 5;e;low c;h;low i;o;low f;r;dots 4 5;d;j;g;a r sign;n;t;q;dot 6;c h sign;low e;g h sign;hyphen;u;low h;v;dots 4 6;s h sign;o w sign;e d sign;i n g sign;x;t h e sign;a n d sign;dots 5 6;w h sign;low d;o u sign;low j;z;low g;o f sign;dots 4 5 6;t h sign;w;e r sign;number sign;y;w i t h sign;f o r sign"

-- The spoken name of one braille cell written as `ch`, or the character itself when it has none.
on cellName(ch)
	if ch is "" then return ""
	set v to id of ch
	if v is 30 or v is 160 then return "hard space"
	if v >= 32 and v < 48 then return item (v - 31) of my splitList(na32)
	if v >= 48 and v <= 64 then return item (v - 47) of my splitList(na48)
	if v >= 65 and v <= 90 then set v to v + 32
	if v >= 91 and v < 107 then return item (v - 90) of my splitList(na91)
	if v >= 107 and v < 123 then return item (v - 106) of my splitList(na107)
	if v >= 123 and v < 128 then return item (v - 122) of my splitList(na123)
	if v >= 1072 and v < 1088 then return item (v - 1071) of my splitList(ru1072)
	if v >= 1088 and v < 1105 then return item (v - 1087) of my splitList(ru1088)
	if v >= 10240 and v < 10304 then return item (v - 10239) of my splitList(uni)
	return ch
end cellName

-- `t` spoken cell by cell, runs of two or more spaces as a count.
on cellsOf(t)
	set out to {}
	set n to length of t
	set i to 1
	repeat while i <= n
		if (character i of t) is " " and i < n and (character (i + 1) of t) is " " then
			set j to i
			repeat while j <= n and (character j of t) is " "
				set j to j + 1
			end repeat
			set end of out to ((j - i) as text) & " spaces"
			set i to j
		else
			set end of out to my cellName(character i of t)
			set i to i + 1
		end if
	end repeat
	set tid to AppleScript's text item delimiters
	set AppleScript's text item delimiters to ", "
	set s to out as text
	set AppleScript's text item delimiters to tid
	return s
end cellsOf

-- The whole: the document parts, or the message and missing value when no document window is in front.
on needDoc()
	set parts to my docParts()
	if parts is missing value then my speak("not in document window")
	return parts
end needDoc
