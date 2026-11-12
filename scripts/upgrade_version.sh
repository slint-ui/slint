#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Set the version of Slint to <major>.<minor>.<patch> in the whole repository.
#
# Usage: scripts/upgrade_version.sh 1.20.0

set -euo pipefail

version=${1:-}
if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    echo "Usage: $0 <major>.<minor>.<patch>" >&2
    exit 1
fi
major=${BASH_REMATCH[1]} minor=${BASH_REMATCH[2]} patch=${BASH_REMATCH[3]}
edition="$major.$minor"

cd "$(dirname "$0")/.."

# On a new minor version, code that selects the edition of the previous version moves to the new one,
# along with the text and diagnostics that name it as the current one.
# Only that edition matches, so code that pins an older edition keeps it.
[[ $(grep -m1 '^version = ' Cargo.toml) =~ \"([0-9]+)\.([0-9]+)\. ]]
old_major=${BASH_REMATCH[1]} old_minor=${BASH_REMATCH[2]}
if [ "$old_major.$old_minor" != "$edition" ]; then
    old="$old_major\.$old_minor"
    git grep -lzIE "$old|$old_major, *$old_minor" -- ':!CHANGELOG.md' ":!tests/helper_components/edition_${old_major}_${old_minor}_library" | xargs -0 sed -i -E \
        -e 's/(\bedition\\?"?\s*[:=]\s*\\?")'"$old"'(\\?")/\1'"$edition"'\2/g' \
        -e 's/(--edition[= ]|\bEDITION )'"$old"'([^.0-9]|$)/\1'"$edition"'\2/g' \
        -e 's/(\b(with|set)_edition\(\(\s*)'"$old_major"',\s*'"$old_minor"'(\s*\)\))/\1'"$major, $minor"'\3/g' \
        -e 's/(such as [`"'"'"']*|newer than Slint |use edition |edition `|the number `)'"$old"'([^.0-9]|$)/\1'"$edition"'\2/g'
fi

# Each Cargo.toml needs to have the version updated
git ls-files '*Cargo.toml' ':!*helper_crates*' | xargs sed -i 's/^version = "[0-9]*\.[0-9]*\.[0-9]*"/version = "'"$version"'"/'
# Each dependencies in cargo.toml
git ls-files '*Cargo.toml' | xargs sed -i 's/\(slint.*version = \)"=[0-9]*\.[0-9]*\.[0-9]*"/\1"='"$version"'"/'

# Update the version in CmakeLists.txt
sed -i 's/ VERSION [0-9]*\.[0-9]*\.[0-9]*)$/ VERSION '"$version"')/' api/cpp/CMakeLists.txt

# The version is also in these files
sed -i 's/^version = "[0-9]*\.[0-9]*\.[0-9]*\(.*\)"/version = "'"$version"'\1"/' api/python/slint/pyproject.toml api/python/slint-dev/pyproject.toml api/python/briefcase/pyproject.toml tools/compiler/pyproject.toml editors/zed/extension.toml

# Keep the slint-dev pin in slint's [dev] extra in sync
sed -i 's/slint-dev==[0-9]*\.[0-9]*\.[0-9]*\(.*\)"/slint-dev=='"$version"'\1"/' api/python/slint/pyproject.toml

# Version in package.json files, the Node.js binary package config, and the AI plugins
# (cursor, claude, ...) mirrored to the ai-plugins repo
{
    git ls-files '*package.json' 'ai-plugins/*plugin.json' 'ai-plugins/*marketplace.json'
    echo api/node/binaries.json api/node/binaries-dev.json
} | xargs sed -i 's/"version": ".*"/"version": "'"$version"'"/'

# The highlight.js grammar that the Claude plugin loads from npm
sed -i 's/highlightjs-slint@[0-9]*\.[0-9]*\.[0-9]*"/highlightjs-slint@'"$version"'"/' ai-plugins/.claude-plugin/plugin.json

# VersionCheck
sed -i "s/VersionCheck_[0-9]*_[0-9]*_[0-9]*;/VersionCheck_${major}_${minor}_${patch};/" api/rs/slint/lib.rs api/slint-sc/lib.rs

# Version in the AboutSlint widget
sed -i 's/@tr("Version {}", "[0-9]*\.[0-9]*\.[0-9]*")/@tr("Version {}", "'"$version"'")/' internal/compiler/widgets/common/about-slint.slint

# Version in the docs (cargo add slint@<VERSION>):
git ls-files | grep "\(^\|/\)docs/.*\.\(md\|rst\)\$" | grep -v "/LICENSES/" | xargs sed --follow-symlinks -i 's/slint@[0-9]\+\.[0-9]\+\.[0-9]\+/slint@'"$version"'/'

# Version in esp-idf component
sed -i 's/^version: "[0-9]*\.[0-9]*\.[0-9]*.*"/version: "'"$version"'"/' api/cpp/esp-idf/slint/idf_component.yml
sed -i -e 's/GIT_TAG v[0-9]*\.[0-9]*\.[0-9]*/GIT_TAG v'"$version"'/' \
    -e 's/find_package(Slint [0-9]*\.[0-9]*\.[0-9]*)/find_package(Slint '"$version"')/' api/cpp/esp-idf/slint/CMakeLists.txt

# Version in the Android docs
sed -i 's/^slint = { version = "[^"]*"/slint = { version = "'"$version"'"/' docs/astro/src/content/docs/guide/platforms/mobile/android.mdx

# Marketing version for the iOS viewer
sed -i 's/^\( *MARKETING_VERSION: \)"[0-9]*\.[0-9]*\.[0-9]*"/\1"'"$version"'"/' tools/viewer/ios-project.yml

# Some documentation uses the `~` syntax
sed -i 's/\(slint.*version = \)"~[0-9]*\.[0-9]*"/\1"~'"$edition"'"/' api/rs/slint/Cargo.toml api/rs/slint/lib.rs

# --workspace only moves the workspace's own and path packages to the new version
git ls-files '*Cargo.lock' | while read -r lock; do
    cargo update --workspace --manifest-path "$(dirname "$lock")/Cargo.toml"
done
# The editable and directory packages of a uv.lock are ours, and carry the version
git grep -l 'source = { \(editable\|directory\) = ' -- '*uv.lock' | while read -r lock; do
    uv lock --project "$(dirname "$lock")"
done

echo "Note that the version is not updated in the documentation and README yet"
