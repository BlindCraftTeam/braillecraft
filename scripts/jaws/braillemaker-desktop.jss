;#pragma usePoFile 0
; script file 	for BrailleMaker for Windows, supporting E-Brailler Cosmo Input

Include "hjglobal.jsh"
Include "hjconst.jsh"
Include "uia.jsh"
Include "default.jsm"
Include "braillemaker-desktop.jsh"
Include "braillemaker-desktop.jsm"
Include "common.jsm"
Include "hjHelp.jsh"
Import "QuickSet.jsd"

globals
	string sWindowName,
	string sOldWindowName,
	string sOldOldWindowName,
	string stWindowName,
	string stOldWindowName,
	string strWindowName,
	string strOldWindowName,
	int nFocusChange,
	int iLineChange,
	int iFirstWord,
	int MotionFlag,
	int SaySkipFlag,
int nMotionDone,
	int LineNum,
	int OldLineNum,
	string sPageNum,
	string sOldPageNum,
	int InTable,
	int OldInTable,
int PreTable,
	string sRowWord,
	string sRowNum,
	string sOldRowNum,
	string sRowTotal,
	string sColWord,
	string sColNum,
	string sOldColNum,
	string sColTotal,
	int BrailleModeSaved,
	int BrailleAutoRouteSaved,
	int LeftMarginSaved,
	int BrailleAutopanSaved,
	int nTypingEchoSaved,
	int nFlashMessagesSaved,
	int nG2TranslationSaved,
	int nG2ExpandCurrentWordSaved,
	int nContractedInputSaved,
	int nEightDotSaved,
	int nBrailleProfileSaved,
	int nBrailleTranslatorModeSaved,
	int nEchoChange,
	int nBrailleDoc,
	int nBrailleEdit,
	int nOldBrailleEdit,
	int nBrailleStart,
	int nBrailleCharSpokenCol,
	int nInBrailleWindow,
	int nStartCol,
	int nStartRow,
	string sStyleName,
	string sOldStyleName,
	int AppHasRunBefore,
	int nCustomWindow,
	int giBrailleLineSpeakingLevel,
	int giBrailleDocInputEcho,
	int giOldBrailleDocInputEcho,
	int giBrailleDocInputEcho,
	int gbAnnounceBrailleCharsAsLocalBrailleSigns,
	int gbStyleChangeVerbosity,
	int gbSkipVerbosity,
		int gbOldShowRequestedStatusInfoOnBrailleDisplay,
	int gbShowRequestedStatusInfoOnBrailleDisplay,
	int gbSpeakPrintLinesByWord,
int gbUseStatusLineForUnicodeValues,
	int nJFWVersion,
	int nBrailleTime,
	int iSuppressSameSpot,
	int nWindowTime,
	int nSelectCharacter,
	int nSelectWord,
int nSelectLine,
int nHighlight,
	int nOldHighlight,
	int nHighlightCheck,
	int nNewDoc,
	int nProfileIndex,
	int iSuppressMulti,
	int nCatchingUp,
	handle SpellCheckWindow,
	int nUsingUIA,
	int nKeysPos,
	int nStartingApp,
	int nToEnd,
	int nOldToEnd,
	int nLineCount,
	int nOldLineCount,
	int nNextLineCount,
	int nOldNextLineCount,
	int nPriorLineCount,
	int nOldPriorLineCount,
	string sPriorChar,
	string sOldPriorChar,
	string sToEnd,
	string sOldToEnd,
	int nFromStart,
	int nOldFromStart,
	string sCurrentWord,
	string sOldCurrentWord,
	string sNextLine,
	string sOldNextLine,
	string sPriorLine,
	string sOldPriorLine,
	string sWindowName,
	string sOldWindowName,
	int nNewPoint,
	int nKeyPressTime,
	int nOldKeyPressTime,
	string sKeyName,
	int nCosmoStick,
	int nOldCosmo,
	int nCosmoStick,
	int nTransLineCheck,
	int nLastMovementTick,
	int nLastStableTick,
	int nOldLastStableTick,
	int nScrollDown,
	int nScrollUp,
	int nBrailleProfileStateSaved,
	int nWordWrapSaved,
	int nSuppressCapsSaved,
	int iTableEntry,
	int iWarning,
	int iViewToggle,
	int iToggleTime,
	int iSpellCheck,
	int iScrolled,
	string sLang,
	string sLine,
	string sOldLine,
	int nChange,
	int NoEcho,
	handle hndStatusLine,
	handle hndTransLine,
	string sTransLineFrame,
	string sStatusLine,
	int nCosmoSave,
	int nSpecialSayAllInProgress,
	int nStableCount,
	int nLineCount,
	handle hndTransLine,
	int gbSpeakCompletedBrailleWords,
	string sLastBrailleInput,
	string sTransLine,
	string sOldTransLine,
	string sOldOldTransLine


int function IsSpeechOnDemandActive ()
return (GetJCFOption (OPT_SPEECH_MODE) >= 1 /*|| GetJCFOption (OPT_LESS_SPEECH) == 1*/)
endFunction

void function AppSpeechOff ()
	if GetJCFOption (OPT_SPEECH_MODE) then return endIf
	SpeechOff ()
endFunction


void function AppSpeechOn ()
	if GetJCFOption (OPT_SPEECH_MODE) then return endIf		
	SpeechOn ()
endFunction

		
Void Function PauseForMovement ()

	Delay(1)

endFunction


Void Function AutoStartEvent ()

	AppSpeechOn ()
	if AppHasRunBefore == FALSE then
		LoadNonJCFOptions ()
		SayFormattedMessage (OT_APP_START, msgAppStart, msgAppStart)
		let AppHasRunBefore = TRUE
		;let giBrailleDocInputEcho = 0
	endIf
	if GetJAWSMajorVersionNumber () >= 2022
	&& BrailleInUse () then
		PerformScript BrailleSelectAProfile()
		let nBrailleProfileStateSaved = BrailleGetProfileState ()
		;SayString ("nBrailleProfileStateSaved = "+IntToString (nBrailleProfileStateSaved))
	endIf
	let nG2TranslationSaved = GetJCFOption(OPT_BRL_G2TRANSLATION)
	let nContractedInputSaved = GetJCFOption(OPT_BRL_CONTRACTED_INPUT)
	let nTypingEchoSaved = GetJCFOption(OPT_TYPING_ECHO)
	;SayString ("nTypingEchoSaved is "+IntToString (nTypingEchoSaved))
	let nEightDotSaved = GetJCFOption (OPTBRL_EIGHT_DOT)
	let nG2ExpandCurrentWordSaved = GetJCFOption(OPT_BRL_G2EXPAND_CURRENT_WORD)
	let nWordWrapSaved = GetJCFOption(OPT_BRL_WORDWRAP)
		let nSuppressCapsSaved = GetJCFOption(OPT_BRL_G2SUPPRESS_CAPS)
	if GetJAWSMajorVersionNumber () >= 1700 then
		let nProfileIndex = GetActiveProfileIndex ()
	endIf
	SetVoicePunctuation (VCTX_KEYBOARD, 3)
	let MotionFlag = 0
	let SaySkipFlag = 0
	let iSuppressSameSpot = 0
	let nStartingApp = 1
	let NoEcho = 0
	let iViewToggle = 0
	if GetWindowName (GetRealWindow (getCurrentWindow ())) != "Welcome to BrailleMaker" then
		SendMessage (GetAppMainWindow (GetCurrentWindow ()), 0X0112, /*WM_SYSCOMMAND,*/ 0XF030, 0) /* maximize the window*/
		SendMessage (GetFirstChild (GetFirstChild (GetAppMainWindow (GetCurrentWindow ()))), 0X0112, /*WM_SYSCOMMAND,*/ 0XF030, 0) /* maximize the window*/
	;else SayString ("curly")
	endIf
		let nCustomWindow = 0
		if nBrailleEdit then
			let nBrailleStart = 1
		endIf
		HandleCustomWindows (GetCurrentWindow ())
		
	SaveCursor ()
	RouteInvisibleToPc ()
	InvisibleCursor ()
	SetRestriction (RestrictRealWindow)
	JawsPageDown ()
	if GetControlID (GetCurrentWindow ()) != Editor_Control then
		PriorLine ()
	EndIf
	RestoreCursor ()
	if GetJAWSMajorVersionNumber () >= 451 then
		SetBrailleRestriction (RestrictWindow)
	endIf

EndFunction


Void Function AutoFinishEvent ()

	if nJFWVersion >= 1700000 then
		SetActiveVoiceProfileByIndex (nProfileIndex, FALSE)
	endIf
	if GetJAWSMajorVersionNumber () >= 2022 then
			BrailleSetProfileState (nBrailleProfileStateSaved)
		else
			SetJCFOption (OPT_BRL_G2TRANSLATION, nG2TranslationSaved)
			SetJCFOption (OPT_BRL_CONTRACTED_INPUT, nContractedInputSaved)
		endIf
		SetJCFOption (OPT_BRL_G2EXPAND_CURRENT_WORD, nG2ExpandCurrentWordSaved)
			SetJCFOption (OPTBRL_EIGHT_DOT, nEightDotSaved)
		SetJCFOption (OPT_BRL_WORDWRAP, nWordWrapSaved)
		if giBrailleDocInputEcho then
			SetDefaultJcfOption(OPT_TYPING_ECHO, nTypingEchoSaved) ; restoring  saved echo setting
			SetJcfOption(OPT_TYPING_ECHO, nTypingEchoSaved) ; restoring  saved echo setting
		endIf

	EndFunction


Script ScriptFileName ()

	ScriptAndAppNames (msgAppName)
	if UserBufferIsActive() then
		UserBufferDeactivate()
	EndIf
	SayFormattedMessage (OT_USER_BUFFER, msgAppTrouble, msgAppTrouble)

EndScript


Script HotKeyHelp ()

	if UserBufferIsActive() then
		UserBufferDeactivate()
	EndIf
	SayFormattedMessage (OT_USER_BUFFER, msgJHotKeyHelp_L,msgJHotKeyHelp_S)

EndScript


Void Function SayBrailleChar (string VCTX, string CurrentChar)

var
	int iVal

	let iVal = GetCharacterValue (CurrentChar)
	if gbAnnounceBrailleCharsAsLocalBrailleSigns == 0 then
		;SayUsingVoice (CurrentChar, VCTX, FALSE)
		SayUsingVoice (VCTX, CurrentChar, OT_CHAR, FALSE)
		return
	endIf
	if iVal == 30 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars123, ";", 5), OT_CHAR, FALSE)
		return
	elIf iVal < 48 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars32, ";", iVal-31), OT_CHAR, FALSE)
		return
	elif iVal <= 64 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars48, ";", iVal-47), OT_CHAR, FALSE)
		return
	elif iVal < 107 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars91, ";", iVal-90), OT_CHAR, FALSE)
		return
	elif iVal < 123 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars107, ";", iVal-106), OT_CHAR, FALSE)
		return
	elif iVal < 128 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars123, ";", iVal-122), OT_CHAR, FALSE)
		return
	elif iVal < 1088 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars1072, ";", iVal-1071), OT_CHAR, FALSE)
		return
	elif iVal < 1106 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars1088, ";", iVal-1087), OT_CHAR, FALSE)
		return
	elif iVal < 10256 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars10240, ";", iVal-10239), OT_CHAR, FALSE)
		return
	elif iVal < 10272 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars10256, ";", iVal-10255), OT_CHAR, FALSE)
		return
	elif iVal < 10288 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars10272, ";", iVal-10271), OT_CHAR, FALSE)
		return
	elif iVal < 10304 then
		SayUsingVoice (VCTX, StringSegment (csBrailleChars10288, ";", iVal-10287), OT_CHAR, FALSE)
		return
	endIf

EndFunction


Function FocusPointMovedEvent (int nX, int nY, int nOldX, int nOldY, int nUnit, int nDir, int nTimeElapsed)

var

	int x,
	string sOldLastBrailleInput

	;SayString ("imagine")
	;SayString ("nUnit is "+IntToString (nUnit))
	;SayString ("nTimeElapsed is "+IntToString (nTimeElapsed))
	;SayString ("ndir is "+IntToString (nDir))
	let sOldOldWindowName = sOldWindowName
	let sOldWindowName = sWindowName
	;let sWindowName = GetWindowName (GetRealWindow (GetCurrentWindow ()))
	
	if not IsDocumentWindow (GetCurrentWindow ())
	&& GetWindowClass (GetCurrentWindow ()) != wcCodesEdit
	|| not IsPCCursor () then
		let MotionFlag = 0
		return
	endIf
	let nLastMovementTick = GetTickCount ()
	if giBrailleDocInputEcho >= 3 
	&& nBrailleEdit then
		let sOldOldTransLine = sOldTransLine
		let sOldTransLine = sTransLine
		PauseForMovement ()
		let sTransLine = GetWindowText (hndTransLine, FALsE)
		;qqq
	endIf

	if PreTable then
		let iLineChange = 1
		let PreTable = PreTable-1
	endIf
	if nSpecialSayAllInProgress then
		while GetTickCount () == nLastMovementTick
			Delay (1)
		endWhile
		Delay (1000/GetVoiceRate (VCTX_PCCURSOR, FALSE))
			SayUsingVoice (VCTX_MESSAGE, msgLineDown, OT_SAYALL)
			;PlaySound (FindJAWSSoundFile ("doodoop1.wav"))
			NextLine ()
		SayLineInsteadOfSayAll ()
 		if BrailleInUse () then
			BrailleLine ()
		endIf
		;xxx
		Scroll ()

		let MotionFlag = 1
		endIf
	if MotionFlag == 1
		&& not iLineChange
		&& not InTable
		&& not PreTable then
		let iFirstWord = 0
		let MotionFlag = 0
		PauseForMovement ()
		return
	endIf
	if InTable
	|| PreTable then
		SaveOldValues ()
		GetNewValues ()
		Pause ()
		return
	endIf
	if not iFirstWord
	&& not (nBrailleEdit 
	&& (giBrailleDocInputEcho >= 2
	&& not StringContains (GetFromStartOfLine (), " "))) then
		let iFirstWord = 1
		SaveOldValues ()
		GetNewValues ()
		if not nBrailleEdit then
			let MotionFlag = 0
			return
		endIf
	endIf
	if not MotionFlag
	|| iLineChange then
		SaveOldValues ()
	endIf
	;SayInteger (giBrailleDocInputEcho)
	if nBrailleEdit
	&& giBrailleDocInputEcho >= 2 then
	;|| giBrailleDocInputEcho == 4) then
; in braille doc window, speak last character as braille cell,
		if not MotionFlag
		&& not iViewToggle
		&& nTimeElapsed < 400
		&& nY-nOldY < 50
		&& nOldY-nY < 50 then
			PauseForMovement ()
			;SayBrailleChar (VCTX_KEYBOARD, GetPriorCharacter ())
			let sOldLastBrailleInput = sLastBrailleInput
			let sLastBrailleInput = GetLastBrailleInput ()
			if giBrailleDocInputEcho != 3 then
				SayBrailleChar (VCTX_KEYBOARD, sLastBrailleInput)
			endIf
			if (GetCharacterValue (sLastBrailleInput) == 32
			|| GetCharacterValue (sLastBrailleInput) == 9)
			&& GetCharacterValue (sOldLastBrailleInput) != 32
						&& GetCharacterValue (sOldLastBrailleInput) != 9
&& giBrailleDocInputEcho >= 3 then
				;qqq
				SpeakCompletedBrailleWord ()
			endIf
			
			let MotionFlag = 0
			let NoEcho = 0
;			PauseForMovement ()
			if GetWindowClass (GetCurrentWindow ()) == wcEditor then
				let nFocusChange = 0
			endIf
		; below avoid saying "new line" or "space" when starting a Find or Replace command
			if nY > nOldY
			|| nx < nOldX
			&& not nFocusChange
			&& not MotionFlag then
				if LineNum != -1 then
					SayMessage (OT_KEYBOARD, msgNewLine, msgNewLine)
				endIf
				let nFocusChange = 0
				let MotionFlag = 1
			endIf
			if GetCharacterValue (GetLastBrailleInput ()) == 0 then
				if GetFromStartOfLine () == "" then
				else SayMessage (OT_KEYBOARD, msgLocalSpace, msgLocalSpace)
			endIf
			endIf
		endIf
	endIf
	if /*!MotionFlag
	||*/ iLineChange
	|| InTable
	|| PreTable then
		GetNewValues ()
	endIf
	let MotionFlag = 0

