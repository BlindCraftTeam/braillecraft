-- Option+comma: the position on the line; twice quickly, the characters to the last non-space character.
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set s to my statusText(statusLine)
	set {row, col, rowText} to my caretOf(area)
	set pos to col
	set c to my afterLabel(s, "Column:")
	if c is not "" then
		try
			set pos to c as integer
		end try
	end if
	set total to length of rowText
	if my pressedTwice("position") then
		my speak(((length of my trimRight(rowText)) - pos) as text) & " to last nonspace character"
	else if total - pos + 1 is 0 then
		my speak("position " & pos & " end of line")
	else
		my speak("position " & pos)
	end if
end run
