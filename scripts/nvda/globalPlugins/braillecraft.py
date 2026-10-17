# BrailleCraft NVDA add-on: the part that runs whatever program is in front.
#
# It does two things. It tells NVDA that the program file braillecraft-desktop.exe is served by the app module
# braillecraft_desktop (the file's name holds a hyphen, which a Python module name cannot), and it adds the
# BrailleCraft page to NVDA's settings, where the choices the BrailleCraft JAWS scripts keep in JAWS Quick Settings
# are made for NVDA.

import appModuleHandler
import globalPluginHandler
import gui
from gui.settingsDialogs import NVDASettingsDialog

from appModules import braillecraft_desktop


class GlobalPlugin(globalPluginHandler.GlobalPlugin):
	def __init__(self):
		super().__init__()
		appModuleHandler.registerExecutableWithAppModule("braillecraft-desktop", "braillecraft_desktop")
		if braillecraft_desktop.SettingsPanel not in NVDASettingsDialog.categoryClasses:
			NVDASettingsDialog.categoryClasses.append(braillecraft_desktop.SettingsPanel)

	def terminate(self):
		try:
			NVDASettingsDialog.categoryClasses.remove(braillecraft_desktop.SettingsPanel)
		except ValueError:
			pass
		try:
			appModuleHandler.unregisterExecutable("braillecraft-desktop")
		except Exception:
			pass
		super().terminate()
