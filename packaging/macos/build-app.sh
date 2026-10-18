#!/bin/bash
# Builds the macOS application bundle and disk image of the native program from its folder (the CI's
# dist/BrailleCraft): BrailleCraft.app with the program in Contents/MacOS, its fonts, tables, templates and embosser
# models in Contents/Resources (the program finds them there), the program and document icons made from the
# iconsets, and a .dmg holding the bundle beside a link to Applications. The fonts stay inside the bundle: the
# program registers them for itself (main.rs), so nothing is installed system-wide. Runs on macOS (iconutil, hdiutil).
#
# Usage: packaging/macos/build-app.sh <program folder> <version> <output folder>   (from the repository root)
set -euo pipefail
dist=$(cd "$1" && pwd)
version=$2
mkdir -p "$3"
out=$(cd "$3" && pwd)
here=$(cd "$(dirname "$0")" && pwd)
icons=$(cd "$here/../icons" && pwd)
stage=$(mktemp -d)
app=$stage/BrailleCraft.app

mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
install -m 755 "$dist/braillecraft-desktop" "$app/Contents/MacOS/braillecraft-desktop"
# The command-line program (the MCP server for AI agents), beside the program so it finds the same data folders.
install -m 755 "$dist/braillecraft-cli" "$app/Contents/MacOS/braillecraft-cli"
for d in fonts tables templates embossers; do
  cp -R "$dist/$d" "$app/Contents/Resources/$d"
done
# The licences and notices (MIT or Apache-2.0, NOTICE).
for f in LICENSE-MIT LICENSE-APACHE NOTICE; do
  cp "$dist/$f" "$app/Contents/Resources/$f"
done
for name in braillecraft print-document braille-document; do
  iconutil -c icns "$icons/$name.iconset" -o "$app/Contents/Resources/$name.icns"
done
# The VoiceOver scripts, compiled, with the preferences file that binds them to Keyboard Commander keys
# (tools/voiceover/build.py); the scripts are run from the installed application in Applications.
python3 "$here/../../tools/voiceover/build.py" "$app/Contents/Resources/VoiceOver"
rm -r "$app/Contents/Resources/VoiceOver/src"

# Unsigned: an ad-hoc signature keeps Apple Silicon willing to run it; Gatekeeper still asks at first launch.
codesign --force --deep --sign - "$app"

mkdir -p "$stage/dmg"
cp -R "$app" "$stage/dmg/"
ln -s /Applications "$stage/dmg/Applications"
# The VoiceOver preferences file and its notes beside the application, for VoiceOver Utility, Import Preferences.
cp "$app/Contents/Resources/VoiceOver/BrailleCraft.voprefs" "$stage/dmg/BrailleCraft.voprefs"
cp "$app/Contents/Resources/VoiceOver/README.txt" "$stage/dmg/VoiceOver keys.txt"
hdiutil create -volname "BrailleCraft $version" -srcfolder "$stage/dmg" -ov -format UDZO \
  "$out/BrailleCraft-$version-$(uname -m).dmg"
rm -rf "$stage"
ls -la "$out"
