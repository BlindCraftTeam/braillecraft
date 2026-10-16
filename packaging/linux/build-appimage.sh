#!/bin/bash
# Builds the AppImage of the native program from its folder (the CI's dist/BrailleCraft): one file that runs on its
# own, with the program in usr/bin and its fonts, tables, templates and embosser models in usr/share/braillecraft
# (where the program looks beside itself), the desktop entry and the program icon at the top as the AppImage format
# asks. The program takes the braille fonts for itself (main.rs), so nothing is installed. GTK 3 and the C library
# come from the system, as on every desktop that has GTK 3.
#
# Usage: APPIMAGETOOL=<appimagetool> packaging/linux/build-appimage.sh <program folder> <version> <output folder>
set -euo pipefail
dist=$(realpath "$1")
version=$2
out=$(realpath -m "$3")
here=$(cd "$(dirname "$0")" && pwd)
icons=$(cd "$here/../icons/png" && pwd)
tool=${APPIMAGETOOL:?APPIMAGETOOL names the appimagetool program}
mkdir -p "$out"
stage=$(mktemp -d)
appdir=$stage/BrailleCraft.AppDir

install -d "$appdir/usr/bin" "$appdir/usr/share/braillecraft" "$appdir/usr/share/applications" \
  "$appdir/usr/share/icons/hicolor/256x256/apps"
install -m 755 "$dist/braillecraft-desktop" "$appdir/usr/bin/braillecraft-desktop"
for d in fonts tables templates embossers; do
  cp -r "$dist/$d" "$appdir/usr/share/braillecraft/$d"
done
# The licences and notices (MIT or Apache-2.0, NOTICE).
for f in LICENSE-MIT LICENSE-APACHE NOTICE; do
  install -m 644 "$dist/$f" "$appdir/usr/share/braillecraft/$f"
done
install -m 644 "$here/braillecraft.desktop" "$appdir/braillecraft.desktop"
install -m 644 "$here/braillecraft.desktop" "$appdir/usr/share/applications/braillecraft.desktop"
install -m 644 "$icons/braillecraft-256.png" "$appdir/braillecraft.png"
install -m 644 "$icons/braillecraft-256.png" "$appdir/usr/share/icons/hicolor/256x256/apps/braillecraft.png"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/braillecraft-desktop" "$@"
EOF
chmod 755 "$appdir/AppRun"

ARCH=x86_64 "$tool" --no-appstream "$appdir" "$out/BrailleCraft-$version-x86_64.AppImage"
rm -rf "$stage"
ls -la "$out"
