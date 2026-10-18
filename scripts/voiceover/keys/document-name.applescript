-- Option+0: the document name and the status line.
on run
	set parts to my needDoc()
	if parts is missing value then return
	set {w, area, statusLine, trans} to parts
	my speak(my docTitle(w) & ": " & my statusText(statusLine))
end run
