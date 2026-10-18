#!/bin/sh
# Puts the BrailleCraft Orca script where Orca looks for it. Orca 45 to 50 search the orca-scripts package in the
# Orca preferences folder (~/.local/share/orca/orca-scripts/braillecraft-desktop) before their own scripts; Orca 51
# and later load approved user extensions from ~/.local/share/orca/extensions instead, so they get the extension
# package and its approval. Run it as the user who runs Orca, then restart Orca (Orca+Q, then start it again, or log
# out and in). Usage: sh install.sh   (from the folder that holds braillecraft-desktop and extension)
# or sh /opt/braillecraft/orca/install.sh
set -e
here=$(cd "$(dirname "$0")" && pwd)
data=${XDG_DATA_HOME:-$HOME/.local/share}
major=$(orca --version 2>/dev/null | grep -o -E '[0-9]+(\.[0-9]+)*' | head -1 | cut -d. -f1)
if [ -n "$major" ] && [ "$major" -ge 51 ]; then
  target=$data/orca/extensions/braillecraft
  mkdir -p "$target"
  cp "$here/extension/braillecraft/__init__.py" "$target/"
  rm -rf "$target/__pycache__"
  orca --approve-extension braillecraft || echo "Approve it in Orca: Preferences, User Extensions, BrailleCraft."
  echo "BrailleCraft Orca extension installed in $target and approved. Restart Orca to use it."
else
  target=$data/orca/orca-scripts
  mkdir -p "$target/braillecraft-desktop"
  touch "$target/__init__.py"
  cp "$here/braillecraft-desktop/__init__.py" "$here/braillecraft-desktop/script.py" "$target/braillecraft-desktop/"
  rm -rf "$target/braillecraft-desktop/__pycache__"
  echo "BrailleCraft Orca script installed in $target/braillecraft-desktop. Restart Orca to use it."
fi