EndFunction


Void Function FocusPointMovedEventCosmo (int nX, int nY, int nOldX, int nOldY, int nUnit, int nDir, int nElapsedTime)

var
	int x

	;SayString ("puppy")
	Say ("", OT_STRING, FALSE)
	
	if nSpecialSayAllInProgress then
	/*let nScrollDown = 0
		SaveCursor ()
		RouteInvisibleToPC ()
		InvisibleCursor ()
		NextLine ()
		if not IsDocumentWindow (GetWindowAtPoint (GetCursorCol (), GetCursorRow ())) then
			;let nScrollDown = 1
		endIf
		PCCursor ()
		RestoreCursor ()
		;SayString ("now "+GetLine ())
	;endIf
		/*if nScrollDown then
		SayString ("scrolling down")
				Scroll ()
				let nScrollDown = 0
				return
			endIf*/
			if nSpecialSayAllInProgress then
SayString ("goat's milk")
			NextLine ()
			PerformScript SpecialSayAll ()
		let MotionFlag = 1
		return
	endIf
	endIf
	let nOldKeyPressTime = nKeyPressTime
	let nKeyPressTime = GetLastKeyPressTime ()
	
	let sOldToEnd = sToEnd
	let sToEnd = GetToEndOfLine ()
	let nOldToEnd = nToEnd
	let nToEnd = StringLength (GetToEndOfLine ())
	let nOldFromStart = nFromStart
	let nFromStart = StringLength (GetFromStartOfLine ())
	let nOldLineCount = nLineCount
	let nLineCount = StringLength (GetLine ())
	let sOldPriorChar = sPriorChar
	let sPriorChar = GetPriorCharacter ()
	let nOldNextLineCount = nNextLineCount
	let sOldNextLine = sNextLine
	let sOldCurrentWord = sCurrentWord
	
	let sCurrentWord = GetWord ()
	SaveCursor ()
	RouteInvisibleToPC ()
	InvisibleCursor ()
	NextLine ()
	let sNextLine = GetLine ()
	let nNextLineCount = StringLength (GetLine ())
	PCCursor ()
	RestoreCursor ()
	let nOldPriorLineCount = nPriorLineCount
	;let sOldPriorLine = sPriorLine
	SaveCursor ()
	RouteInvisibleToPC ()
	InvisibleCursor ()
	PriorLine ()
	let sPriorLine = GetLine ()
	let nPriorLineCount = StringLength (GetLine ())
	PCCursor ()
	RestoreCursor ()
	
	if stOldWindowName == stWindowName
	&& nFocusChange < 0 then
		if nLastMovementTick -nLastStableTick > nLastStableTick -nOldLastStableTick then
			let nFocusChange = 0
		endIf
	endIf
	if nSpecialSayAllInProgress
	|| (nKeyPressTime == nOldKeyPressTime
	|| nKeyPressTime == 0)
	&& not nFocusChange then
	;&& sKeyName != GetCurrentScriptKeyName ()
	;|| nOldKeyPressTime == 0
	;&& nLastStableTick -nOldLastStableTick > 20000 then
		NewFocusPoint (nX, nY, nOldX, nOldY)
	if not iFirstWord
	&& not StringContains (GetFromStartOfLine (), " ") then
		let iFirstWord = 1
		;SaveOldValues ()
		;PauseForMovement ()
		;GetNewValues ()
	endIf
		if MotionFlag == 1 then
			let MotionFlag = 0
			return
		endIf
	endIf
	if nFocusChange then
		let MotionFlag = 1
		;PauseForMovement ()
	endIf
	if nBrailleEdit
	&& giBrailleDocInputEcho then
; in braille doc window, speak last character as braille cell,th
		if not MotionFlag
		&& not iViewToggle
		&& nElapsedTime < 800
		&& nY-nOldY < 50
		&& nOldY-nY < 50 then
			;SayBrailleChar (VCTX_KEYBOARD, GetPriorCharacter ())
			SayBrailleChar (VCTX_KEYBOARD, GetLastBrailleInput ())
;			PauseForMovement ()
			if GetWindowClass (GetCurrentWindow ()) == wcEditor then
				let nFocusChange = 0
			endIf
		; below avoid saying "new line" or "space" when starting a Find or Replace command
			if nY != nOldY
			|| nx < nOldX
			&& nKeyPressTime != 0
			&& not nFocusChange then
				if LineNum != -1 then
					SayMessage (OT_KEYBOARD, msgNewLine, msgNewLine)
				endIf
				let nFocusChange = 0
			endIf
			if nKeyPressTime != 0
			&& GetCharacterValue (GetLastBrailleInput ()) == 0 then
				SayMessage (OT_KEYBOARD, msgLocalSpace, msgLocalSpace)
			endIf
		endIf
	endIf
	if not MotionFlag
	&& iLineChange then
		SaveOldValues ()
		PauseForMovement ()
		GetNewValues ()
	endIf

EndFunction


Void Function NewFocusPoint (int nX, int nY, int nOldX, int nOldY)

var
	int nBackspace

	;SayString ("spinach")
	if iViewToggle then
		return
	endIf
	if iScrolled then
		let iScrolled = iScrolled-1
		;return
	endIf
	if nHighlight then
		;SayString ("elevator")
		NewFocusPointHighlight (nX, nY, nOldX, nOldY)
		;let nKeyPressTime = nOldKeyPressTime
		return
	endIf

	if nSpecialSayAllInProgress () then
	SpeakLineOfText ()
endIf
	;AppSpeechOn ()
	let nBackspace = 0
	let nNewPoint = 1

	if StringIsBlank (GetLine ()) then
		AppSpeechOn ()
		SpeakLineOfText ()
		AppSpeechOff ()
		return
	endIf
		if nY > nOldY
		&& nOldLineCount > 0
		&& nOldToEnd == 0
	 	&& nFromStart == 0 then
			;SayString ("cabbage1")
			PerformScript SayCharacter ()
			return
		endIf
		if nY < nOldY
		&& nOldLineCount > 0
		&& nOldFromStart == 0
		&& nToEnd == 0 then
			;SayString ("cabbage2")
			PerformScript SayCharacter ()
			return
		endIf
		if nY < nOldY
		&& StringLength (GetLine ()) != nOldPriorLineCount then
			SayDocChar (VCTX_KEYBOARD, sOldPriorChar)
			let nBackspace = 1
			return
		endIf
		if nY > nOldY
		&& nOldToEnd == 0
		;&& not StringIsBlank (sOldCurrentWord)
		&& not StringCompare (sOldCurrentWord, StringChopRight (sCurrentWord, 1), TRUE) then
				;SayString ("balloon")
				SayDocPriorChar ()
				if LineNum != -1 then
					Say (msgNewLine, OT_KEYBOARD, FALSE)
					return
				endIf
					endIf
		if nY > nOldY
		&& nLineCount > nOldNextLineCount then
			SayDocPriorChar ()
			return
		endIf
	if nY != nOldY then
		SpeakLineOfText ()
		AppSpeechOff ()
		return
	endIf
			;elif nY < nOldY
			;&& nToEnd == 0
			;&& GetPriorCharacter () == " " then
		;&& StringStartsWith (sOldCurrentWord, GetPriorWord ()) then
			;SayString ("pajamas")
			;Say (msgLocalSpace, OT_KEYBOARD, FALSE)
			;SayString ("milkshake")
			;SpeakLineOfText ()
			;return
		;endIf
			;if GetWindowClass (GetCurrentWindow ()) == wcEditor then
				;let nFocusChange = 0
			;endIf
		; below avoid saying "new line" or "space" when starting a Find or Replace command
			if nY > nOldY
			|| nx < nOldX
			&& not nFocusChange then
				if LineNum != -1 then
					;SayMessage (OT_KEYBOARD, msgNewLine+" 2", msgNewLine+" 2")
				endIf
				let nFocusChange = 0
				endIf
			;endIf
		;endIf

	If nY == nOldY then
		if nX < nOldX
		&& StringStartsWith (sToEnd, sOldToEnd, TRUE)
		&& nPriorLineCount < nOldPriorLineCount then
			let nBackspace = 1
			SayDocChar (VCTX_KEYBOARD, sPriorChar)
			return
			;else SayTheCharacter ()
		endIf
	;endIf
	if nLineCount == nOldLineCount
	&& not nBackspace then
		if StringIsBlank (GetToEndOfLine ())
		&& StringIsBlank (GetFont ()) then
			Say (msgLocalSpace, OT_TEXT, FALSE)
		else ;SayString ("dulcimer")
			SayDocChar (VCTX_PCCURSOR, GetCharacter ())
		return
		endIf
	else
		If nLineCount > nOldLineCount then
			;SayString ("piano")
			SayDocChar (VCTX_KEYBOARD, sPriorChar)
			endIf
	if nLineCount < nOldLineCount then
		if nToEnd == nOldToEnd then
			;SayString ("back")
			SayDocChar (VCTX_KEYBOARD, sOldPriorChar)
		endIf
			elIf nFromStart == 0
			&& nPriorLineCount == nOldPriorLineCount
			&& GetPriorCharacter () == " " then
				;SayString ("moving down")
				SayDocChar (VCTX_KEYBOARD, sPriorChar)
			endIf
		endIf
			else SayDocChar (VCTX_KEYBOARD, sOldPriorChar)
	endIf
	
EndFunction


Void Function NewFocusPointHighlight (int nX, int nY, int nOldX, int nOldY)

var
	int nBackspace

	let nBackspace = 0
	let nNewPoint = 1

	;SayString ("purple")
	if nY > nOldY then
		;SayString ("yellow")
		if nOldToEnd == 0
		&& nFromStart == 0 then
			;SayString ("cabbage")
			AppSpeechOff ()
			PriorCharacterSelectIfOn ()
			AppSpeechOn ()
			SelectNextCharacter ()
			return
		else ;SayString ("barbi")
			SaveCursor ()
			AppSpeechOff ()
			PerformScript SelectPriorLine ()
			AppSpeechOn ()
			SpeakLineOfText ()
			AppSpeechOff ()
			PerformScript SelectNextLine ()
			AppSpeechOn ()
			RouteInvisibleToPC ()
			InvisibleCursor ()
			PriorLine ()
			If GetCharacterAttributes ()&ATTRIB_HIGHLIGHT then
				Say (msgSelected, OT_TEXT, FALSE)
			else Say (msgUnselected, OT_TEXT, FALSE)
			endIf
			RestoreCursor ()
			PCCursor ()
			AppSpeechOff ()
			PauseForMovement ()
			AppSpeechOn ()
			let nKeyPressTime = 0
			return
		endIf
	endIf
	if nY < nOldY then
		if nOldFromStart == 0
		&& nToEnd == 0 then
			;SayString ("cabbage")
			AppSpeechOff ()
			NextCharacterSelectIfOn ()
			AppSpeechOn ()
			PerformScript SelectPriorCharacter ()
			return
		else ;SayString ("barbi")
			SaveCursor ()
			AppSpeechOff ()
			PerformScript SelectNextLine ()
			AppSpeechOn ()
			SpeakLineOfText ()
			AppSpeechOff ()
			PerformScript SelectPriorLine ()
			AppSpeechOn ()
			RouteInvisibleToPC ()
			InvisibleCursor ()
			NextLine ()
			If GetCharacterAttributes ()&ATTRIB_HIGHLIGHT then
				Say (msgSelected, OT_TEXT, FALSE)
			else Say (msgUnselected, OT_TEXT, FALSE)
			endIf
			RestoreCursor ()
PCCursor ()
AppSpeechOff ()
PauseForMovement ()
AppSpeechOn ()
let nKeyPressTime = 0
			return
		endIf
	endIf
	if nY == nOldY then
		;SayString ("llama")
		if nX < nOldX then
			if nBrailleEdit then
				SayBrailleChar (VCTX_PCCURSOR, GetCharacter ())
			else SayCharacter ()
			endIf
			If GetCharacterAttributes ()&ATTRIB_HIGHLIGHT then
				Say (msgSelected, OT_TEXT, FALSE)
			else Say (msgUnselected, OT_TEXT, FALSE)
			endIf
			AppSpeechOff ()
			PauseForMovement ()
			AppSpeechOn ()
			return
		endIf
		if nX > nOldX then
			if nBrailleEdit then
				SayBrailleChar (VCTX_PCCURSOR, GetPriorCharacter ())
			endIf
		SaveCursor ()
			RouteInvisibleToPC ()
			InvisibleCursor ()
			PriorCharacter ()
			if not nBrailleEdit then
				SayCharacter ()
			endIf
			If GetCharacterAttributes ()&ATTRIB_HIGHLIGHT then
				Say (msgSelected, OT_TEXT, FALSE)
			else Say (msgUnselected, OT_TEXT, FALSE)
			endIf
			PCCursor ()
			RestoreCursor ()
			AppSpeechOff ()
			PauseForMovement ()
			AppSpeechOn ()
			return
		endIf
	endIf

EndFunction


Void Function FocusChangedEvent (handle hWnd, handle hOldWnd)

	FocusChangedEvent (hWnd, hOldWnd)
	
let nFocusChange = 1
	let sOldWindowName = GetWindowName (GetRealWindow (hOldWnd))
	let sWindowName = GetWindowName (GetRealWindow (hWnd))
	if not DialogActive ()
	|| GetWindowTypeCode (hWnd) == WT_BUTTON
	&& GetWindowName (hWnd) == wnOK then
		HandleCustomWindows (hWnd)
		else FocusChangedEvent (hWnd, hOldWnd)
	endIf
	let nFocusChange = 1
 
endFunction


Int Function MenuModeEvent (handle WinHandle, int mode)

	let nFocusChange = 1
	MenuModeEvent (WinHandle, mode)
	PauseForMovement ()
	HandleCustomWindows (GetCurrentWindow ())

	endFunction
	
	
void Function SayCharacterValue ()

var
	int iUnicode,
	string sUnicodeString,
	string sUnicodeMod,
	string UniMessage

	if not IsPCCursor ()
	|| not IsDocumentWindow (GetCurrentWindow ()) then
		SayCharacterValue ()
		return
	endIf
	let MotionFlag = 1
	if not DecipherItemofText () then
		return
	endIf
		if gbUseStatusLineForUnicodeValues then
			let sUnicodeString = GetUnicodeValue ()
			if StringStartsWith (sUnicodeString, msgUnicodePrefix) then
			Say (sUnicodeString+msgHex, OT_TEXT, 0)
		else Say (msgCode, OT_TEXT, 0)
			SpellString (sUnicodeString)
		endIf
	else SayCharacterValue ()
	endIf

EndFunction


string Function GetUnicodeValue ()

var
	int iUnicode,
	int iStart,
	int iLength,
	string sUnicodeMod,
	string sUnicodeString,
	string sUnicodeValue,
	int i

	if not IsDocumentWindow (GetCurrentWindow ()) then
		return (GetCharacterValue (GetCharacter ()))
	endIf
	if DecipherItemOfText () == 0 then
		return
	endIf
		if !gbUseStatusLineForUnicodeValues
		/*|| GetCharacter () == " "*/ then
		let sUnicodeValue = IntToString (GetCharacterValue (GetCharacter ()))
		return sUnicodeValue
	endIf
	PauseForMovement ()
	let sStatusLine = GetStatusLineText ()
	let iUnicode = StringContains (sStatusLine, scUnicode)
	if iUnicode > 0 then
		let iStart = iUnicode+2
			;let iLength = min (StringContains (StringChopLeft (sStatusLine, iStart), "}"), 4)
			let iLength = 4
		if iLength > 0 then
			let sUnicodeString = Substring (sStatusLine, iStart, iLength)
			let sUnicodeValue = StringReplaceChars (StringTrimLeadingBlanks (StringReplaceChars (sUnicodeString, "0", " ")), " ", "0")
			return msgUnicodePrefix+sUnicodeValue
		endIf
	else let iStart = StringContains (sStatusLine, "[")
		if iStart > 0 then
			let iLength = StringContains (StringChopLeft (sStatusLine, iStart+1), "]")
			if iLength > 0 then
				let sUnicodeValue = Substring (sStatusLine, iStart+1, iLength)
				return sUnicodeValue
			endIf
							endIf
						endIf

EndFunction


Void Function SayCharacterPhonetic ()

	if not IsPCCursor ()
	|| not IsDocumentWindow (GetCurrentWindow ())
		SayCharacterPhonetic ()
		return
	endIf
	let MotionFlag = 1
	if not DecipherItemOfText () then
		return
	else SayCharacterPhonetic ()
	endIf

