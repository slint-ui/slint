#!/bin/bash
# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Run cargo clippy only on packages containing the modified files (for pre-commit use).
# Files are passed as arguments by the pre-commit framework.
#
# To use this, add these lines to your .pre-commit-config.yaml
# - repo: local
#  hooks:
#  - id: clippy
#    name: cargo clippy
#    entry: scripts/pre-commit-clippy.sh
#    language: script
#    types: [rust]
#    pass_filenames: true

declare -A seen
declare -A workspace_of_dir
declare -A pkgs_by_workspace

for file in "$@"; do
    dir=$(dirname "$file")
    while [ "$dir" != "." ] && [ "$dir" != "/" ]; do
        if [ -f "$dir/Cargo.toml" ]; then
            pkg=$(grep -m1 '^name\s*=' "$dir/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')
            if [ -n "$pkg" ]; then
                workspace=${workspace_of_dir[$dir]}
                if [ -z "$workspace" ]; then
                    # Some directories are their own workspace, and clippy has
                    # to run from there rather than from the repository root.
                    workspace=$(cargo locate-project --workspace --message-format plain --manifest-path "$dir/Cargo.toml") || exit 1
                    workspace_of_dir[$dir]=$workspace
                fi
                if [ -z "${seen[$workspace:$pkg]}" ]; then
                    seen[$workspace:$pkg]=1
                    pkgs_by_workspace[$workspace]+="$pkg "
                fi
            fi
            break
        fi
        dir=$(dirname "$dir")
    done
done

status=0
for workspace in "${!pkgs_by_workspace[@]}"; do
    workspace_dir=$(dirname "$workspace")
    args=()
    for pkg in ${pkgs_by_workspace[$workspace]}; do
        args+=(-p "$pkg")
    done
    # Only some of the workspaces have a Cargo.lock, and --locked refuses to create one.
    [ -f "$workspace_dir/Cargo.lock" ] && args+=(--locked)
    ( cd "$workspace_dir" && cargo clippy "${args[@]}" -- -D warnings ) || status=1
done

exit $status
