# The Orca script for BrailleCraft (the Linux desktop program, whose accessibility application name is
# braillecraft-desktop). Orca loads the package named after the application from the orca-scripts package in its
# preferences folder (~/.local/share/orca/orca-scripts/braillecraft-desktop) on Orca 45 to 50; see README.md beside
# this folder for installing it.
from .script import Script

__all__ = ["Script"]
