-- Option+C: whether a braille keyboard is connected (the + after Keys on the status line).
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set k to my afterLabel(my statusText(statusLine), "Keys:")
	if k contains "+" then
		my speak("braille keyboard connected")
	else
		my speak("braille keyboard not connected")
	end if
end run
