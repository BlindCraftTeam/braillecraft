#!/bin/bash
# Builds the AppImage of the native program from its folder (the CI's dist/BrailleMaker): one file that runs on its
# own, with the program in usr/bin and its fonts, tables, templates and embosser models in usr/share/braillemaker
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
appdir=$stage/BrailleMaker.AppDir

install -d "$appdir/usr/bin" "$appdir/usr/share/braillemaker" "$appdir/usr/share/applications" \
  "$appdir/usr/share/icons/hicolor/256x256/apps"
install -m 755 "$dist/braillemaker-desktop" "$appdir/usr/bin/braillemaker-desktop"
for d in fonts tables templates embossers; do
  cp -r "$dist/$d" "$appdir/usr/share/braillemaker/$d"
done
install -m 644 "$here/braillemaker.desktop" "$appdir/braillemaker.desktop"
install -m 644 "$here/braillemaker.desktop" "$appdir/usr/share/applications/braillemaker.desktop"
install -m 644 "$icons/braillemaker-256.png" "$appdir/braillemaker.png"
install -m 644 "$icons/braillemaker-256.png" "$appdir/usr/share/icons/hicolor/256x256/apps/braillemaker.png"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/braillemaker-desktop" "$@"
EOF
chmod 755 "$appdir/AppRun"

ARCH=x86_64 "$tool" --no-appstream "$appdir" "$out/BrailleMaker-$version-x86_64.AppImage"
rm -rf "$stage"
ls -la "$out"
