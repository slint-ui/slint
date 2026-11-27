#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Replace the plugin in a checkout of a mirror with ai-plugins/ and the preview checker built from it:
# slint-ui/ai-plugins for the `release` channel, slint-ui/ai-plugins-nightly for `snapshot`.
#
# Usage: scripts/prepare_ai_plugins.sh <release|snapshot> <mirror checkout>

set -euo pipefail

channel=${1:-}
mirror=${2:-}
if [[ ! "$channel" =~ ^(release|snapshot)$ || ! -d "$mirror/.git" ]]; then
    echo "Usage: $0 <release|snapshot> <mirror checkout>" >&2
    exit 1
fi
if [ -n "$(git -C "$mirror" status --porcelain)" ]; then
    echo "$mirror has uncommitted changes" >&2
    exit 1
fi
mirror=$(cd "$mirror" && pwd)

cd "$(dirname "$0")/.."

pnpm --dir ai-plugins/mcp/slint-preview build:checker

# The mirror keeps its own README and workflows.
# wasm-pack ignores its whole output directory, which the mirror commits.
rsync -aL --delete \
    --exclude=/.git --exclude=/.github --exclude=/README.md \
    --exclude=/mcp/slint-preview/checker/.gitignore \
    ai-plugins/ "$mirror/"
printf '{ "channel": "%s" }\n' "$channel" > "$mirror/mcp/slint-preview/channel.json"

if [ "$channel" = release ]; then
    exit 0
fi

cd "$mirror"

# Clients key marketplaces by name, so a distinct name lets the nightly be added next to the release.
for marketplace in .claude-plugin/marketplace.json .cursor-plugin/marketplace.json; do
    perl -pi -e 's/^  "name": "slint",$/  "name": "slint-nightly",/' "$marketplace"
    if ! grep -q '^  "name": "slint-nightly",$' "$marketplace"; then
        echo "Couldn't rename the marketplace in $marketplace" >&2
        exit 1
    fi
done

# Clients only offer an update when the version changes, so each change in content gets a new timestamp.
manifests=$(find . -path ./.git -prune -o \( -name plugin.json -o -name marketplace.json \) -print)
set_version() {
    echo "$manifests" | xargs perl -pi -e "s/\"version\": \"[^\"]*\"/\"version\": \"$1\"/"
}
version=$(sed -n 's/.*"version": "\([^"]*\)".*/\1/p' mcp/.claude-plugin/plugin.json)
previous=$(git show HEAD:mcp/.claude-plugin/plugin.json | sed -n 's/.*"version": "[^"]*-nightly\.\([0-9]*\)".*/\1/p')
if [ -n "$previous" ]; then
    set_version "$version-nightly.$previous"
    if [ -z "$(git status --porcelain)" ]; then
        exit 0
    fi
fi
set_version "$version-nightly.$(date -u +%Y%m%d%H%M)"
