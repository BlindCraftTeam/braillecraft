#!/bin/bash
# Builds the Linux packages of the native program from its folder (the CI's dist/BrailleMaker): a .deb and an .rpm
# laid out as the Windows installer is (re/spec/26-installer.md): the program and its tables, templates, embosser
# models, fonts and JAWS scripts in /opt/braillemaker (the program finds them beside itself); the braillemaker
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

install -d "$root/opt/braillemaker" "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/mime/packages" \
  "$root/usr/share/fonts/truetype/braillemaker"
cp -r "$dist/." "$root/opt/braillemaker/"
chmod 755 "$root/opt/braillemaker/braillemaker-desktop"
ln -s /opt/braillemaker/braillemaker-desktop "$root/usr/bin/braillemaker"
install -m 644 "$here/braillemaker.desktop" "$root/usr/share/applications/braillemaker.desktop"
install -m 644 "$here/braillemaker-mime.xml" "$root/usr/share/mime/packages/braillemaker.xml"
install -m 644 "$dist"/fonts/*.ttf "$root/usr/share/fonts/truetype/braillemaker/"
for size in 16 24 32 48 64 128 256 512; do
  d=$root/usr/share/icons/hicolor/${size}x${size}
  install -d "$d/apps" "$d/mimetypes"
  install -m 644 "$icons/braillemaker-$size.png" "$d/apps/braillemaker.png"
  install -m 644 "$icons/print-document-$size.png" "$d/mimetypes/application-x-braillemaker-print.png"
  install -m 644 "$icons/braille-document-$size.png" "$d/mimetypes/application-x-braillemaker-braille.png"
done

# .deb: the shared libraries the program needs, as dpkg-shlibdeps finds them; the mime, desktop and font caches are
# brought up to date by the system's own triggers.
mkdir -p "$stage/shlibs/debian"
printf 'Source: braillemaker\n\nPackage: braillemaker\nArchitecture: any\n' > "$stage/shlibs/debian/control"
depends=$(cd "$stage/shlibs" && dpkg-shlibdeps -O -e"$root/opt/braillemaker/braillemaker-desktop" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
size_kb=$(du -sk "$root" | cut -f1)
install -d "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<EOF
Package: braillemaker
Version: $version
Section: text
Priority: optional
Architecture: amd64
Installed-Size: $size_kb
Depends: $depends
Maintainer: Jeremiah Beasley <jbeasley@jb11.net>
Description: Braille translation, formatting and embossing
 BrailleMaker translates print to braille and back, lays out braille pages,
 and prints and embosses documents.
EOF
dpkg-deb --root-owner-group --build "$root" "$out/braillemaker_${version}_amd64.deb"
rm -rf "$root/DEBIAN"

# .rpm: the same tree.
mkdir -p "$stage/rpm/SPECS"
cat > "$stage/rpm/SPECS/braillemaker.spec" <<EOF
Name: braillemaker
Version: $version
Release: 1
Summary: Braille translation, formatting and embossing
License: Not yet chosen
BuildArch: x86_64
AutoReqProv: yes
%description
BrailleMaker translates print to braille and back, lays out braille pages, and prints and embosses documents.
%install
cp -a $root/. %{buildroot}/
%files
/opt/braillemaker
/usr/bin/braillemaker
/usr/share/applications/braillemaker.desktop
/usr/share/mime/packages/braillemaker.xml
/usr/share/fonts/truetype/braillemaker
/usr/share/icons/hicolor/*/apps/braillemaker.png
/usr/share/icons/hicolor/*/mimetypes/application-x-braillemaker-*.png
%post
update-mime-database /usr/share/mime >/dev/null 2>&1 || :
update-desktop-database /usr/share/applications >/dev/null 2>&1 || :
fc-cache -f /usr/share/fonts/truetype/braillemaker >/dev/null 2>&1 || :
%postun
update-mime-database /usr/share/mime >/dev/null 2>&1 || :
update-desktop-database /usr/share/applications >/dev/null 2>&1 || :
fc-cache -f >/dev/null 2>&1 || :
EOF
if command -v rpmbuild >/dev/null; then
  rpmbuild --define "_topdir $stage/rpm" --define "_rpmdir $stage/out" -bb "$stage/rpm/SPECS/braillemaker.spec"
  mv "$stage"/out/*/*.rpm "$out/"
else
  echo "rpmbuild is not installed here: no .rpm made"
fi
rm -rf "$stage"
ls -la "$out"
