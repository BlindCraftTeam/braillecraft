-- Option+L: the last word on the line (Alt+End in the JAWS scripts; End is not a Keyboard Commander key).
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set s to my statusText(statusLine)
	if my isCodedView(s) then
		my speak("location not available in coded view, press Option F3 to turn codes off")
		return
	end if
	set {row, col, rowText} to my caretOf(area)
	set t to my trimRight(rowText)
	set tid to AppleScript's text item delimiters
	set AppleScript's text item delimiters to {" ", "-"}
	set parts2 to text items of t
	set AppleScript's text item delimiters to tid
	set word1 to ""
	if (count of parts2) > 0 then set word1 to item -1 of parts2
	if my isBrailleDoc(w) then
		my speak(my cellsOf(word1))
	else if word1 is "" then
		my speak("blank")
	else
		my speak(word1)
	end if
end run
