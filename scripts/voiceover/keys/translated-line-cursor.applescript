-- Option+semicolon: keyboard focus (and with it the VoiceOver cursor) to the translated line, and back to the editing
-- area.
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	tell application "System Events"
		set onTrans to false
		if trans is not missing value then
			try
				set onTrans to focused of trans
			end try
		end if
	end tell
	if onTrans then
		tell application "System Events" to set focused of area to true
		my speak("cursor at caret")
		return
	end if
	if trans is missing value then
		set trans to my showTranslatedLine()
		if trans is missing value then
			my speak("translated line not available")
			return
		end if
		my speak("turning on translated line")
	end if
	tell application "System Events" to set focused of trans to true
	my speak("cursor at translated line")
end run