EndFunction


string Function GetColumnNum ()

var
	int iColumn,
	string sColumnMod

	/*SaveCursor ()
	InvisibleCursor ()
	RouteInvisibleToPc ()
	JawsPageDown ()
	JawsHome ()
	if GetControlID (GetCurrentWindow ()) != Editor_Control then
		PriorLine ()
	EndIf*/
	;let sStatusLine = GetStatusLineText ()
	let iColumn = StringContains (sStatusLine, scColumn)
	if iColumn == 0 then
		;RestoreCursor ()
		return -1
	else
		let sColumnMod = SubString (sStatusLine, iColumn + StringLength (scColumn), 2)
		;RestoreCursor () ;SayInteger (iColMod)
		return sColumnMod
	EndIf

EndFunction


Int Function GetLineNum ()

var
	int iLine,
	int iLineMod,
	int iscLineLength = StringLength (scLine),
	int iColumn

	/*SaveCursor ()
	InvisibleCursor ()
	RouteInvisibleToPc ()
	JawsPageDown ()
	JawsHome ()
	if GetControlID (GetCurrentWindow ()) != Editor_Control then
		PriorLine ()
	EndIf*/
	let sStatusLine = GetStatusLineText ()
	let iLine = StringContains (sStatusLine, scLine)
	if iLine == 0 then
		RestoreCursor ()
		return -1
	else
		let iColumn = StringContains(sStatusLine, scColumn)
		let iLineMod = StringToInt (SubString (sStatusLine, iLine + iscLineLength, iColumn-iLine-iscLineLength))
		RestoreCursor ()
		return iLineMod
	EndIf

EndFunction


String Function GetPageNum ()

var
	int iPage,
	int iScPageLength = StringLength (scPage),
	int iLine,
	string sPageMod

	if not IsDocumentWindow (GetCurrentWindow ()) then
		return ""
	endIf
	;GetStatusLine ()
	let sStatusLine = GetStatusLineText ()
	let iPage = StringContains (sStatusLine, scPage)
	if iPage == 0 then
		;RestoreCursor ()
		return -1
	else
		let iLine = StringContains (sStatusLine, scLine)
		let sPageMod = SubString (sStatusLine, iPage+iScPageLength, iLine-iPage-iScPageLength)
		;RestoreCursor ()
		;SayString ("page "+sPageMod)
		return sPageMod
	EndIf

EndFunction


string Function GetStyleName ()

var
	int iLabelStyle,
	int iLabelRow,
	int iLabelCol,
	string sStyle,
	string sStyleChunk,
	string s,
	int iStyleChunk,
	int i,
	int iEnteringTable,
	int iNewRow,
	int iNewCol

	let OldInTable = InTable
	let sOldRowNum = sRowNum
	let sOldColNum = sColNum
	if not IsDocumentWindow (GetCurrentWindow ()) then
		return
	endIf
	;RouteInvisibleToPC ()
	SaveCursor ()
	/*RouteInvisibleToPc ()
	InvisibleCursor ()
	SetRestriction (RestrictRealWindow)
	JawsPageDown ()
	if GetControlID (GetCurrentWindow ()) != Editor_Control then
		PriorLine ()
	EndIf*/
	MoveToWindow (hndStatusLine)
	JawsHome ()
	let iLabelRow = StringContains (GetLine (), scRow)
		let iLabelCol = StringContains (GetLine (), scCol)
		if iLabelRow > 0
		&& iLabelCol > 0
		|| StringContains (GetLine (), msgMatrix) then
			let InTable = 1
		else let InTable = 0
			let iWarning = 0
		if OldInTable then
			SayUsingVoice (VCTX_MESSAGE, msgExitingTable, OT_STATUS)
			let iWarning = 0
			let OldInTable = 0
		endIf
	endIf
		let iLabelStyle = StringLength (scCurrentStyle)
		let sStyleChunk = GetChunk ()
		let iStyleChunk = StringLength (sStyleChunk)
		if iLabelRow > 0
		&& iLabelRow < iStyleChunk then
			let s= StringChopRight (sStyleChunk, iStyleChunk-iLabelRow+2)
		else let s = sStyleChunk
		endIf
		if StringStartsWith (s, scCurrentStyle) then
			let sStyle = StringChopLeft (s, iLabelStyle)
		else let sStyle = s
		endIf
		if InTable then
			if not OldInTable then
				let iEnteringTable = 1
				SayUsingVoice (VCTX_MESSAGE, msgEnteringTable, OT_STATUS)
			endIf
		let i = 0
		while i < iLabelRow
			let i = i+1
			NextCharacter ()
		endWhile
			NextWord ()
			let sRowWord = GetWord ()
			let sRowNum = StringSegment (sRowWord, "/", 1)
			if iEnteringTable then
				let sRowTotal = StringSegment (sRowWord, "/", 2)
			endIf
			NextWord ()
			NextWord ()
			let sColWord = GetWord ()
			let sColNum = StringSegment (sColWord, "/", 1)
			if iEnteringTable then
				let sColTotal = StringSegment (sColWord, "/", 2)
			endIf
	if InTable then
		;SpeakTableEntry ()
		;SayString ("sRowNum is "+sRowNum+"; sColNum is "+sColNum)
		if sRowNum != sOldRowNum
		|| iEnteringTable then
			let iNewRow = 1
			SayUsingVoice (VCTX_MESSAGE, msgRow+sRowNum+msgOf+sRowTotal, OT_STATUS)
		else let iNewRow = 0
		endIf
		if sColNum != sOldColNum
		|| iEnteringTable then
			let iNewCol = 1
			SayUsingVoice (VCTX_MESSAGE, msgCol+sColNum+msgOf+sColTotal, OT_STATUS)
		else let iNewCol = 0
		endIf
			if not iNewRow 
			&& not iNewCol then
				let iWarning = 1
			endIf
			if sRowNum == "1"
			&& sColNum == 1
			&& not iWarning then
				let iWarning = 1
				SayUsingVoice (VCTX_MESSAGE, msgNoShiftTab, OT_STATUS)
			elIf sRowNum == sRowTotal
			&& sColNum == sColTotal
			&& not iWarning then
			let iWarning = 1
				SayUsingVoice (VCTX_MESSAGE, msgNoTab, OT_STATUS)
			else let iWarning = 0
			endIf
			;SpeakTableEntry ()
			let iEnteringTable = 0
		endIf
	endIf
	PCCursor ()
	RestoreCursor ()
	;RoutePCToInvisible ()
		return sStyle

EndFunction


Script AltF3Press ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
	;if not StringContains (GetWindowName (GetRealWindow (GetCurrentWindow ())), scDocument) then
		SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
		return
	endIf
; to avoid giving incorrect message because of a timing lag,
; look at status line before sending Alt+F3 to the application
;SayInteger (GetLineNum ()) ;5/21/2013
	if GetLineNum() != -1 then
		SayMessage (OT_STATUS, msgCodesOn_l, msgCodesOn_s)
	else
		SayMessage (OT_STATUS, msgCodesOff_l, msgCodesOff_s)
	endIf
	TypeCurrentScriptKey ()
	let iViewToggle = 1

EndScript


Script F2Press ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
; previously just said "not in document window" with no change of setting,
; but now we change setting even though script can't announce it properly
		TypeCurrentScriptKey ()
		return
	endIf
	TypeCurrentScriptKey ()
	PauseForMovement ()
	if Get6KeyStatus () then
		SayMessage (OT_STATUS, msgPerkinsOn, msgPerkinsOn) 
	else
		SayMessage (OT_STATUS, msgPerkinsOff, msgPerkinsOff)
	endIf

EndScript


Int Function Get6KeyStatus ()

var
	string sLine,
	int nKeysPos

	SaveCursor ()
	InvisibleCursor ()
	RouteInvisibleToPc ()
	JawsPageDown ()
	if GetControlID (GetCurrentWindow ()) != Editor_Control then
		PriorLine ()
	EndIf	
	let sLine = GetLine ()
	let nKeysPos = StringContains (sLine, scKeys)
	if SubString (sLine, nKeysPos+5, 1) == "6" then
		return 1
	else
		return 0
	endIf
	RestoreCursor ()

EndFunction

Script ControlF8Press ()

var
	handle hndTransLine,
	int NoBackTrans

	if not StringContains (GetWindowName (GetRealWindow (GetCurrentWindow ())), scDocument) then
		SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
		return
	endIf
	let NoBackTrans = 0
	TypeCurrentScriptKey ()
	PauseForMovement ()
	let nTransLineCheck = CheckTransLine ()
	if nTransLineCheck == 1 then
		SayMessage (OT_STATUS, msgTransLineOn, msgTransLineOn)
	elif nTransLineCheck == 0 then
		SayMessage (OT_STATUS, msgTransLineOff, msgTransLineOff)
	elif nTransLineCheck == -1 then
		SayMessage (OT_SCREEN_MESSAGE, msgTransTableError, msgTransTableError)
		endIf

endScript


Void Function SayHighlightedText (handle hwnd, string buffer)

	let sWindowName = GetWindowName (GetRealWindow (GetCurrentWindow ()))
	if sWindowName == wnSpellCheck then
	SayMessage (OT_USER_REQUESTED_INFORMATION, buffer)
 	SayMessage (OT_STATUS, msgSpelled, msgSilent)
	SpellString (buffer)
	if GetObjectTypeCode () == WT_BUTTON then
		ReadContextLine ()
	endIf
elIf sWindowName == wnConfirmReplacement then
	if GetObjectTypeCode () == WT_BUTTON then
		ReadContextLine ()
	endIf
Else SayHighlightedText (hwnd, buffer)
endIf

EndFunction


Script SayLine ()

var
	int nTimesPressed

	let nTimesPressed = IsSameScript ()
	if nTimesPressed > 0 then
		SayLine()
		return
 	endIf
	if GetWindowClass (GetCurrentWindow ()) == wcMDIClient then
		SayUsingVoice (VCTX_PCCURSOR, msgLocalBlank, OT_TEXT)
		return
	endIf
	if GetWindowClass (GetCurrentWindow ()) == wcCodesEdit then
		Refresh (0)
		if nBrailleEdit then
			SpeakStringAsBrailleCells (VCTX_MENU_AND_DIALOG, GetWindowText (GetCurrentWindow (), 0))
			return
		else PerformScript SayLine ()
		endIf
	endIf
	if MenusActive ()
	;|| not IsDocumentWindow (GetCurrentWindow ())
	|| GetWindowClass (GetCurrentWindow ()) != wcEditor
	|| Not IsPcCursor () then
		PerformScript SayLine ()
		return
	endIf
	GetNewValues ()
	SpeakLineOfText ()

EndScript


Void Function SayLineUnit (int UnitMovement)

	if IsDocumentWindow(GetCurrentWindow ())
	&& IsPCCursor () then
		let iSuppressMulti = 1
		SpeakLineOfText ()
		let MotionFlag = 1
		let SaySkipFlag = 1
;let nStartCol = GetCursorCol ()
;let nStartRow = GetCursorRow ()
		let iSuppressMulti = 0
	else SayLineUnit (UnitMovement)
	endIf

EndFunction


Script SayNextLine ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
;	|| not IsPCCursor () then
		PerformScript SayNextLine ()
		return
	endIf
	let MotionFlag = 1
	;SayString ("houseboat")
	if nHighlight then
		PerformScript SelectNextLine()
		return
	endIf
	SaveOldValues ()
	let SaySkipFlag = 1
	let MotionFlag = 1
	NextLine ()
	let MotionFlag = 1
	PauseForMovement ()
	let MotionFlag = 1
	GetNewValues ()
	let MotionFlag = 1
	SpeakLineOfText ()
	let MotionFlag = 1

EndScript


Script BrailleNextLine ()

var
	int iActiveCursor

	if GetJCFOption (OPT_BRL_MOVE_ACTIVE_CURSOR) == 1 then
		SaveOldValues ()
		let SaySkipFlag = 1
		NextLine ()
		let MotionFlag = 1
		PauseForMovement ()
		let MotionFlag = 1
		GetNewValues ()
		let MotionFlag = 1
	else let iActiveCursor = GetActiveCursor ()
		BrailleCursor ()
		PerformScript BrailleNextLine()
		SetActiveCursor (iActiveCursor)
	endIf

EndScript


Script SayPriorLine ()	

	if MenusActive () then
		PriorLine ()
		return
	endIf
	if not IsDocumentWindow (GetCurrentWindow ()) then
;	|| Not IsPcCursor () then
		PauseForMovement ()
		PerformScript SayPriorLine ()
		return
	endIf
	let MotionFlag = 1
	if nHighlight then
		PerformScript SelectPriorLine()
		return
	endIf
	;let MotionFlag = 1
	SaveOldValues ()
	let SaySkipFlag = 1
	PriorLine ()
	PauseForMovement ()
	GetNewValues ()
	let MotionFlag = 1
	SpeakLineOfText ()

EndScript


Script BraillePriorLine ()

var
	int iActiveCursor

	if GetJCFOption (OPT_BRL_MOVE_ACTIVE_CURSOR) == 1 then
		SaveOldValues ()
		let SaySkipFlag = 1
		PriorLine ()
		let MotionFlag = 1
		PauseForMovement ()
		let MotionFlag = 1
		GetNewValues ()
		let MotionFlag = 1
	else let iActiveCursor = GetActiveCursor ()
		BrailleCursor ()
		PerformScript BraillePriorLine ()
	endIf
	 
EndScript


Script BottomOfFile ()

	if MenusActive ()
	|| GetWindowClass (GetCurrentWindow ()) != wcEditor
	|| Not IsPcCursor () then
		PerformScript BottomOfFile ()
		return
	endIf
	PerformScript ChangeLine()

EndScript


Script TopOfFile ()

	if MenusActive ()
	|| GetWindowClass (GetCurrentWindow ()) != wcEditor
	|| Not IsPcCursor () then
		PerformScript TopOfFile ()
		return
	endIf
	PerformScript ChangeLine()

EndScript


Script JAWSPageDown ()

	if MenusActive ()
	|| GetWindowClass (GetCurrentWindow ()) != wcEditor
	|| Not IsPcCursor () then
		PerformScript JAWSPageDown ()
		return
	endIf
	PerformScript ChangeLine()

EndScript


Script JAWSPageUp ()

	if MenusActive ()
	|| GetWindowClass (GetCurrentWindow ()) != wcEditor
	|| Not IsPcCursor () then
		PerformScript JawsPageUp ()
		return
	endIf
	PerformScript ChangeLine()

EndScript


Script ChangeLine ()

	if MenusActive ()
	|| GetWindowClass (GetCurrentWindow ()) != wcEditor
	|| Not IsPcCursor () then
		TypeCurrentScriptKey ()
		PauseForMovement ()
		SayLine ()
		return
	endIf
	SaveOldValues ()
	TypeCurrentScriptKey ()
	PauseForMovement ()
	GetNewValues ()
	SpeakLineOfText ()
	let MotionFlag = 1

EndScript


Void Function SpeakLineOfText ()

	if BrailleInUse () then
		BrailleLine ()
	endIf
	if StringIsBlank (GetLine ())
	&& IsDocumentWindow (GetCurrentWindow ()) then
		SayUsingVoice (VCTX_PCCURSOR, msgLocalBlank, OT_TEXT)
		return
	endIf
	
	if not nBrailleEdit
	&& gbSpeakPrintLinesByWord
	&& GetPageNum () == -1 then
		SpeakLineAsBrailleCells (GetLine ())
		return
	endIf
	/*if InTable
	&& IsPCCursor () then
		;GetNewValues ()
		SpeakTableEntry ()
		return
	endIf*/
	if not nBrailleEdit then
		SayLine ()
		return
	endIf
	if giBrailleLineSpeakingLevel==1 then
	SpeakLineAsBrailleCells (GetLine ())
	elif giBrailleLineSpeakingLevel==2 then
		let nCatchingUp = 1
		PerformScript SpeakTransLine ()
		while nCatchingUp
		&& not IsKeyWaiting ()
		let nCatchingUp = nCatchingUp-1
			PerformScript SpeakTransLine()
		endWhile
	endIf

EndFunction


Function SpeakSpaces (int iNumSpaces)

	if iNumSpaces > 0 then
		if iNumSpaces == 1 then
			Say (msgSpace, OT_Message)
		else Say (IntToString (iNumSpaces)+msgMultipleSpaces, OT_MESSAGE)
		endIf
	endIf

EndFunction


