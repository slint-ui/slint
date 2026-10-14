#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cSpell: ignore admindir esac fsys libsqlite lname mktemp noninteractive readlink

# Installs Ubuntu 22.04 packages for the target architecture into the sysroot,
# for example in a cross `pre-build` step:
#
#     pre-build = ["sysroot-apt-install libsqlite3-dev"]
#
# The packages come from the same release as the sysroot, so they work with
# its glibc. apt resolves the dependencies against the packages the sysroot
# already has, which it tracks in $CROSS_SYSROOT/var/lib/dpkg/status, and the
# packages are only unpacked: their maintainer scripts don't run.

set -eu

if [ $# -eq 0 ]; then
    echo "usage: sysroot-apt-install <package>..." >&2
    exit 1
fi

: "${CROSS_SYSROOT:?}" "${SYSROOT_DEB_ARCH:?}" "${SYSROOT_APT_URI:?}"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/lists/partial" "$work/archives/partial" "$work/parts"

admindir="$CROSS_SYSROOT/var/lib/dpkg"

cat > "$work/sources.list" <<EOF
deb [arch=$SYSROOT_DEB_ARCH signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] $SYSROOT_APT_URI jammy main universe
deb [arch=$SYSROOT_DEB_ARCH signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] $SYSROOT_APT_URI jammy-updates main universe
deb [arch=$SYSROOT_DEB_ARCH signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] $SYSROOT_APT_URI jammy-security main universe
EOF

# An apt that is separate from the one of the image itself. It sees the
# packages the way the Ubuntu 22.04 system the sysroot was taken from did:
# amd64 is the native architecture and the target a foreign one.
cat > "$work/apt.conf" <<EOF
APT::Architecture "amd64";
APT::Architectures { "amd64"; "$SYSROOT_DEB_ARCH"; };
APT::Install-Recommends "false";
APT::Sandbox::User "root";
Dir::Etc::SourceList "$work/sources.list";
Dir::Etc::SourceParts "$work/parts";
Dir::Etc::Preferences "$work/preferences";
Dir::Etc::PreferencesParts "$work/parts";
Dir::State::Lists "$work/lists";
Dir::State::status "$admindir/status";
Dir::Cache "$work";
Dir::Cache::Archives "$work/archives";
EOF
export APT_CONFIG="$work/apt.conf"

# Packages are for the target unless they name an architecture.
for package in "$@"; do
    case "$package" in
        *:*) set -- "$@" "$package" ;;
        *) set -- "$@" "$package:$SYSROOT_DEB_ARCH" ;;
    esac
    shift
done

apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install --assume-yes --download-only "$@"

for deb in "$work"/archives/*.deb; do
    [ -e "$deb" ] || continue
    echo "Unpacking $(basename "$deb") into $CROSS_SYSROOT"
    # The sysroot's /lib is a symlink to usr/lib, which must stay one.
    dpkg-deb --fsys-tarfile "$deb" | tar -x -C "$CROSS_SYSROOT" --keep-directory-symlink --no-same-owner

    # Record the package as installed, replacing an older version.
    package=$(dpkg-deb --field "$deb" Package)
    arch=$(dpkg-deb --field "$deb" Architecture)
    awk -v RS= -v ORS='\n\n' -v p="Package: $package" -v a="Architecture: $arch" \
        '!(index($0 "\n", p "\n") == 1 && index($0 "\n", "\n" a "\n"))' \
        "$admindir/status" > "$work/status"
    {
        cat "$work/status"
        dpkg-deb --field "$deb" | awk 'NR == 1 { print; print "Status: install ok installed"; next } { print }'
        echo
    } > "$admindir/status"
done

# Absolute symlinks are made relative so that they resolve inside the sysroot.
find "$CROSS_SYSROOT" -type l -lname '/*' -exec sh -c 'ln -sfr "$0$(readlink "$1")" "$1"' "$CROSS_SYSROOT" {} \;
