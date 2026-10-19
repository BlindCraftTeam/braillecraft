-- Option+0: the document name, its place among the tabs of its window when there are several, and the status line.
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	set place to my tabPlace(w)
	if place is "" then
		my speak(my docTitle(w) & ": " & my statusText(statusLine))
	else
		my speak(my docTitle(w) & ", " & place & ": " & my statusText(statusLine))
	end if
end run