Int Function DecipherItemOfText ()

var
	string CharacterCheck,
	String StringCheck,
	string FontCheck,
	string sStyleName,
	string nVersion,
	string c

	if GetCursorCol () == nStartCol
	&& GetCursorRow () == nStartRow
;	&& not iSuppressSameSpot
	&& GetPageNum () != -1
	&& not nHighlight
	&& not nSelectingText
	&& not nSaySelectAfter then
		PlaySound (FindJAWSSoundFile ("tonguethump.wav"))
		GetStyleName ()
		return 1
	endIf
	if GetPageNum () != -1 then
		return 1
	endIf
	let c = GetCharacter ()
	if StringLength (c) > 1 then
		DecipherItemUIA (c)
		return 0
	endIf
	if GetCharacter () == "[" then
		NextCharacterSelectIfOn  ()
		RouteInvisibleToPc ()
		PauseForMovement ()
		InvisibleCursor ()
		let CharacterCheck = GetPriorCharacter ()
		PCCursor ()
		PriorCharacterSelectIfOn ()
		if CharacterCheck != "[" then
			SayMessage (OT_TEXT, msgCode, msgCode)
			SpellString (StringChopLeft (StringSegment (GetToEndOfLine (), "]", 1), 1))
			if StringStartsWith (StringChopLeft (StringSegment (GetToEndOfLine (), "]", 1), 1), "htbs") then
				let PreTable = 3
			endIf
			return 0
		else return 1
		endIf
	elIf StringIsBlank (GetFont ())
	&& StringContains (GetToEndOfLine (), "[") == 1 then
		SayMessage (OT_TEXT, msgCode, msgCode)
		SpellString (StringChopLeft (StringSegment (GetToEndOfLine (), "]", 1), 1))	
		return 0
	elif GetCharacter () == "<" then
		NextCharacterSelectIfOn ()
		RouteInvisibleToPc ()
		PauseForMovement ()
		InvisibleCursor ()
		let CharacterCheck = GetPriorCharacter ()
		PCCursor ()
		PriorCharacterSelectIfOn ()
		if CharacterCheck != "<" then
			let sStyleName = (StringChopLeft (StringSegment (GetToEndOfLine (), ">", 1), 1))	
			if StringLeft (sStyleName, 1) == "/" then
				SayUsingVoice (VCTX_KEYBOARD, msgEndStyle+" "+(StringChopLeft (sStyleName, 1)), OT_TEXT)
			else
				SayUsingVoice (VCTX_KEYBOARD, msgBeginStyle+" "+sStyleName, OT_TEXT)
			endIf
			return 0
		else return 1 
		endIf
	/* section below is for a code or style code at the start of a line
	following a code or style code at the end of the previous line */
	elif GetColorBackground () == 0
	&& StringIsBlank (GetFont ())
	&& not nHighlight
	&& GetLineNum () == -1 then
		if StringLength (GetToEndOfLine ()) == 0 then
			SaveCursor ()
			RouteInvisibleToPc ()
			NextLine ()
			JawsHome ()
			let CharacterCheck = GetCharacter ()
			let StringCheck = GetToEndOfLine ()
			PcCursor ()
			RestoreCursor ()
			Pause ()
		else let StringCheck = GetToEndOfLine ()
			let CharacterCheck = SubString (StringCheck, 1, 1)
		endIf
		if CharacterCheck == "<" then
			SayMessage (OT_TEXT, msgStyle, msgStyle)
			Say (StringChopLeft (StringSegment (StringCheck, ">", 1), 1), OT_TEXT, FALSE)
		elif CharacterCheck == "[" then
			SayMessage (OT_TEXT, msgCode, msgCode)
			SpellString (StringChopLeft (StringSegment (StringCheck, "]", 1), 1))
		else SayMessage (OT_TEXT, msgLocalSpace, msgLocalSpace)
		endIf
		return 0
	endIf
; A "{" is spoken as a character: the verbose labels in braces belonged to program versions BrailleMaker never had.
	if GetCharacter () == "{" then
		SayTheCharacter ()
		return 0
	endIf
	return 1

endFunction


Function DecipherItemUIA (string sText)

var
	string sStyleName

	if StringStartsWith (sText, "[", FALSE) then
		SayMessage (OT_TEXT, msgCode, msgCode)
		SpellString (StringChopLeft (StringSegment (sText, "]", 1), 1))
		return
	elIf StringStartsWith (sText, "<", FALSE) then
		let sStyleName = (StringChopLeft (StringSegment (sText, ">", 1), 1))
		if StringLeft (sStyleName, 1) == "/" then
			SayUsingVoice (VCTX_KEYBOARD, msgEndStyle+" "+(StringChopLeft (sStyleName, 1)), OT_TEXT)
		else
			SayUsingVoice (VCTX_KEYBOARD, msgBeginStyle+" "+sStyleName, OT_TEXT)
		endIf
	else return
	endIf

EndFunction



Void Function NextCharacterSelectIfOn ()

	if nSelectingText
	&& !nHighlight then
		AppSpeechOff ()
		SelectNextCharacter ()
		AppSpeechOn ()
	else NextCharacter ()
	endIf

EndFunction



Void Function PriorCharacterSelectIfOn ()

	if nSelectingText
	&& !nHighlight then
		AppSpeechOff ()
		SelectPriorCharacter ()
		AppSpeechOn ()
	else PriorCharacter ()
	endIf

EndFunction


Void Function NextWordSelectIfOn ()

	if nSelectingText
	&& !nHighlight then
		AppSpeechOff ()
		SelectNextWord ()
		AppSpeechOn ()
	else NextWord ()
	endIf

EndFunction


Void Function PriorWordSelectIfOn ()

	if nSelectingText
	&& !nHighlight then
		AppSpeechOff ()
		SelectPriorWord ()
		AppSpeechOn ()
	else PriorWord ()
	endIf

EndFunction


Void Function NextLineSelectIfOn ()

	if nSelectingText
	&& !nHighlight then
		AppSpeechOff ()
		SelectNextLine ()
		AppSpeechOn ()
	else NextLine ()
	endIf

EndFunction


Void Function PriorLineSelectIfOn ()

	if nSelectingText
	&& !nHighlight then
		AppSpeechOff ()
		SelectPriorLine ()
		AppSpeechOn ()
	else PriorLine ()
	endIf

EndFunction


Script SayWord ()

var
	int nTimesPressed,
	int nL,
	string sString1,
	string sString2

	let nTimesPressed = IsSameScript ()
	if nTimesPressed > 0
	&& not iSuppressMulti then
		PerformScript SayWord()
		return
	endIf
	let iSuppressMulti = 0
	if not IsDocumentWindow (GetCurrentWindow ())
	|| Not IsPcCursor () then
		PerformScript SayWord()
		return
	endIf
	if DecipherItemOfText () == 0 then
		return
	endIf
	if nBrailleDoc then
		SpeakWordAsBrailleCells (GetToEndOfLine ())
	else
		let sString1 = GetToEndOfLine ()
		NextWordSelectIfOn ()
		let sString2 = GetToEndOfLine ()
		PriorWordSelectIfOn ()
		let nL = StringContains (sString1, sString2)
		if nL > 0 then
			SayUsingVoice (VCTX_PCCURSOR, Substring (GetToEndOfLine (), 1, nL-1), OT_WORD)
		else SayUsingVoice (VCTX_PCCURSOR, sString1, OT_WORD)
		endIf
	endIf

EndScript


Void Function SayWordUnit (int UnitMovement)

	if IsDocumentWindow(GetCurrentWindow ())
	&& IsPCCursor () then
		let iSuppressMulti = 1
		PerformScript SayWord ()
		let MotionFlag = 1
		let SaySkipFlag = 1
;let nStartCol = GetCursorCol ()
;let nStartRow = GetCursorRow ()
		let iSuppressMulti = 0
	else SayWordUnit (UnitMovement)
	endIf


EndFunction


Script SayNextWord ()

	if MenusActive () then
		NextWord ()
		return
	endIf
	if not IsDocumentWindow (GetCurrentWindow ())
	|| Not IsPcCursor () then
		PauseForMovement ()
		PerformScript SayNextWord ()
		return
	endIf
	if nHighlight then
		PerformScript SelectNextWord ()
		return
	endIf
	let MotionFlag = 1
	if GetPageNum () == -1 then
		let iLineChange = 0
	else
		let nStartCol = GetCursorCol ()
		let nStartRow = GetCursorRow ()
		if StringIsBlank (GetToEndOfLine ()) then
			let iLineChange = 1
			;let MotionFlag = 1
	SaveOldValues ()
			let SaySkipFlag = 1
		else let iLineChange = 0
		endIf
	endIf
	NextWord ()
	if iLineChange then
		GetNewValues ()
	endIf
	let MotionFlag = 1
	if DecipherItemOfText () == 0 then
		return
	endIf
	let iSuppressMulti = 1
	PerformScript SayWord()

EndScript


Script SayPriorWord ()

	if MenusActive () then
		PriorWord ()
		return
	endIf
	if not IsDocumentWindow (GetCurrentWindow ())
	|| Not IsPcCursor () then
		PauseForMovement ()
		PerformScript SayPriorWord ()
		return
	endIf
	if nHighlight then
		PerformScript SelectPriorWord()
		return
	endIf
	let MotionFlag = 1
	if GetPageNum () == -1 then
		let iLineChange = 0
	else
		let nStartCol = GetCursorCol ()
		let nStartRow = GetCursorRow ()
		if StringIsBlank (GetFromStartOfLine ()) then
			let iLineChange = 1
			let MotionFlag = 1
			SaveOldValues ()
			let SaySkipFlag = 1
		endIf
	endIf
	PriorWord ()
	if iLineChange == 1 then
		GetNewValues ()
	endIf
	let MotionFlag = 1
	if DecipherItemOfText () == 0 then
		return
	endIf
	let iSuppressMulti = 1
	PerformScript SayWord()

EndScript


Script SayCharacter ()

var
	int nTimesPressed

	let nTimesPressed = IsSameScript ()
	if nTimesPressed > 0
	&& not nNewPoint
	&& not iSuppressMulti then
		if nTimesPressed == 2 then
			AddHook (HK_SCRIPT, "CharacterValueHook")
		elif nTimesPressed == 1 then
			AddHook (HK_SCRIPT, "PhoneticSpellHook")
		endIf
		if DecipherItemOfText () == 0 then
			return
		endIf
		let MotionFlag=1
		PerformScript SayCharacter()
		return
	endIf
	let nNewPoint = 0
	if MenusActive () then
		SayCharacter ()
		return
	endIf
	If IsJawsCursor ()
	&& GetWindowClass (GetCurrentWindow ()) == wcEditor then
		if Not IsPointInWindow (GetFirstWindow (GetCurrentWindow ()), GetCursorCol (), GetCursorRow ()) then
			SayBrailleChar (VCTX_PCCURSOR, GetCharacter ())
			return
		EndIf
	EndIf
	if GetWindowClass (GetCurrentWindow ()) != wcEditor
	|| Not IsPcCursor () then
		PerformScript SayCharacter ()
		return
	endIf
	if DecipherItemOfText () == 0 then
		return
	endIf
	SayTheCharacter ()

EndScript


Void Function SayCharacterUnit (int UnitMovement)

	if IsDocumentWindow(GetCurrentWindow ()) then
		let iSuppressMulti = 1
		PerformScript SayCharacter ()
		let MotionFlag = 1
		let nStartCol = GetCursorCol ()
		let nStartRow = GetCursorRow ()
	endIf

EndFunction


Void Function SayTheCharacter ()

var
	int i1,
	int i2,
	string sUniVal

	if GetWindowClass (GetCurrentWindow ()) == wcCodesEdit then
		Refresh (0)
		PauseForMovement ()
	endIf
	PauseForMovement ()
	let sStatusLine = GetStatusLineText ()
	PauseForMovement ()
	let sUniVal = GetUnicodeValue ()
	if GetCharacterValue (GetCharacter ()) == 30
	|| GetCharacterValue (GetCharacter ()) == 160
|| sUniVal == "[']" then
		SayMessage (OT_TEXT, msgLocalHardSpace, msgLocalHardSpace)
		return
	endIf
	if sUniVal == "[-]" then
		SayMessage (OT_TEXT, msgLocalAssistedHyphen, msgLocalAssistedHyphen)
		return
	endIf
	If GetCharacter () == " "
	|| GetCharacterValue (GetCharacter ()) == 20 then
		SayMessage (OT_TEXT, msgLocalSpace, msgLocalSpace)
		return
	endIf
	if sUniVal == "2011" then
		SayMessage (OT_TEXT, msgLocalHardHyphen, msgLocalHardHyphen)
		return
	endIf
	if nBrailleEdit then
		SayBrailleChar (VCTX_PCCURSOR, GetCharacter ())
		return
	endIf
	/*if DecipherItemOfText () == 0 then
		return
	endIf*/
	if !gbUseStatusLineForUnicodeValues then
		SayCharacter ()
		return
	endIf
	if not StringIsBlank (sUniVal) then
		SpellString (MakeCharacterFromValue (HexToDec (sUniVal)))
		return
	endIf
	if not nBrailleEdit then
		;SayString ("mango")
		SayCharacter ()
	elif StringIsBlank (GetFont ()) then
		SayMessage (OT_TEXT, msgLocalBlank, msgLocalBlank)
	endIf

EndFunction


Script SayNextCharacter ()

	if MenusActive () then
		PerformScript SayNextCharacter ()
		return
	endIf
	If IsJawsCursor ()
	&& GetCurrentWindow () == GetNextWindow (GetNextWindow (GetFocus ())) then
		SayNextCharacterTranslatedLine ()
		return
	endIf
	if GetWindowClass (GetCurrentWindow ()) == wcCodesEdit then
		let MotionFlag = 1
		NextCharacter ()
		;let sTransLine = GetWindowText (hndTransLine, 0)
		Refresh (0)
		;PauseForMovement ()
		SayTheCharacter ()
		return
	endIf
	if not IsDocumentWindow (GetCurrentWindow ())
	|| Not IsPcCursor () then
		NextCharacter ()
		SayCharacter ()
		return
	endIf
	if nHighlight then
		PerformScript SelectNextCharacter()
		return
	endIf
	let MotionFlag = 1
	if GetPageNum () == -1
		let iLineChange = 0
	else
		let nStartCol = GetCursorCol ()
		let nStartRow = GetCursorRow ()
		if StringIsBlank (GetToEndOfLine ()) then
			let iLineChange = 1
			let MotionFlag = 1
			SaveOldValues ()
			let SaySkipFlag = 1
		else let iLineChange = 0
		endIf
	endIf
	NextCharacter ()
	if iLineChange then
		GetNewValues ()
	endIf
	let MotionFlag = 1
	if DecipherItemOfText () == 0 then
		;let MotionFlag = 1
		return
	endIf
	SayTheCharacter ()
	let MotionFlag = 1

EndScript


Script SayPriorCharacter ()

	if MenusActive () then
		PerformScript SayPriorCharacter ()
		return
	endIf
	
	If IsJawsCursor ()
	&& GetCurrentWindow () == GetNextWindow (GetNextWindow (GetFocus ())) then
		SayPriorCharacterTranslatedLine ()
		return
	endIf
	if GetWindowClass (GetCurrentWindow ()) == wcCodesEdit then
		PriorCharacter ()
		let MotionFlag = 1
		RefreshWindow (GetCurrentWindow ())
		SayTheCharacter ()
		return
	endIf
	if not IsDocumentWindow (GetCurrentWindow ())
	|| Not IsPcCursor () then
		PriorCharacter ()
		SayCharacter ()
		;SayString ("hello there")
		return
	endIf
	if nHighlight then
		PerformScript SelectPriorCharacter()
		return
	endIf
	let MotionFlag = 1
	if GetPageNum () == -1 then
		let iLineChange = 0
	else let nStartCol = GetCursorCol ()
		let nStartRow = GetCursorRow ()
		if StringIsBlank (GetFromStartOfLine ()) then
			let iLineChange = 1
			;let MotionFlag = 1
			SaveOldValues ()
			let SaySkipFlag = 1
		endIf
	endIf
	PriorCharacter ()
	if iLineChange == 1 then
		GetNewValues ()
	endIf
	let iLineChange = 0
	let MotionFlag = 1
	if DecipherItemOfText () == 0 then
		return
	endIf
	SayTheCharacter ()
	let MotionFlag = 1

EndScript


Void Function SayNextCharacterTranslatedLine ()

	NextCharacter ()
	if nBrailleEdit then
		SayCharacter ()
	else
