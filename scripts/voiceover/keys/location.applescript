-- Option+9: the location: page, line and column.
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set s to my statusText(statusLine)
	if my isCodedView(s) then
		my speak("location not available in coded view, press Option F3 to turn codes off")
		return
	end if
	my speak("P" & my afterLabel(s, "Page:") & " L" & my afterLabel(s, "Line:") & " C" & my afterLabel(s, "Column:"))
end run
