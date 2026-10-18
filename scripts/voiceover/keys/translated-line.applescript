-- Option+7: the translated line (the print of the line in a braille document, the braille cells in a print one).
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	if trans is missing value then
		set trans to my showTranslatedLine()
		if trans is missing value then
			my speak("translated line not available")
			return
		end if
		my speak("translated line on")
	end if
	set t to my transText(trans)
	if t is "Error message 0" then
		my speak("translation not available")
	else if my trimRight(t) is "" then
		my speak("blank")
	else if my isBrailleDoc(w) then
		my speak(t)
	else
		my speak(my cellsOf(t))
	end if
end run