SayBrailleChar (VCTX_JAWSCURSOR, GetCharacter ())
	EndIf

	if StringLength (GetToEndOfLine ()) == 1 then
		SayMessage (OT_STATUS, msgEndLine, msgEndLine)
	endIf

EndFunction 	


Void Function SayPriorCharacterTranslatedLine ()

		PriorCharacter ()
	if IsBrailleDocumentWindow (GetCurrentWindow ()) then
		SayCharacter ()
	else
SayBrailleChar (VCTX_JAWSCURSOR, GetCharacter ())
	EndIf
	if StringLength (GetFromStartOfLine ()) == 0 then
		SayMessage (OT_STATUS, msgStartLine, msgStartLine)
	endIf

EndFunction


Script SayDocNameAndStatusLine ()

;HandleCustomWindows (GetCurrentWindow ())
	if not StringContains (GetWindowName (GetRealWindow (GetCurrentWindow())), scDocument) then
		SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
	else
		Say (GetWindowName (GetRealWindow (GetCurrentWindow ()))+":", OT_USER_REQUESTED_INFORMATION)
		let sStatusLine = GetStatusLineText ()
		Say (sStatusLine, OT_USER_REQUESTED_INFORMATION)
	endIf

EndScript


Int Function CheckTransLine ()

var
	handle hndDocument,
	;handle hndTransLine,
	int nTransLineShowing,
	string sTransLineFrame

	let nTransLineShowing = 0
	let hndDocument = GetCurrentWindow ()
	SaveCursor ()
	InvisibleCursor ()
	let hndTransLine = GetNextWindow (GetNextWindow (hndDocument))
	PauseForMovement ()
	if StringContains (StringLower (GetWindowType (GetCurrentWindow ())), wtUnknown) then
		let nTransLineShowing = 0
		else
		if GetLine () == scErrorMessage0 then
			let nTransLineShowing = -1
			else let nTranslineShowing = 1
			FrameSetOnWindow (hndTransLine)
			FramesListHelper (sTransLineFrame, GetWindowLeft (hndTransLine), GetWindowTop (hndTransLine), GetWindowBottom (hndTransLine), GetWindowRight (hndTransLine))
			;SayString (sTransLineFrame)
;ShowFrameText ()			
		endIf
	endIf
	
	return nTransLineShowing

EndFunction


Script SpeakTransLine ()

var
	handle hndTransLine,
	string CurLine,
	int NOBackTrans,
	int VoiceRate,
	int xPos,
	int DelayCounter,
	int BrailleModeSave,
	int nTransLineCheck,
	int nPanTime,
	string sTransLine,
	string buffer,
	int nSpellIt,
	int nReverseCheck,
	int nTransLength,
	string sFinalTransLine

	if GetCurrentScriptKeyName () == ksSpeakTransLine then
		if not IsDocumentWindow (GetCurrentWindow ()) then
			SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
			return
	endIf
;	let NoBackTrans = 0
	endIf
	if not CheckTransLine () then
		PauseForMovement ()
		if not CheckTransLine () then
			TypeKey ("Control+F8")
			SayMessage (OT_STATUS, msgTransLineOn, msgTransLineOn)
			PauseForMovement () 
		endIf
	endIf
	let hndTransLine = GetNextWindow (GetNextWindow (GetCurrentWindow ()))
	SaveCursor ()
	InvisibleCursor ()
	MoveToWindow (hndTransLine)
	let sTransLine = GetLine ()
	let nReverseCheck = 0
	let nSpellIt = 0
	let nTransLength = StringLength (sTransLine)
	if StringLength(StringRemoveCharsInRange (sTransLine, 1424, 1791)) < nTransLength-2
	|| StringLength(StringRemoveCharsInRange (sTransLine, 65024, 65279)) < nTransLength-2
	|| StringLength(StringRemoveCharsInRange (sTransLine, 64285, 64335)) < nTransLength-2 then
	;&& not 	IsRTLLanguageProcessorLoaded () then
		;SayString ("dina")
		let nReverseCheck = 1
		let nSpellIt = 1
	endIf
	PCCursor ()
	RestoreCursor ()
	if nBrailleEdit then
		if not IsDocumentWindow (GetCurrentWindow ())
		&& not IsKeyWaiting () then
;			SayUsingVoice (VCTX_MESSAGE, msgCatchingUp, OT_STATUS)
			let nCatchingUp = 2
			return
		elif nCatchingUp == 1 then
			return
		endIf
		if nReverseCheck then
			let sFinalTransLine = StringReverse(sTransLine)
		else let sFinalTransLine = sTransLine
		endIf
			
		if not nCatchingUp then
			if not nSpellIt then
			Say (sFinalTransline, OT_TEXT, FALSE)
		else SpellString (sFinalTransLine)
		endIf
		endIf
		return
	endIf
	SaveCursor ()
	InvisibleCursor ()
	MoveToWindow (hndTransLine)
	SpeakStringAsBrailleCells (VCTX_JAWSCURSOR, sTransLine)
	RestoreCursor ()
	PcCursor ()
	return

EndScript

Void Function SpeakWordAsBrailleCells (string sBrailleText)

var
	int i,
	string sNextWord

	let i = 1
	NextWordSelectIfOn ()
	let sNextWord = GetToEndOfLine ()
	PriorWordSelectIfOn ()
	while (i <= StringLength (sBrailleText))
		if SubString (sBrailleText, i, 1) == " " || StringChopLeft (sBrailleText, i-1) == sNextWord then
			return
		endIf
		SayBrailleChar (VCTX_PCCURSOR, SubString (sBrailleText, i, 1))
		let i = i+1
		Delay (3)
	endWhile

EndFunction


Void Function SpeakLineAsBrailleCells (string  sBrailleLine)

var
	int iFinalSpaces,
	int iCol,
	int iRow

	if StringIsBlank (GetLine ()) then
		SayMessage (OT_TEXT, msgLocalBlank)
		return
	endIf
	let iCol = GetCursorCol ()
	let iRow = GetCursorRow ()
	let iFinalSpaces = CountFinalSpaces ()
	if GetLineNum () != -1 then
		SpeakStringAsBrailleCells (VCTX_PCCURSOR, StringTrimTrailingBlanks (GetLine ()))
		if nBrailleEdit then
			SpeakSpaces (iFinalSpaces)
		endIf
	else
		JAWSHome ()
		if GetCharacterValue (GetCharacter ()) == 32 then
			NextWord ()
			SpeakSpaces (CountPrecedingSpaces ())
		endIf
		PerformScript SayWord ()
		NextWord ()
		while StringLength (StringTrimLeadingBlanks (GetFromStartOfLine ())) > 0
		&& not IsKeyWaiting ()
		&& not StringIsBlank (GetToEndOfLine ())
			if nBrailleEdit then
				SpeakSpaces (CountPrecedingSpaces ())
			endIf
			PerformScript SayWord ()
			NextWord ()
		endWhile
		if nBrailleEdit then
			SpeakSpaces (iFinalSpaces)
		endIf
		let iViewToggle = 1
		if GetCursorCol () != iCol then
			MoveTo (iCol, iRow)
		endIf
endIf

EndFunction


Void Function SpeakStringAsBrailleCells (string VCTX, string sBrailleText)

var
	int i,
	int j,
	int iStringLength

	let i = 1
	let iStringLength = StringLength (sBrailleText)
	while i <= iStringLength
	&& not IsKeyWaiting ()
		if Substring (sBrailleText, i, 1) == " "
		&& Substring (sBrailleText, i+1, 1) == " " then
			let j = StringLength (StringChopLeft (sBrailleText, i)) - StringLength (StringTrimLeadingBlanks (StringChopLeft (sBrailleText, i)))+1
			Say (IntToString (j)+msgMultipleSpaces, OT_MESSAGE, FALSE)
			;Say (IntToString (j)+"spaces", FALSE)
			let i = i+j
		else
			SayBrailleChar (VCTX, Substring(sBrailleText, i, 1))
			Delay (3)
			let i = i+1

		endIf
	endWhile

EndFunction

Int Function CountPrecedingSpaces ()

var
	string sChar,
	int iCount,
	int iSpaces
	
	SaveCursor ()
	InvisibleCursor ()
	RouteInvisibleToPc ()
	PriorCharacter ()
	let sChar = GetCharacter ()
	let iCount = 0
	while GetCharacterValue (GetCharacter ()) == 32
		let iCount = iCount+1
		PriorCharacter ()
	endWhile
	let iSpaces = iCount
	;SayString ("iSpaces is "+IntToString (iSpaces))
	RestoreCursor ()
	PCCursor ()
	return iSpaces

EndFunction


Int Function CountFinalSpaces ()

var
	int iCount,
	int iSpaces,
	string sChar

	SaveCursor ()
	InvisibleCursor ()
	RouteInvisibleToPc ()
	JAWSEnd ()
	let sChar = GetCharacter ()
	let iCount = 0
	
	if GetCharacterValue (sChar) == 0 then
		PriorCharacter ()
	endIf
	while GetCharacterValue (GetCharacter ()) == 32
		let iCount = iCount+1
		PriorCharacter ()
	endWhile
	let iSpaces = iCount
	;SayString ("iSpaces is "+IntToString (iSpaces))
	RestoreCursor ()
	PCCursor ()
	return iSpaces

EndFunction


Script TransLineCursorToggle ()

var
	int nTransLineCheck

	if not iTableEntry then
		if not IsDocumentWindow (GetCurrentWindow ())
		&& not IsDocumentWindowJAWSCursor (GetCurrentWindow ()) then
			SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
			return
		endIf
	endIf
	if IsJawsCursor () then
		PcCursor ()
		SayMessage (OT_STATUS, cmsg9_l, cmsg9_s)
		return
	endIf
	if not CheckTransLine () then
		TypeKey (ksControlF8Press)
			SayMessage (OT_STATUS, msgTurningOnTransLine, msgSilent)
		;PauseForMovement ()
	endIf
	if not CheckTransLine () then
		SayMessage (OT_ERROR, msgTransLineNotFound, msgTransLineNotFound)
		return
	endIf
	RouteJAWSToPc ()
	JAWSCursor ()
	MoveToWindow (GetNextWindow (GetNextWindow (GetCurrentWindow ())))
	JawsHome ()
	if not ITableEntry then
		SayMessage (OT_STATUS, msgJawsCursorOnTransLine, msgJawsCursorOnTransLine)
	endIf
	
EndScript


Script ReadLineInBrailleDocToggle ()

	if giBrailleLineSpeakingLevel == 0 then
		SayMessage (OT_STATUS, msgReadBackTrans, msgReadBackTrans)
		let giBrailleLineSpeakingLevel = 2
	elif giBrailleLineSpeakingLevel == 2 then
		SayMessage (OT_STATUS, msgReadBrailleLinesAsBrailleChars, msgReadBrailleLinesAsBrailleChars)
		let giBrailleLineSpeakingLevel = 1
	elif giBrailleLineSpeakingLevel == 1 then
		SayMessage (OT_STATUS, msgSilentBrailleLines, msgSilentBrailleLines)
		let giBrailleLineSpeakingLevel = 0
	endIf

EndScript


Script ReadStyleChangeToggle ()

var
	int nTimesPressed

; if not in a document window, just pass keystroke through to the application
; there was a conflict for Alt+I in the help system
;	toggles through settings for reading style change and skipped lines
	if not IsDocumentWindow (GetFocus ()) then
		TypeCurrentScriptKey ()
		return
	endIf
	if not gbSkipVerbosity 
	&& not gbStyleChangeVerbosity then
		SayMessage (OT_STATUS, msgSpeakStyleChange, msgSpeakStyleChange)
		SayMessage (OT_STATUS, msgSkipsOff, msgSkipsOff)
		let gbStyleChangeVerbosity = TRUE
	elif gbStyleChangeVerbosity
	&& not gbSkipVerbosity then
		SayMessage (OT_STATUS, msgSpeakStyleChange, msgSpeakStyleChange)
		SayMessage (OT_STATUS, msgSpeakSkips, msgSpeakSkips)
		let gbSkipVerbosity = 1
	elif gbStyleChangeVerbosity
	&& gbSkipVerbosity then
		SayMessage (OT_STATUS, msgStyleChangeOff, msgStyleChangeOff)
		SayMessage (OT_STATUS, msgSpeakSkips, msgSpeakSkips)
		let gbStyleChangeVerbosity = 0
elif not gbStyleChangeVerbosity
	&& gbSkipVerbosity then
		SayMessage (OT_STATUS, msgStyleChangeOff, msgStyleChangeOff)
		SayMessage (OT_STATUS, msgSkipsOff, msgSkipsOff)
		let gbSkipVerbosity = 0
	endIf

EndScript


Script BrailleDocInputEchoToggle ()

; if not in a document window, just pass keystroke through to the application
; there was a conflict for Alt+P for Printer Properties
if GetCurrentScriptKeyName () == ksBrailleDocInputEchoToggle
		&& not IsDocumentWindow (GetCurrentWindow ()) then
		TypeCurrentScriptKey ()
		return
	endIf
	let giBrailleDocInputEcho = giBrailleDocInputEcho+1
	if giBrailleDocInputEcho == 5 then
		let giBrailleDocInputEcho = 0
	endIf
	Say (msgBrailleDocInputEcho
	+StringSegment (csBrailleDocInputEcho, ";", giBrailleDocInputEcho+1), FALSE)
	if nBrailleEdit then
		if giBrailleDocInputEcho == 1 then
			let nTypingEchoSaved = GetJCFOption(OPT_TYPING_ECHO) ; GetJCFOption(OPT_TYPING_ECHO)
			PauseForMovement ()
			SetJcfOption(OPT_TYPING_ECHO,0) ; echo off
		endIf
		if giBrailleDocInputEcho == 0 then
			SetJcfOption(OPT_TYPING_ECHO,nTypingEchoSaved) ; restoring saved echo setting
		endIf
	endIf

EndScript


Script ShowRequestedStatusInfoOnBrailleDisplayToggle ()

; if not in a document window, just pass keystroke through to the application
	if gbShowRequestedStatusInfoOnBrailleDisplay==0 then
		Say (msgShowRequestedStatusInfoOnBrailleDisplayOn, OT_STATUS)
		let gbShowRequestedStatusInfoOnBrailleDisplay= 1
		SetJCFOption(OPT_BRL_MESSAGES, 1)
	else Say (msgShowRequestedStatusInfoOnBrailleDisplayOff, OT_STATUS)
		let gbShowRequestedStatusInfoOnBrailleDisplay= 0
		SetJCFOption(OPT_BRL_MESSAGES, 0)
	endIf
	if GetWindowClass (GetCurrentWindow ()) != "MDIClient" then
		HandleCustomWindows (GetCurrentWindow ())
	endIf

EndScript


Script SayStyleName ()

var
	string sStyleName,
	int nTimesPressed

	let nTimesPressed = IsSameScript ()
	if nTimesPressed > 0
	&& InTable then
		SayUsingVoice (VCTX_MESSAGE, msgRow+sRowNum+" "+msgCol+sColNum, OT_USER_REQUESTED_INFORMATION)
		return
	endIf
	let sStyleName = GetStyleName ()
	if StringIsBlank (sStyleName) then
		SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
	else
		SayUsingVoice (VCTX_MESSAGE, sStyleName, OT_USER_REQUESTED_INFORMATION)
		RestoreCursor ()
	endIf

EndScript


Script SayLocation ()

var
;	string sWindowName,
	string sLocation,
	string sPage,
	string sLine,
	string sColumn,
	int iPagePos

	let sWindowName = GetWindowName (GetRealWindow (GetCurrentWindow ())) 
	if not IsDocumentWindow (GetFocus ()) then
		SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
		return
	endIf
	if GetPageNum () == -1 then
		SayMessage (OT_STATUS, msgNoLocation_l, msgNoLocation_s)
		return
	endIf
	let sLine = IntToString (GetLineNum ())
	let sPage = GetPageNum ()
	let sColumn = GetColumnNum ()
	let sLocation = msgPageId+StringTrimTrailingBlanks(sPage)+" "+msgLineId+sLine+" "+msgColId+sColumn
	Say (sLocation, OT_USER_REQUESTED_INFORMATION)

EndScript
	


Void Function HandleCustomWindows (handle hWnd)

