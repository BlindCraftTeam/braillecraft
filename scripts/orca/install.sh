#!/bin/sh
# Puts the BrailleCraft Orca script where Orca looks for it: the orca-scripts package in Orca's preferences folder
# (~/.local/share/orca/orca-scripts/braillecraft-desktop), which Orca 45 to 50 search before their own scripts.
# Run it as the user who runs Orca, then restart Orca (Orca+Q, then start it again, or log out and in).
# Usage: sh install.sh    (from the folder that holds braillecraft-desktop)   or   sh /opt/braillecraft/orca/install.sh
set -e
here=$(cd "$(dirname "$0")" && pwd)
data=${XDG_DATA_HOME:-$HOME/.local/share}
target=$data/orca/orca-scripts
mkdir -p "$target/braillecraft-desktop"
touch "$target/__init__.py"
cp "$here/braillecraft-desktop/__init__.py" "$here/braillecraft-desktop/script.py" "$target/braillecraft-desktop/"
rm -rf "$target/braillecraft-desktop/__pycache__"
echo "BrailleCraft Orca script installed in $target/braillecraft-desktop. Restart Orca to use it."
