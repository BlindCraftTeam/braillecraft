-- Option+T: the current table entry (the table keys of the JAWS scripts; Option with an arrow is not a Keyboard
-- Commander key, so the entry is read after the move).
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set s to my statusText(statusLine)
	if not my inTable(s) then
		my speak("not in a table")
		return
	end if
	set {row, col, rowText} to my caretOf(area)
	set c to 1
	try
		set c to (my afterLabel(s, "Col")) as integer
	end try
	set tid to AppleScript's text item delimiters
	if my isCodedView(s) then
		set AppleScript's text item delimiters to {"[<]", "[>]"}
	else if s contains "Linear" then
		set AppleScript's text item delimiters to {"; ", ": "}
	else if s contains "Listed" then
		set AppleScript's text item delimiters to tid
		my speak(rowText)
		return
	else
		set AppleScript's text item delimiters to {" ..", "..", "  "}
	end if
	set parts2 to text items of rowText
	set AppleScript's text item delimiters to tid
	set cells to {}
	repeat with p in parts2
		if my trimRight(my trimLeft(p as text)) is not "" then set end of cells to my trimRight(my trimLeft(p as text))
	end repeat
	set entry to ""
	if c > 0 and c <= (count of cells) then set entry to item c of cells
	if my isBrailleDoc(w) then
		my speak(my cellsOf(entry))
	else if entry is "" then
		my speak("blank")
	else
		my speak(entry)
	end if
end run
