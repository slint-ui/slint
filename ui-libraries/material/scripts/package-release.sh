#!/bin/sh

# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -eu

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 VERSION OUTPUT_DIRECTORY" >&2
    exit 2
fi

version=$1
output_directory=$2

for required_command in zip unzip sha256sum; do
    if ! command -v "$required_command" >/dev/null 2>&1; then
        echo "Missing required command: $required_command" >&2
        exit 1
    fi
done

case "$version" in
    *[!0-9.]* | "")
        echo "Invalid release version: $version" >&2
        exit 2
        ;;
esac

script_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
material_directory=$(CDPATH= cd -- "$script_directory/.." && pwd)

for required_file in material.slint README.md LICENSE.md; do
    if [ ! -f "$material_directory/src/$required_file" ]; then
        echo "Missing release file: src/$required_file" >&2
        exit 1
    fi
done

mkdir -p "$output_directory"
output_directory=$(CDPATH= cd -- "$output_directory" && pwd)
archive_name="material-$version.zip"
temporary_directory=$(mktemp -d)
trap 'rm -rf "$temporary_directory"' EXIT HUP INT TERM

cp -a "$material_directory/src" "$temporary_directory/material-$version"
(
    cd "$temporary_directory"
    zip -qr "$output_directory/$archive_name" "material-$version"
)

unzip -tq "$output_directory/$archive_name"
(
    cd "$output_directory"
    sha256sum "$archive_name" > "$archive_name.sha256"
)
