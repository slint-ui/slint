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
pkgs=()

for file in "$@"; do
    dir=$(dirname "$file")
    while [ "$dir" != "." ] && [ "$dir" != "/" ]; do
        if [ -f "$dir/Cargo.toml" ]; then
            pkg=$(grep -m1 '^name\s*=' "$dir/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')
            if [ -n "$pkg" ] && [ -z "${seen[$pkg]}" ]; then
                seen[$pkg]=1
                pkgs+=(-p "$pkg")
            fi
            break
        fi
        dir=$(dirname "$dir")
    done
done

if [ ${#pkgs[@]} -eq 0 ]; then
    exit 0
fi

cargo clippy --locked "${pkgs[@]}" -- -D warnings
