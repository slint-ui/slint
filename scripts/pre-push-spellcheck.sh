#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cspell:ignore ACMR
#
# Spell-check the files a `git push` is about to send, so CI's cspell gate
# (which blocks the rest of the CI matrix on any unrecognized word) doesn't
# have to catch it first.
#
# Install as a native git pre-push hook:
#   cp scripts/pre-push-spellcheck.sh .git/hooks/pre-push
#   chmod +x .git/hooks/pre-push
#
# Bypass once with: git push --no-verify

set -eu

zero="0000000000000000000000000000000000000000"
changed_files=""

while read -r local_ref local_sha remote_ref remote_sha; do
    [ "$local_sha" = "$zero" ] && continue # deleting a ref: nothing to check

    if [ "$remote_sha" = "$zero" ]; then
        # New ref: no remote history to diff against, so fall back to the
        # merge-base with the default branch.
        base=$(git merge-base "$local_sha" master 2>/dev/null || echo "$local_sha")
    else
        base="$remote_sha"
    fi

    files=$(git diff --name-only --diff-filter=ACMR "$base" "$local_sha" 2>/dev/null || true)
    changed_files="$changed_files
$files"
done

changed_files=$(printf '%s\n' "$changed_files" | sed '/^$/d' | sort -u)

if [ -z "$changed_files" ]; then
    exit 0
fi

if ! command -v pnpm >/dev/null 2>&1; then
    echo "pre-push: pnpm not found, skipping spell-check (CI will still catch it)" >&2
    exit 0
fi

# shellcheck disable=SC2086
if ! pnpm exec cspell --no-progress --gitignore $changed_files; then
    echo "" >&2
    echo "pre-push: cspell failed on the files above." >&2
    echo "Fix the words, or add genuinely new technical terms to .cspell/slint-project-words.txt (alphabetical)." >&2
    echo "Bypass once with: git push --no-verify" >&2
    exit 1
fi
