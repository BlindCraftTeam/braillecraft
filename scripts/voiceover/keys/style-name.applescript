-- Option+8: the current style; pressed twice quickly in a table, the row and column.
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set s to my statusText(statusLine)
	if my pressedTwice("style") and my inTable(s) then
		set r to my afterLabel(s, "Row")
		set c to my afterLabel(s, "Col")
		my speak("Row " & r & " Column " & c)
		return
	end if
	if my inTable(s) then
		my speak(s)
	else
		set style to my afterLabel(s, "Current Style:")
		if style is "" then set style to "not in document window"
		my speak(style)
	end if
end run
