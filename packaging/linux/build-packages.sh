#!/bin/bash
# Builds the Linux packages of the native program from its folder (the CI's dist/BrailleCraft): a .deb and an .rpm
# laid out as the Windows installer is (re/spec/26-installer.md): the program and its tables, templates, embosser
# models, fonts and JAWS scripts in /opt/braillecraft (the program finds them beside itself); the braillecraft
# command; the desktop entry with Print and Emboss actions; the .dxp and .dxb document types with their icons; the
# braille fonts installed for every program. Linux desktop actions belong to the program, not to a document type, so
# Print is the /print of print documents.
#
# Usage: packaging/linux/build-packages.sh <program folder> <version> <output folder>   (from the repository root)
set -euo pipefail
dist=$(realpath "$1")
version=$2
out=$(realpath -m "$3")
here=$(cd "$(dirname "$0")" && pwd)
icons=$(cd "$here/../icons/png" && pwd)
mkdir -p "$out"
stage=$(mktemp -d)
root=$stage/root

install -d "$root/opt/braillecraft" "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/mime/packages" \
  "$root/usr/share/fonts/truetype/braillecraft"
cp -r "$dist/." "$root/opt/braillecraft/"
chmod 755 "$root/opt/braillecraft/braillecraft-desktop"
ln -s /opt/braillecraft/braillecraft-desktop "$root/usr/bin/braillecraft"
install -m 644 "$here/braillecraft.desktop" "$root/usr/share/applications/braillecraft.desktop"
install -m 644 "$here/braillecraft-mime.xml" "$root/usr/share/mime/packages/braillecraft.xml"
install -m 644 "$dist"/fonts/*.ttf "$root/usr/share/fonts/truetype/braillecraft/"
for size in 16 24 32 48 64 128 256 512; do
  d=$root/usr/share/icons/hicolor/${size}x${size}
  install -d "$d/apps" "$d/mimetypes"
  install -m 644 "$icons/braillecraft-$size.png" "$d/apps/braillecraft.png"
  install -m 644 "$icons/print-document-$size.png" "$d/mimetypes/application-x-braillecraft-print.png"
  install -m 644 "$icons/braille-document-$size.png" "$d/mimetypes/application-x-braillecraft-braille.png"
done

# .deb: the shared libraries the program needs, as dpkg-shlibdeps finds them; the mime, desktop and font caches are
# brought up to date by the system's own triggers.
mkdir -p "$stage/shlibs/debian"
printf 'Source: braillecraft\n\nPackage: braillecraft\nArchitecture: any\n' > "$stage/shlibs/debian/control"
depends=$(cd "$stage/shlibs" && dpkg-shlibdeps -O -e"$root/opt/braillecraft/braillecraft-desktop" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
size_kb=$(du -sk "$root" | cut -f1)
install -d "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<EOF
Package: braillecraft
Version: $version
Section: text
Priority: optional
Architecture: amd64
Installed-Size: $size_kb
Depends: $depends
Maintainer: BlindCraft <info@blindcraft.dev>
Description: Braille translation, formatting and embossing
 BrailleCraft translates print to braille and back, lays out braille pages,
 and prints and embosses documents.
EOF
dpkg-deb --root-owner-group --build "$root" "$out/braillecraft_${version}_amd64.deb"
rm -rf "$root/DEBIAN"

# .rpm: the same tree.
mkdir -p "$stage/rpm/SPECS"
cat > "$stage/rpm/SPECS/braillecraft.spec" <<EOF
Name: braillecraft
Version: $version
Release: 1
Summary: Braille translation, formatting and embossing
License: MIT OR Apache-2.0
Vendor: BlindCraft Team
Packager: BlindCraft <info@blindcraft.dev>
URL: https://github.com/jeremiahbeasley/braillecraft
BuildArch: x86_64
AutoReqProv: yes
%description
BrailleCraft translates print to braille and back, lays out braille pages, and prints and embosses documents.
%install
cp -a $root/. %{buildroot}/
%files
/opt/braillecraft
/usr/bin/braillecraft
/usr/share/applications/braillecraft.desktop
/usr/share/mime/packages/braillecraft.xml
/usr/share/fonts/truetype/braillecraft
/usr/share/icons/hicolor/*/apps/braillecraft.png
/usr/share/icons/hicolor/*/mimetypes/application-x-braillecraft-*.png
%post
update-mime-database /usr/share/mime >/dev/null 2>&1 || :
update-desktop-database /usr/share/applications >/dev/null 2>&1 || :
fc-cache -f /usr/share/fonts/truetype/braillecraft >/dev/null 2>&1 || :
%postun
update-mime-database /usr/share/mime >/dev/null 2>&1 || :
update-desktop-database /usr/share/applications >/dev/null 2>&1 || :
fc-cache -f >/dev/null 2>&1 || :
EOF
if command -v rpmbuild >/dev/null; then
  rpmbuild --define "_topdir $stage/rpm" --define "_rpmdir $stage/out" -bb "$stage/rpm/SPECS/braillecraft.spec"
  mv "$stage"/out/*/*.rpm "$out/"
else
  echo "rpmbuild is not installed here: no .rpm made"
fi
rm -rf "$stage"
ls -la "$out"
