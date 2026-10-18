-- Option+period: the number of characters on the line; twice quickly, the left spaces.
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set s to my statusText(statusLine)
	if my isCodedView(s) then
		my speak("coded view")
		return
	end if
	set {row, col, rowText} to my caretOf(area)
	if my pressedTwice("length") then
		my speak((((length of rowText) - (length of my trimLeft(rowText))) as text) & " left spaces")
	else
		my speak(((length of rowText) as text) & " total characters.")
	end if
end run