var

	string buffer,
	handle hWindow,
	int i,
	int wt,
	int wl,
	int wb,
	int wr
	
		SendMessage (GetAppMainWindow (GetCurrentWindow ()), 0X0112, /*WM_SYSCOMMAND,*/ 0XF030, 0) /* maximize the window*/
		SendMessage (GetFirstChild (GetFirstChild (GetAppMainWindow (GetCurrentWindow ()))), 0X0112, /*WM_SYSCOMMAND,*/ 0XF030, 0) /* maximize the window*/
	if GetWindowClass (GetCurrentWindow ()) != wcCodesEdit then
		SendMessage (GetFirstChild (GetFirstChild (GetAppMainWindow (GetCurrentWindow ()))), 0X0112, /*WM_SYSCOMMAND,*/ 0XF030, 0) /* maximize the window*/
	else Refresh (0)
		PauseForMovement ()
		let MotionFlag = 0
	endIf

	PCCursor ()
	if sWindowName != GetWindowName (GetRealWindow (hWnd)) then
		let sOldWindowName = sWindowName
		let sWindowName = GetWindowName (GetRealWindow (hWnd))
	endIf
	let nHighlightCheck = IsDocumentWindow (GetCurrentWindow ())
	let nOldBrailleEdit = nBrailleEdit
	if GetWindowClass (hWnd) == wcMDIClient then
		if nCustomWindow == 0 then
			SayMessage (OT_CONTROL_NAME, msgMdiClient)
			let nCustomWindow = 1
		elif nCustomWindow == 1 then
			let nCustomWindow = 0
		endIf
	endIf
	if GetWindowName (GetRealWindow (GetCurrentWindow ())) == wnFileNew then
		let nNewDoc = 1
	endIf
	let nFocusChange = 1
	let MotionFlag = 1
	if IsDocumentWindow (hWnd) then
		PauseForMovement ()
		if sOldWindowName == GetWindowName (GetRealWindow (hWnd)) 
		|| nBrailleEdit != nOldBrailleEdit then
		;&& GetWindowName (GetRealWindow (hWnd)) == sWindowName then
		;|| sWindowName != sOldOldWindowName then
				SayUsingVoice (VCTX_MESSAGE, sWindowName, OT_WINDOW_NAME, false)
				PauseForMovement ()
		endIf
let hndStatusLine = GetStatusLine ()
		let sStatusLine = GetStatusLineText ()
		CheckTransLine ()
		;SayString (sStatusLine)
		;SayWindowTypeAndText (hndStatusLine)
		let nKeyPressTime = 0
		let sToEnd = GetToEndOfLine ()
		let nFromStart = StringLength (GetFromStartOfLine ())
		let nLineCount = StringLength (GetLine ())
		let sPriorChar = GetPriorCharacter ()
		let sCurrentWord = GetWord ()
	endIf
;	for the Spell checker
if sWindowName != wnSpellCheck then
	let iSpellCheck = 0
endIf
	if GetWindowTypeCode (hWnd) == WT_BUTTON

	&& GetWindowName (hWnd) == wnOK then
			if(GetWindowName (GetRealWindow (hWnd)) == "Error"
			|| GetWindowName (GetRealWindow (hWnd)) == "Warning") then
				SayWindowTypeAndText (GetParent (hWnd))
			endIf
				;SayString ("puppy")
			MoveToWindow (hWnd)
		endIf
	if sWindowName == wnSpellCheck
	&& not iSpellCheck then
		let iSpellCheck = 1
		let SpellCheckWindow = GetParent (GetCurrentWindow ())
		PerformScript SpeakSpellCheckWord ()
		if getScriptAssignedTo (GetCurrentScriptKeyName ()) != "SayWindowPromptAndText"
		;if GetCurrentScriptKeyName () != "Insert+Tab"
		; && GetCurrentScriptKeyName () != "JAWSKey+Tab"
		&& nFocusChange != 3 then
			let nFocusChange = 2
		elif nFocusChange == 3 then
			let nFocusChange = 1
		elif nFocusChange == 2 then
			let nFocusChange = 0
		endIf
	;for window that terminates replace dialog
	elif GetWindowTypeCode (hWnd) == WT_BUTTON
	&& GetWindowName (GetCurrentWindow ()) == wnOK
	&& GetWindowName (GetParent (hWnd)) == wnApp 
	&& GetWindowType (GetParent (hWnd)) == "dialog" then
		MoveToWindow (GetParent (hWnd))
		let hWindow = GetFirstChild (GetCurrentWindow ())
		While GetParent (hWindow) == GetParent (hWnd)
			SayWindowTypeAndText (hWindow)
			let hWindow = GetNextWindow (hWindow)
		endWhile
		MoveToWindow (hWnd)
		return
	endIf
	if sWindowName == msgShortcutPreferences then
		StopSpeech ()
	elif GetWindowClass (GetCurrentWindow ()) == wcEditor
	&& not StringIsBlank (GetStyleName ()) then
		if not nNewDoc then
;			PauseForMovement ()
			let nOldHighlight = nHighlight
			let nHighlightCheck = 1
		else let nHighlightCheck = 0
			let nNewDoc = 0
		endIf
	endIf
	if nHighlightCheck then
		let nOldHighlight = nHighlight
			highlighting ()
		endIf
		PauseForMovement ()
		let nBrailleDoc =IsBrailleDocumentWindow (GetCurrentWindow ()) 
	let nBrailleEdit = nBrailleDoc
	|| nOldBrailleEdit
&& GetWindowClass (hWnd) == wcCodesEdit
		if nBrailleEdit
	&& (not nOldBrailleEdit
	|| nBrailleStart) then
		if GetJAWSMajorVersionNumber () >= 2022 then
			let nBrailleProfileStateSaved = BrailleGetProfileState ()
			;SayString ("nBrailleProfileStateSaved = "+IntToString (nBrailleProfileStateSaved))
		BrailleSetProfileState (0)
	else
		let nG2TranslationSaved = GetJCFOption (OPT_BRL_G2TRANSLATION)
		let nContractedInputSaved = GetJCFOption (OPT_BRL_CONTRACTED_INPUT)
		SetJCFOption (OPT_BRL_G2TRANSLATION, 0)
	endIf
	let iViewToggle = 1

	let nEightDotSaved = GetJCFOption (OPTBRL_Eight_Dot)
	SetJCFOption (OPTBRL_Eight_Dot, 0)
	let nG2ExpandCurrentWordSaved = GetJCFOption (OPT_BRL_G2EXPAND_CURRENT_WORD)
		let nG2ExpandCurrentWordSaved = GetJCFOption (OPT_BRL_CONTRACTED_INPUT)
		SetJCFOption (OPT_BRL_G2EXPAND_CURRENT_WORD, 0)
if giBrailleDocInputEcho then
	if GetJCFOption (OPT_TYPING_ECHO) then
			let nTypingEchoSaved = GetJCFOption (OPT_TYPING_ECHO)
		endIf
		SetJCFOption (OPT_TYPING_ECHO, 0)
		;SetDefaultJCFOption (OPT_TYPING_ECHO, 0)
	endIf
	if not giBrailleDocInputEcho then
		setJCFOption (OPT_TYPING_ECHO, nTypingEchoSaved)
		;setDefaultJCFOption (OPT_TYPING_ECHO, nTypingEchoSaved)
	endIf
	endIf
	if not nBrailleEdit then
		if GetJAWSMajorVersionNumber () >= 2022 then
			BrailleSetProfileState (nBrailleProfileStateSaved)
		else
		SetJCFOption (OPT_BRL_G2TRANSLATION, nG2TranslationSaved)
		SetJCFOption (OPT_BRL_CONTRACTED_INPUT, nContractedInputSaved)
	endIf
		SetJCFOption (OPT_BRL_G2EXPAND_CURRENT_WORD, nG2ExpandCurrentWordSaved)
			SetJCFOption (OPTBRL_Eight_Dot, nEightDotSaved)
		if giBrailleDocInputEcho then
			SetJcfOption(OPT_TYPING_ECHO,nTypingEchoSaved) ; restoring  saved echo setting
		endIf
	endIf
	;UIARefresh (0, 1)
	RouteBrailleToPc ()
	;HjDialogs should always use older WindowTypeAndText code,
	;So that on Windows 9X PositionInGroup info never misreports items
	If InHjDialog () then
		SayWindowTypeAndText (hWnd)
		;For input edit boxes:
		If GetWindowSubtypeCode (hWnd) == WT_MULTILINE_EDIT then
			SayWindow (hWnd, TRUE)
			;SayString ("mama")
		EndIf
		Return TRUE
	EndIf
	Return FALSE

EndFunction


Script JAWSBackspace ()

var
	int iIsTop

	if not IsDocumentWindow (GetCurrentWindow ())
 && not (GetWindowClass (GetCurrentWindow ()) == wcCodesEdit)
	|| IsLastKeyRepeating ()then
		DoBackSpace ()
		return
	endIf
	if GetWindowClass (GetCurrentWindow ()) == wcCodesEdit then
		refresh (0)
		PauseForMovement ()
		refresh (0)
	endIf
	if not PriorCharacter ()
	&& not GetCharacterValue (GetCharacter ()) == 29 then
		let iIsTop = 1
	endIf
	if DecipherItemOfText () == 0 then
		if not IIsTop then
			TypeKey(cksDelete)
		endIf
		return
	endIf
	let MotionFlag = 1
	SayTheCharacter ()
	PauseForMovement ()
	if StringLength (GetFromStartOfLine ()) == 1 then
		let iViewToggle = 1
	endIf
	if iIsTop then
		return
	else let MotionFlag = 1
		TypeKey(cksDelete)
	endIf

endScript


Script JAWSDelete ()

	if not IsDocumentWindow (GetCurrentWindow ())
	&& not (GetWindowClass (GetCurrentWindow ()) == wcCodesEdit) then
		PerformScript JAWSDelete ()
		return
	endIf
	let iSuppressSameSpot = 1
	TypeKey (cks4)
	PauseForMovement ()
	if nHighlight then
		Highlighting()
	endIf
	if DecipherItemOfText () == 0 then
		return
	endIf
	SayTheCharacter ()
	let iSuppressSameSpot = 0

EndScript



Script CutToClipboard ()

	if not IsDocumentWindow (GetCurrentWindow ())
	&& not (GetWindowClass (GetCurrentWindow ()) == wcCodesEdit) then
		PerformScript CutToClipboard()
		return
	endIf
	PerformScript CutToClipboard ()
	if nHighlight then
		Highlighting()
	endIf

EndScript


Script BrailleRouting ()

;	if GetJcfOption (OPT_BRL_MODE) == 2 then
;		SetJcfOption (OPT_BRL_MODE, BrailleModeSave)
;	endIf
	SaveOldValues ()
	PerformScript BrailleRouting()
	if 1 /*GetJCFOption (OPT_BRL_MOVE_ACTIVE_CURSOR) == 1*/ then
		let MotionFlag = 1
	endIf
	if IsDocumentWindow (GetCurrentWindow ()) then
		PauseForMovement ()
		GetNewValues ()
		PerformScript SayCharacter()
		let MotionFlag = 1
	endIf

EndScript


Int Function IsDocumentWindow (handle hwnd)

	if GetWindowTypeCode (hwnd) == WT_MULTILINE_EDIT
	&& IsPCCursor ()
	&& GetWindowClass (hwnd) == wcEditor
		&& not DialogActive () 
	&& not MenusActive ()
		&& not DialogActive () 
	&& not MenusActive ()
	|| (UserBufferIsActive ()
	&& GetWindowType (GetFocus ()) == "Edit") then
		return 1
	else
		return 0
	endIf

EndFunction


Int Function IsDocumentWindowJAWSCursor (handle hwnd)

;var
;	string sWindowName

	if GetWindowTypeCode (hwnd) == 66
		&& IsJAWSCursor () then
		return 1
	else
		return 0
	endIf

EndFunction


int Function IsBrailleDocumentWindow (handle hwnd)

;var
;	string sWindowName

	let sWindowName = GetWindowName (GetRealWindow (hwnd))
	if StringContains (sWindowName, scBrailleDocument)
	|| StringContains (sWindowName, scBrailleDocumentNew) then
;		&& IsPCCursor () 	then
		return 1
	else
		return 0
	endIf

EndFunction


Script ChangeCase ()

var
	string buffer

	TypeCurrentScriptKey ()
	PauseForMovement ()
	PauseForMovement ()
	let buffer = GetSelectedText ()
	StopSpeech ()
	if not StringIsBlank (buffer) then
;OT_CHAR is marked for FS internal use, but we want caps indication
		SayFormattedMessage (OT_CHAR, buffer, buffer)
	else
		SpellWord ()
	endIf

EndScript


Void Function SelectNextCharacter ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
		SelectNextCharacter()
		return
	endIf
	let nSelectCharacter = 1
	if GetJFWVersion () >= 1400000 then
		SayCharacterUnit (2)
	endIf
	if nHighlight then
		NextCharacter ()
	else SelectNextCharacter ()
	endIf

EndFunction


Void Function SelectPriorCharacter ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
		SelectPriorCharacter()
		return
	endIf
	let nSelectCharacter = 1
	if nHighlight then
		PriorCharacter ()
	else SelectPriorCharacter ()
	endIf
	if GetJFWVersion () >= 1400000 then
		SayCharacterUnit (3)
	endIf

EndFunction


Void Function SelectNextWord ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
		SelectNextWord()
		return
	endIf
	let nSelectWord = 1
	if GetJFWVersion () >= 1400000 then
		SayWordUnit (32)
	endIf
	let MotionFlag = 1
	SaveOldValues ()
	if nHighlight then
		NextWord ()
	else SelectNextWord ()
	endIf
	PauseForMovement ()
	GetNewValues ()

EndFunction


Void Function SelectPriorWord ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
		SelectPriorWord()
		return
	endIf
	SaveOldValues ()
	let SaySkipFlag = 1
	let nSelectWord = 1
	if nHighlight then
		PriorWord ()
	else SelectPriorWord ()
	endIf
	if GetJFWVersion () >= 1400000 then
		SayWordUnit (33)
	endIf

EndFunction


Void Function SelectNextLine ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
		SelectNextLine()
		return
	endIf
	if GetJFWVersion () >= 1400000 then
		BrailleLine ()
		SayLineUnit (34)
	endIf
	let nSelectLine = 1
	if nHighlight then
		NextLine ()
	else SelectNextLine ()
	endIf

endFunction


Void Function SelectPriorLine ()

	if not IsDocumentWindow (GetCurrentWindow ()) then
		SelectPriorLine()
		return
	endIf
		let nSelectLine = 1
	if nHighlight then
		PriorLine ()
	else SelectPriorLine ()
	endIf
	if GetJFWVersion () >= 1400000 then
		BrailleLine ()
		SayLineUnit (35)
	endIf

EndFunction


Void Function SayTextSelection (string Select, string sText, int bUseOTChar)

	if nSelectCharacter then
		Say(Select, OT_SELECT)
	elif nSelectWord then
		Say(Select, OT_SELECT)
	elif nSelectLine then
		Say(Select, OT_SELECT)
		let nSelectCharacter = 0
		let nSelectWord = 0
		let nSelectLine = 0
	else SayTextSelection (Select, sText, bUseOTChar)
	endIf

EndFunction


Void Function Highlighting ()

var
	int nCount
		
; The program shows highlighting in the Keys field of its status line (see CheckHighlight).
	if TRUE then
		CheckHighlight ()
		return
	endIf
	let nCount = 0
		let nOldHighlight = nHighlight
	if nNewDoc then
		nHighlight = 0
		nNewDoc = 0
		return
	endIf
		/*SaveCursor ()
		InvisibleCursor ()
		RouteInvisibleToPc ()
		JawsPageDown ()
		if GetControlID (GetCurrentWindow ()) != Editor_Control then
			PriorLine ()
			let sLine = GetLine ()
			let nKeysPos = StringContains (sLine, scKeys)
			if SubString (sLine, nKeysPos+6, 1) == "H" then
				SayMessage (OT_STATUS, msgHighlightingOn, msgHighlightingOn) 
			else
				SayMessage (OT_STATUS, msgHighlightingOff, msgHighlightingOff)
			endIf
			if nHighlight && not nOldHighlight then
				SayMessage (OT_SCREEN_MESSAGE, msgHighlightingOn)
			elif not nHighlight && nOldHighlight then
				SayMessage (OT_SCREEN_MESSAGE, msgHighlightingOff)
				return
			endIf
		endIf*/
	RouteInvisibleToPc ()
	SaveCursor ()
	PCCursor ()
	let nStartRow = GetCursorRow ()
	let nStartCol = GetCursorCol ()
	SpeechOff ()
