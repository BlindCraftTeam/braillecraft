#!/bin/bash
# Builds the Flatpak bundle of the native program from its folder (the CI's dist/BrailleCraft): the program in
# /app/bin with the braillecraft command, its fonts, tables, templates and embosser models in /app/share/braillecraft
# (where the program looks beside itself), the desktop entry, icons, document types and AppStream data under the
# application ID. GTK 3, CUPS and the C library come from the GNOME runtime; what the runtime lacks is taken from the
# build system: the GLU library and the CUPS commands lp, lpstat and lpoptions that embossing runs. The program is
# not compiled again: the bundle holds the same build as the other Linux packages.
#
# Usage: packaging/linux/build-flatpak.sh <program folder> <version> <output folder>
# Needs flatpak, with org.gnome.Platform at $runtime_version installed (the CI installs it from Flathub).
set -euo pipefail
dist=$(realpath "$1")
version=$2
out=$(realpath -m "$3")
here=$(cd "$(dirname "$0")" && pwd)
icons=$(cd "$here/../icons/png" && pwd)
id=io.github.BlindCraftTeam.BrailleCraft
runtime_version=51
mkdir -p "$out"
stage=$(mktemp -d)
build=$stage/build
files=$build/files

flatpak build-init "$build" $id org.gnome.Platform org.gnome.Platform $runtime_version

install -d "$files/bin" "$files/lib" "$files/share/braillecraft" "$files/share/applications" \
  "$files/share/mime/packages" "$files/share/metainfo"
install -m 755 "$dist/braillecraft-desktop" "$files/bin/braillecraft-desktop"
install -m 755 "$dist/braillecraft-cli" "$files/bin/braillecraft-cli"
ln -s braillecraft-desktop "$files/bin/braillecraft"
for d in fonts tables templates embossers orca; do
  cp -r "$dist/$d" "$files/share/braillecraft/$d"
done
for f in LICENSE-MIT LICENSE-APACHE NOTICE; do
  install -m 644 "$dist/$f" "$files/share/braillecraft/$f"
done

# From the build system: GLU (the wxWidgets OpenGL canvas links it) and the CUPS commands (libcups is in the runtime).
glu=$(ldconfig -p | sed -n 's/^\s*libGLU\.so\.1 (libc6,x86-64) => //p' | head -n1)
cp -L "${glu:?libGLU.so.1 is not installed}" "$files/lib/libGLU.so.1"
for c in lp lpstat lpoptions; do
  install -m 755 "$(command -v $c || { echo "$c is not installed (cups-client)" >&2; exit 1; })" "$files/bin/$c"
done

# Exported files carry the application ID in their names, so the desktop entry, its icon and the document type icons
# are renamed to it.
sed "s/^Icon=braillecraft$/Icon=$id/" "$here/braillecraft.desktop" > "$files/share/applications/$id.desktop"
sed -e "s/name=\"application-x-braillecraft-/name=\"$id.application-x-braillecraft-/" \
  "$here/braillecraft-mime.xml" > "$files/share/mime/packages/$id.xml"
for size in 16 24 32 48 64 128 256 512; do
  d=$files/share/icons/hicolor/${size}x${size}
  install -d "$d/apps" "$d/mimetypes"
  install -m 644 "$icons/braillecraft-$size.png" "$d/apps/$id.png"
  install -m 644 "$icons/print-document-$size.png" "$d/mimetypes/$id.application-x-braillecraft-print.png"
  install -m 644 "$icons/braille-document-$size.png" "$d/mimetypes/$id.application-x-braillecraft-braille.png"
done
sed "s/@VERSION@/$version/; s/@DATE@/$(date -u +%Y-%m-%d)/" "$here/$id.metainfo.xml" > "$files/share/metainfo/$id.metainfo.xml"

# Permissions: the display, documents anywhere in the home folder, printers through CUPS, and every device for
# embossers on serial, parallel and USB ports.
flatpak build-finish "$build" --command=braillecraft-desktop \
  --socket=wayland --socket=fallback-x11 --share=ipc --device=all --socket=cups --filesystem=home

flatpak build-export "$stage/repo" "$build" stable
flatpak build-bundle "$stage/repo" "$out/BrailleCraft-$version-x86_64.flatpak" $id stable \
  --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
rm -rf "$stage"
ls -la "$out"