PauseForMovement ()
	while nCount < 4
; 10-25-2016: fixed grouping below
	&& ((GetFromStartOfLine () == ""
	|| GetCursorCol () == nStartCol)
	&& GetCursorRow () == nStartRow)
		NextCharacter ()
		let nCount = nCount+1
		PauseForMovement ()
	endWhile
	if nCount < 2 then
		if GetSelectedText () then
			let nHighlight = 1
		else let nHighlight = 0
		endIf
		PriorCharacter ()
	else let nCount = 0
		while nCount < 4
; 10-25-2016: fixed grouping below
		&& ((GetToEndOfLine () == ""
		|| GetCursorCol () == nStartCol)
		&& GetCursorRow () == nStartRow)
;			PauseForMovement ()
			PriorCharacter ()
			let nCount = nCount+1
			PauseForMovement ()
		endWhile
		if GetSelectedText () then
					let nHighlight = 1
		else let nHighlight = 0
		endIf
		NextCharacter ()
	endIf
	InvisibleCursor ()
	RestoreCursor ()
	RoutePCToInvisible ()
	PCCursor ()
	AppSpeechOn ()
	if nHighlight /*&& not nOldHighlight*/ then
		SayMessage (OT_SCREEN_MESSAGE, msgHighlightingOn)
	elif not nHighlight && nOldHighlight then
		SayMessage (OT_SCREEN_MESSAGE, msgHighlightingOff)
	endIf

EndFunction


void function ProcessKeyPressed(int nKey, string strKeyName, int nIsBrailleKey, int nIsScriptKey)
;This function can be used to specify behavior in KeyPressedEvent which does not belong in any of the other functions called by that event.
;Currently, the other function are:
;PreProcessKeyPressedEvent, intended for all code which must be processed before any other
;ProcessHJDialogKeyPressed, handles all special processing for HJ dialogs
;ProcessSpaceBarKeyPressed, intended to be overwritten for any behavior tied to the spacebar
	;SayString ("mushroom")
	let nOldHighlight = nHighlight
	/*if not StringContains (strKeyName, "Arrow") then
		UIARefresh (0, 1)
	endIf*/
	;SayString (strKeyName)
	
	if StringContains (strKeyName, "Alt+F3") then
		let iViewToggle = 2
	elIf iViewToggle > 0 then
		let iViewToggle = iViewToggle-1
	endIf
	if StringContains (strKeyName, "Control+H")
&& not StringContains (strKeyName, "Home") then
		PauseForMovement ()
		highlighting () 
	endIf 
	
endFunction


Void Function ScreenStabilizedEvent (handle LastScreenWrite)

	;SayString ("goody")
	if nSpecialSayAllInProgress then
		if nLineCount == nOldLineCount then
			;SayString ("hold on")
		endIf
	endIf
		if iViewToggle then
			let iViewToggle = 0
		endIf
let nOldLastStableTick = nLastStableTick
	let nLastStableTick = GetTickCount ()
	;SayInteger (nLastStableTick)
	if nCosmoStick
	&& IsDocumentWindow (LastScreenWrite)
	&& nLastMovementTick < nOldLastStableTick
	&& not nFocusChange
	&& nKeyPressTime == nOldKeyPressTime then
		Scroll ()
	endIf
	/*if nSpecialSayAllInProgress then
		NextLine ()
		PerformScript SpecialSayAll ()
		return
	endIf*/
	;CheckHighlight ()
	if (GetWindowName (LastScreenWrite) == wnSuggestedReplacements
	|| GetWindowName (GetParent (GetCurrentWindow ())) == wnSpellCheck)
	&& GetWindowName (GetCurrentWindow ()) == wnOK then
		if nFocusChange == 2 then
			let nFocusChange = 1
		else PerformScript SpeakSpellCheckWord ()
			SayWindowTypeAndText (GetCurrentWindow ())
			let nFocusChange = 1 
			return
		endIf 
	endIf
	if GetWindowName(GetRealWindow (LastScreenWrite)) == sOldWindowName
	&& nFocusChange == 1 then
		let nFocusChange = -2
	elif nFocusChange < 0 then
		let nFocusChange = nFocusChange +1
	endIf
		
EndFunction


Void Function IndicateComputerBraille ()

; Overrides the default function, to avoid having JAWS
	;say "computer braille" wen focus changes.

EndFunction


Script SayLinePosition ()

Var
	int nAgain,
	int nFullLength,
	int nPosition,
	int nToEnd

	Let nAgain = IsSameScript ()
	let nFullLength = StringLength (GetLine ())
	let nPosition = StringToInt (GetColumnNum ())
	Let nToEnd = nFullLength - nPosition+1
	If nAgain == 0 Then
		PauseForMovement ()
		if nToEnd == 0 then
			Say (msgLinePosition+IntToString (nPosition)+" "+msgEndOfLine, FALSE)
		else
			Say (msgLinePosition+" "+IntToString (nPosition), FALSE)
		endIf
	ElIf nAgain == 1 Then
		;Say (IntToString (nFullLength-nPosition+1)+msgLocalSpace+msgToEndOfLine, OT_TEXT, FALSE)
		Say (IntToString (nToEnd-1)+" "+msgToEndOfLine, FALSE)
	endIf

endScript


Script SayTotalAndLeftOffset ()

var
	int nAgain,
	int nFullLength,
	int nTextLength
	if not IsDocumentWindow (GetCurrentWindow ()) then
		SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
		return
	elif GetLineNum() == -1 then
		SayMessage (OT_STATUS, msgCodesOn_l, msgCodesOn_s)
		return
	endIf
	Let nAgain = IsSameScript ()
	let nFullLength = StringLength (GetLine ())
	let nTextLength = StringLength (StringTrimLeadingBlanks (GetLine ()))
	if nAgain == 0 then
		PauseForMovement ()
		Say (IntToString (nFullLength)+" "+msgTotalCharacters, FALSE)
	elif nAgain == 1 then
		Say (IntToString (nFullLength - nTextLength)+" "+msgLeftSpaces, FALSE)
	endIf

EndScript


Script SpeakLastWord ()			

	if not IsDocumentWindow (GetCurrentWindow ()) then
		SayUsingVoice (VCTX_MESSAGE, msgNotInDocument, OT_USER_REQUESTED_INFORMATION)
		return
	endIf
	if GetPageNum () == -1 then
		SayMessage (OT_STATUS, msgNoLocation_l, msgNoLocation_s)
		return
	endIf
	if nBrailleEdit then
		SpeakStringAsBrailleCells (VCTX_JAWSCURSOR, StringSegment (GetLine (), " -", -1))
	else
		SayUsingVoice (VCTX_PCCURSOR, StringSegment (GetLine (), " -", -1), OT_LINE)
	endIf

EndScript


Void Function SaveOldValues ()

	let SaySkipFlag = 0
	let sOldStyleName = sStyleName
	let sOldPageNum = sPageNum
	let OldLineNum = LineNum
	/*let sOldRowNum = sRowNum
	let sOldColNum = sColNum*/

EndFunction


 Function GetNewValues ()

var
	int x,
	int i

	let sPageNum = GetPageNum ()
	let LineNum = GetLineNum ()
	let sStyleName = GetStyleName ()
	if sPageNum != sOldPageNum
	&& sPageNum != -1
; below, don't say word "page" when entering a code or character code
	&& not DialogActive () then
		SayUsingVoice (VCTX_MESSAGE, msgPage+sPageNum, OT_STATUS)
	endIf
	let LineNum = GetLineNum ()
	let x = Max ((LineNum-OldLineNum), (OldLineNum-LineNum))
	if x > 1
	&& SaySkipFlag
	&& sPageNum == sOldPageNum
	&& gbSkipVerbosity then
		SayUsingVoice (VCTX_MESSAGE, msgLineSkip, OT_STATUS)
	endIf
	if gbStyleChangeVerbosity
	;&& MotionFlag
	&& sStyleName != sOldStyleName
	&& sStyleName != "-1" then
		SayUsingVoice (VCTX_MESSAGE, GetStyleName (), OT_STATUS)
	endIf
	if MotionFlag != -1 then
		let MotionFlag = 0
	endIf
	if nBrailleEdit
	&& BrailleInUse ()
	&& x >= 1
	&& giBrailleLineSpeakingLevel == 2 then
	;|| giBrailleDocInputEcho >= 3) then
		PauseForMovement ()
		PauseForMovement ()
		let sTransLine = GetWindowText(hndTransLine, FALSE)
	endIf

EndFunction


Function OldGetNewValues ()

var
	int x

	let sPageNum = GetPageNum ()
	let LineNum = GetLineNum ()
	let sStyleName = GetStyleName ()
	if sPageNum != sOldPageNum
	&& sPageNum != -1
; below, don't say word "page" when entering a code or character code
	&& not DialogActive () then
		SayUsingVoice (VCTX_MESSAGE, msgPage+sPageNum, OT_STATUS)
	endIf
	let LineNum = GetLineNum ()
	let x = Max ((LineNum-OldLineNum), (OldLineNum-LineNum))
	if x > 1
	&& SaySkipFlag
	&& sPageNum == sOldPageNum
	&& gbSkipVerbosity then
		SayUsingVoice (VCTX_MESSAGE, msgLineSkip, OT_STATUS)
	endIf
	if gbStyleChangeVerbosity
	;&& MotionFlag
	&& sStyleName != sOldStyleName
	&& sStyleName != "-1" then
		SayUsingVoice (VCTX_MESSAGE, GetStyleName (), OT_STATUS)
	endIf
	if MotionFlag != -1 then
		let MotionFlag = 0
	endIf

EndFunction


Script SpeakSpellCheckWord ()

	if sWindowName != wnSpellCheck then
		return
	EndIf
	SaveCursor ()
	InvisibleCursor ()
	MoveToWindow (SpellCheckWindow)
	JAWSPageUp ()
	NextLine ()
	NextLine ()
	SayLine ()
	JAWSEnd ()
	;SayWord ()
	SayMessage (OT_JAWS_MESSAGE, msgSpelled, msgSilent)
		SpellString (GetWord ())
	BrailleMessage (GetWord (), 0, 5000)
	RestoreCursor ()
		PcCursor ()

EndScript

 
Script ReadWordInContext ()

	MoveToWindow (GetParent (SpellCheckWindow))
	PerformScript ReadWordInContext ()
	if BrailleInUse () then
		RouteBrailleToJAWS ()
		if IsKeyWaiting () then
			RouteBrailleToPC ()
		endIf
	endIf		

EndScript


Void Function LoadNonJCFOptions ()

	let giBrailleLineSpeakingLevel = GetNonJCFOption ("BrailleLineSpeakingLevel")-1
	let giBrailleDocInputEcho = GetNonJCFOption ("BrailleDocInputEcho")-1
	let gbAnnounceBrailleCharsAsLocalBrailleSigns = GetNonJCFOption ("SpeakBrailleCharsAsLocalBrailleSigns")
	let gbStyleChangeVerbosity = GetNonJCFOption ("AnnounceStyleChanges")
	let gbSkipVerbosity = GetNonJCFOption ("AnnounceForcedLineSkips")
	let gbShowRequestedStatusInfoOnBrailleDisplay = GetNonJCFOption ("ShowRequestedStatusInfoOnBrailleDisplay")
	SetJCFOption(OPT_BRL_MESSAGES, gbShowRequestedStatusInfoOnBrailleDisplay)
	let gbSpeakPrintLinesByWord = GetNonJCFOption("SpeakPrintLinesByWord")
		let gbUseStatusLineForUnicodeValues = GetNonJCFOption("UseStatusLineForUnicodeValues")
;SayString ("giBrailleDocInputEcho is "+IntToString(giBrailleDocInputEcho))

EndFunction


Function QuickSettings ()
let MotionFlag = 1
if getRunningFSProducts () == product_MAGic then
	;do nothing if MAGic-specific limitations are not met:
;	if  ! gbSupportsMAGicOptions && ! isVirtualPcCursor () && ! IsFormsModeActive () then
;		return
;	endIf
endIf
If !AreUtilitiesAllowed() then
	return
endIf
if InHJDialog () then
	SayFormattedMessage (OT_error, cMSG337_L, cMSG337_S)
	return
endIf
QuickSettingsPreProcess ()
LaunchQuickSettings ()
delay (2, TRUE)
loadNonJCFOptions (); update application-wide options.
SetUpStuffForNewPage (TRUE)

	handle_custom_windows (GetCurrentWindow (), GetCurrentWindow ())
QuickSettingsPostProcess ()
EndFunction


void function QuickSettingsPreProcess ()
;Code goes here which runs before loading the Quick Settings tool.
let giOldBrailleDocInputEcho = giBrailleDocInputEcho
let giBrailleDocInputEcho = giBrailleDocInputEcho
let gbShowRequestedStatusInfoOnBrailleDisplay = gbOldShowRequestedStatusInfoOnBrailleDisplay

endFunction


Void Function QuickSettingsPostProcess ()
	SetJCFOption(OPT_BRL_MESSAGES, gbShowRequestedStatusInfoOnBrailleDisplay)
	SetDefaultJCFOption(OPT_BRL_MESSAGES, gbShowRequestedStatusInfoOnBrailleDisplay)
		if giBrailleDocInputEcho != giOldBrailleDocInputEcho
 	&& nBrailleEdit then
		HandleCustomWindows (GetFocus ())
	endIf
		
EndFunction


String Function GetLastBrailleInput ()

var
	string sWord
	
	if GetCharacterValue (GetPriorCharacter ()) != 0 then
		return GetPriorCharacter ()
	else let sWord = GetWord ()
		return (Substring (sWord, StringLength(sWord), 1))
	endIf

EndFunction


Void Function CheckHighlight ()

var
	string sLine

	let nOldHighlight = nHighlight
	;PauseForMovement ()
	SaveCursor ()
  	InvisibleCursor ()
	RouteInvisibleToPc ()
	SetRestriction (RestrictRealWindow)
	JawsPageDown ()
	if GetControlID (GetCurrentWindow ()) != Editor_Control then
		PriorLine ()
	endIf
	let sLine = GetLine ()
	let nKeysPos = StringContains (sLine, scKeys)
	if SubString (sLine, nKeysPos+6, 1) == "h" then
		let nHighlight = 1
	else
		let nHighlight = 0
	endIf
	PauseForMovement ()
	if nHighlight && not nOldHighlight then
		SayMessage (OT_SCREEN_MESSAGE, msgHighlightingOn)
	elif not nHighlight && nOldHighlight then
		SayMessage (OT_SCREEN_MESSAGE, msgHighlightingOff)
	endIf 

EndFunction


/*
void function ProcessKeyPressed(int nKey, string strKeyName, int nIsBrailleKey, int nIsScriptKey)

	;SayString ("pumpkin")
	let sKeyName = strKeyName
;This function can be used to specify behavior in KeyPressedEvent which does not belong in any of the other functions called by that event.
;Currently, the other function are:
;PreProcessKeyPressedEvent, intended for all code which must be processed before any other
;ProcessHJDialogKeyPressed, handles all special processing for HJ dialogs
;ProcessSpaceBarKeyPressed, intended to be overwritten for any behavior tied to the spacebar
	let nOldHighlight = nHighlight
	;if not StringContains (strKeyName, "Arrow") then
		;UIARefresh (0, 1)
	;endIf
	if StringContains (strKeyName, "Control+H") then
		highlighting () 
	endIf 
	
endFunction
*/


Function ProcessEventOnFocusChangedEvent (handle AppWindow, handle RealWindow, string RealWindowName, handle FocusWindow, handle PrevWindow)

	if nStartingApp then
		ProcessSayAppWindowOnFocusChange(AppWindow, FocusWindow)
		let nStartingApp = 0
	elif IsDocumentWindow (FocusWindow) then
		ProcessSayRealWindowOnFocusChange(AppWindow,RealWindow,RealWindowName,FocusWindow)
	else ProcessEventOnFocusChangedEvent(AppWindow, RealWindow, RealWindowName, FocusWindow, PrevWindow)
endIf
	
	endFunction



Function IsCosmoConnected ()

var
	string sLine

	if nCosmoStick then
		return 1
	endIf
	SaveCursor ()
	InvisibleCursor ()
	RouteInvisibleToPc ()
	JawsPageDown ()
	if GetControlID (GetCurrentWindow ()) != Editor_Control then
		PriorLine ()
	EndIf	
	let sLine = GetLine ()
	let nKeysPos = StringContains (sLine, scKeys)
	if SubString (sLine, nKeysPos+6, 1) == "+" then
		let nCosmoStick = 1
	else let nCosmoStick = 0
	endIf
	return nCosmoStick
	
EndFunction


Script CheckCosmo ()

var
	int nAgain

	let nAgain = IsSameScript ()
	PauseForMovement ()
	PauseForMovement ()
	if nAgain == 0 then
		IsCosmoConnected ()
	elif nAgain == 1 then
		let nCosmoStick = 1
	elif nAgain == 2 then
		let nCosmoStick = 0
	endIf
	if nCosmoStick then
		Say (msgCosmoConnected, OT_USER_REQUESTED_INFORMATION)
	else Say (msgCosmoNotConnected, OT_USER_REQUESTED_INFORMATION)
	endIf

EndScript


Void Function Scroll ()

	if iViewToggle then return endIf
	let nScrollDown = 0
	let nScrollUp = 0
SaveCursor ()
	;RouteInvisibleToPC ()
	InvisibleCursor ()
	RouteInvisibleToPC ()
	NextLine ()
	if CheckTransLine () then
		NextLine ()
	endIf
	if StringContains (GetLine (), scCurrentStyle) then
		let nScrollDown = 1
	endIf
	PCCursor ()
	RestoreCursor ()
	if not nSpecialSayAllInProgress then
		InvisibleCursor ()
		RouteInvisibleToPC ()
		PriorLine ()
		if StringContains (GetWindowClass (GetWindowAtPoint (GetCursorCol (), GetCursorRow ())), scMDI) then
			let nScrollUp = 1
		endIf
	PCCursor ()
		RestoreCursor ()
	endIf
	if nScrollDown then
			AppSpeechOff ()
			if NextLine () then
				AppSpeechOn ()
				Say (msgScrollingDown, OT_SAYALL, FALSE)
				AppSpeechOff ()
			NextLine ()
			NextLine ()
			PriorLine ()
			PriorLine ()
			PriorLine ()
			PriorLine ()
			AppSpeechOn ()
			NextLine ()
		endIf
			if not nSpecialSayAllInProgress then
				SpeakLineOfText ()
			else
					PriorLine ()
					let nLastMovementTick = GetTickCount ()
					while GetTickCount () == nLastMovementTick
					Delay (1000/GetVoiceRate (VCTX_PCCURSOR, FALSE))
				endWhile
				NextLine ()
				return
			endIf
		let iScrolled = 2
	elif nScrollUp then
		Say (msgScrollingUp, OT_JAWS_MESSAGE, FALSE)
		AppSpeechOff ()
		PriorLine ()
		PriorLine ()
		PriorLine ()
		NextLine ()
		NextLine ()
		NextLine ()
		NextLine ()
		AppSpeechOn ()
		PriorLine ()
		SpeakLineOfText ()
		let iScrolled = 2
	endIf

EndFunction


Script CloseDocumentWindow ()

	let nFocusChange = 1
	PerformScript CloseDocumentWindow()
	/*if IsDocumentWindow (GetCurrentWindow ()) then
		HandleCustomWindows (GetCurrentWindow ())
	endIf*/

EndScript


Script TranslateDocument ()

	if IsDocumentWindow (GetCurrentWindow ()) then
		let nFocusChange = 1
		TypeKey ("Control+T")
		HandleCustomWindows (GetCurrentWindow ())
	endIf

EndScript



Script Tab ()

	PerformScript Tab()
	PauseForMovement ()
	if InTable then
		let MotionFlag = 1
		SpeakTableEntry ()
	endIf

EndScript


Script ShiftTab ()

	PerformScript ShiftTab()
		PauseForMovement ()
if InTable then
		let MotionFlag = 1
		SpeakTableEntry ()
		let MotionFlag = 0
	endIf

EndScript


Void Function SpeakTableEntry ()

var
	string buffer,
	int iColNum,
	string sColSep,
	int iColSep,
	int i,
	int iPageNum,
	int iColTotal,
	string sPN,
	string sLastDigit

	GetNewValues ()
	;Pause ()
	let iTableEntry = 1
	let iColNum = StringToInt (sColNum)
	if nBrailleEdit then
		PerformScript TransLineCursorToggle ()
		pause ()
endIf
	if sPageNum == -1 then
		let sColSep = "[>]"
		let buffer = StringDeleteBlankFields (StringReplaceSubstrings (StringReplaceSubstrings(GetLine (), "[<]", "~"), sColSep, "~"), "~")
	elIf StringContains (sStyleName, msgLinear) then
		let buffer = StringDeleteBlankFields (StringReplaceSubstrings (StringReplaceSubstrings (GetLine (), "; ", "~"), ": ", "~"), "~")
	elIf StringContains (sStyleName, msgListed) then
		if BrailleInUse () then
			BrailleRefresh (0)
		endIf
		SayUsingVoice (VCTX_PCCursor, GetLine (), OT_STRING)
		let buffer = ""
	else let buffer = StringDeleteBlankFields (StringReplaceSubstrings (StringReplaceSubstrings (StringReplaceSubstrings (StringReplaceSubstrings (GetLine (), " ..", "~"), "~.", "~"), "..", "~"), "  ", "~"), "~")
	endIf
	If StringContains (sStyleName, msgTwoPage)
	&& sPageNum != -1 then
		let sPN = StringTrimTrailingBlanks (sPageNum)
		let sLastDigit = SubString (sPN, StringLength(sPN), 1)
		if StringContainsChars (sLastDigit, "13579") then
			;SayString ("mama mia")
			let iColTotal = StringToInt (sColTotal)
			let iColNum = StringToInt(sColNum)-iColTotal-1
		endIf
	endIf
		SayUsingVoice (VCTX_MESSAGE, StringSegment (buffer, "~", iColNum), OT_STRING)
	if nBrailleEdit then
			PerformScript TransLineCursorToggle()
		endIf
		let iTableEntry = 0

EndFunction


Script Enter ()

	PerformScript Enter ()
	if sWindowName == wnSpellCheck then
		PerformScript SpeakSpellCheckWord ()
	endIf

EndScript


Void Function SayDocChar (string VCTX, string sChar)

	if nBrailleEdit then
		SayBrailleChar (VCTX, sChar)
	else SayUsingVoice (VCTX, sChar, OT_STRING)
	endIf

EndFunction


Void Function SayDocPriorChar ()

	if nBrailleEdit then
		SayUsingVoice (VCTX_KEYBOARD, GetLastBrailleInput (), OT_KEYBOARD)
	else SayUsingVoice (VCTX_KEYBOARD, sPriorChar, OT_KEYBOARD)
	endIf

EndFunction


Script BraillePanLeft ()

var
	int iActiveCursor

	if GetJCFOption (OPT_BRL_MOVE_ACTIVE_CURSOR) == 1 then
		let MotionFlag = 1
		PerformScript BraillePanLeft ()
	else let iActiveCursor = GetActiveCursor ()
		BrailleCursor ()
		PerformScript BraillePanLeft()
		SetActiveCursor (iActiveCursor)
	endIf

EndScript


Script BraillePanRight ()

var
	int iActiveCursor

	if GetJCFOption (OPT_BRL_MOVE_ACTIVE_CURSOR) == 1 then
		let MotionFlag = 1
		PerformScript BraillePanRight()
	else let iActiveCursor = GetActiveCursor ()
		BrailleCursor ()
		PerformScript BraillePanRight()
		SetActiveCursor (iActiveCursor)
	endIf

EndScript


Script BrailleMoveLeft ()

var
	int iActiveCursor

	if GetJCFOption (OPT_BRL_MOVE_ACTIVE_CURSOR) == 1 then
		let MotionFlag = 1
		PerformScript BrailleMoveLeft ()
	else BrailleCursor ()
		PerformScript BrailleMoveLeft()
		SetActiveCursor (iActiveCursor)
	endIf

EndScript


Script BrailleMoveRight ()

var
	int iActiveCursor

	if GetJCFOption (OPT_BRL_MOVE_ACTIVE_CURSOR) == 1 then
		let MotionFlag = 1
		PerformScript BrailleMoveRight ()
	else let iActiveCursor = GetActiveCursor ()
		BrailleCursor ()
		PerformScript BrailleMoveRight ()
		SetActiveCursor (iActiveCursor)
	endIf

EndScript


Script SpeakLineAsWords ()

	JAWSHome ()
	PerformScript SayWord ()
	NextWord ()
	while StringLength (StringTrimLeadingBlanks (GetFromStartOfLine ())) > 0 && not IsKeyWaiting ()
		PerformScript SayWord ()
		NextWord ()
		Delay (1)
	endWhile
	PriorCharacter ()
	PriorCharacter ()
	JAWSHome ()


	/*RouteInvisibleToPC ()
	InvisibleCursor ()
	JAWSHome ()
	SpeakWord ()
	NextWord ()
	While not StringIsBlank (GetToEndOfLine ())
		SpeakWord ()
		NextWord ()
	endWhile*/

EndScript

Void Function SpeakWord ()

var
	string charry

	let charry = GetCharacter ()
	if GetFont () == "Arial"
	&& GetCharacterValue (charry) == 91 then
		SayMessage (OT_TEXT, msgCode, msgCode)
		SpellString (StringSegment (GetToEndOfLine (), "]", 1))
		NextWord ()
	endIf

EndFunction

prototype string function GetLocaleNameForLangID(string langAbbrevOrLangIdStr);
prototype string function BrailleGetCurrentProfileName()

script BrailleSelectAProfile()

var
string profileList,
string profileListEntry,
string profileName,
string langIDList,
int index,
int count,
int curIndex

let langIDList=readSettingString (SECTION_BRAILLE, sHKey_PREFERRED_BRL_PROFILE_LIST, "", FT_CURRENT_JCF	)
let count=StringSegmentCount (langIDList, "|")
if count==0 then
	sayFormattedMessage(OT_ERROR, c1msgNoPreferredBrailleProfileListConfigured_L, c1msgNoPreferredBrailleProfileListConfigured_S)
	return
endIf

for index=1 to count
	let profileName = GetLocaleNameForLangID(StringSegment (langIDList, "|", index))
	if profileName == BrailleGetCurrentProfileName() then
		curIndex=index
		BrailleSetCurrentProfile (StringSegment (langIDList, "|", index), wdSession)
		return
	endIf
	profileListEntry=profileName+"|"
	profileList=profileList+profileListEntry
endFor
;let index=DlgSelectItemInList (profileList, cscBrlProfileListTitle, FALSE, curIndex)
;if index then
; Must load using langID.
	;BrailleSetCurrentProfile(StringSegment (langIDList, "|", index), wdSession)
;endIf
BrailleRefresh()
endScript


/*Void Function ProcessSpeechOnNewTextEvent(handle hFocus, handle hwnd, string buffer, int nAttributes,
	int nTextColor, int nBackgroundColor, int nEcho, string sFrameName)
 


endFunction*/


Int Function SayLineInsteadOfSayAll ()

var
	int i

		if nBrailleEdit then
		SpeakLineOfText ()
		return FALSE
	else SayLineInsteadOfSayAll ()
	return FALSE
	endIf

EndFunction


Script SpecialSayAll ()

	if nBrailleEdit
	&& not IsKeyWaiting () then
		let nSpecialSayAllInProgress = 1
		let nCosmoStick = 1
		let nLineCount = 0
		let nStableCount = 0
	endIf
	PerformScript SayAll ()
	let nCosmoStick = 0

EndScript

/*Script SpecialSayAll ()

	int nOldTickCount,
	int nNewTickCount

	;SayString ("pizza")
	if IsKeyWaiting () then
		let nSpecialSayAllInProgress = 0
		let nCosmoStick = nCosmoSave
		let nScrollDown = 0
		return
	endIf
	if nBrailleEdit then
		if not nSpecialSayAllInProgress then
			let nCosmoSave = nCosmoStick
			let nCosmoStick = 1
			let nScrollDown = 0
			let nSpecialSayAllInProgress = 1
		endIf
		SpeakLineOfText ()
		;NextLine ()
		PerformScript ChangeLine ()
		/*while not IsKeyWaiting ()
			let nOldX = GetCursorCol ()PerformScript SayAll ()
		endIf
			let nOldY = GetCursorRow ()			let nOldTickCount = GetTickCount ()
			;Delay (2)
			;PerformScript SayNextLine ()
			;NextLine ()
			;PauseForMovement ()
			;PerformScript ChangeLine ()
			let nX = GetCursorCol ()
			let nY = GetCursorRow ()
			let nNewTickCount = GetTickCount ()
			FocusPointMovedEventCosmo (nX, nY, nOldX, nOldY, 11, 0, nNewTickCount-nOldTickCount)
endWhile
else
	PerformScript SayAll ()
endIf

EndScript*/


Void Function SayAllStoppedEvent ()

	if nBrailleEdit then
		let nSpecialSayAllInProgress = 0
		let nCosmoStick = 0
	endIf
	SayAllStoppedEvent ()

EndFunction


HANDLE Function GetStatusLine ()

var
	handle hndStatusLine

; The status line is the window after the document's editing area; the translated line is the one after it.
	let hndStatusLine = GetNextWindow (GetCurrentWindow ())
	return hndStatusLine

EndFunction


String Function GetStatusLineText ()

; The whole status line: Current Style, Page:, Line:, Column:, the character at the caret, Keys:.
	if !hndStatusLine then
		let hndStatusLine = GetStatusLine ()
	endIf
	return GetWindowText (hndStatusLine, 0)

EndFunction


Function SpeakCompletedBrailleWord ()

var
	string s1,
	string s2,
	string s3,
	int i,
	int j,
	int j1,
	int iShorten,
	int n,
	int n1
	
	PauseForMovement ()
	let sTransLine = GetWindowText (hndTransLine, 0)
	SaveCursor ()
	InvisibleCursor ()
	MoveToWindow (hndTransLine)
	let s1 = StringReplaceSubstrings (sTransLine, " ", "\30")
	let s2 = StringReplaceSubstrings (sOldTransLine, " ", "\30")
	let j = StringLength (s2)
		let s3 = StringReplaceSubstrings (sOldOldTransLine, " ", "\30")

	StringTrimCommon (s2, s3, 2)
	let j1 = StringLength (s2)
	let iShorten = j-j1
	let n = StringLength (s1)
	let n1 = n-iShorten
	JAWSHome ()
	let i = 0
	while i < n1
		NextCharacter ()
		let i = i+1
	endWhile
	PriorCharacter ()
	if GetCharacter () == " "
	|| GetCharacter () == "\30" then
		PriorCharacter ()
	endIf
	let i = 0
	while GetCharacter () != " "
	&& i < n1
		let i = i+1
		PriorCharacter ()
	endWhile
 		Say (SubString (s1, n1-i, i+1), OT_KEYBOARD, FALSE)
	PCCursor ()
	RestoreCursor ()

EndFunction
 

Script NextRowEntry ()

	TypeKey ("Alt+DownArrow")
	if InTable then
		SpeakTableEntry ()
	else SpeakLineOfText ()
	endIf

EndScript

Script PriorRowEntry ()

	TypeKey ("Alt+UpArrow")
	if InTable then
		SpeakTableEntry ()
	else SpeakLineOfText ()
	endIf

EndScript

Script NextColumnEntry ()

	TypeKey ("Alt+RightArrow")
	if InTable then
		SpeakTableEntry ()
	endIf

EndScript

Script PriorColumnEntry ()

	TypeKey ("Alt+LeftArrow")
	if InTable then
		SpeakTableEntry ()
	endIf

EndScript


Script SayTableEntry ()

	if InTable then
		SpeakTableEntry ()
	endIf

EndScript


Script SpeakCompletedBrailleWordsToggle ()

	if giBrailleDocInputEcho == 2 then
		let giBrailleDocInputEcho = 4
		SayMessage (OT_MESSAGE, msgSpellCompletedBrailleWordsOn_l, msgOn)
	elif giBrailleDocInputEcho == 3 then
		let giBrailleDocInputEcho = 5
		SayMessage (OT_MESSAGE, msgSpellCompletedBrailleWordsOn_l, msgOn)
	elif giBrailleDocInputEcho == 4 then
		let giBrailleDocInputEcho = 2
		SayMessage (OT_MESSAGE, msgSpellCompletedBrailleWordsOff_l, msgOff)
	elif giBrailleDocInputEcho == 5 then
		let giBrailleDocInputEcho = 3
		SayMessage (OT_MESSAGE, msgSpellCompletedBrailleWordsOff_l, msgOff)
	endIf

EndScript






