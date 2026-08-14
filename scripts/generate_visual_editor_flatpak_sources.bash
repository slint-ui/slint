#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tomlkit

# Generate the source lists that tools/editor/dev.slint.VisualEditor.yml references.
# The flatpak sandbox has no network, so every crate and every part of the Skia
# source tree has to be declared up front. Re-run this whenever Cargo.lock
# changes; the generated files are not checked in.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"

cargo_generator_url=https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/737c0085912f9f7dabf9341d4608e2a77a51a73a/cargo/flatpak-cargo-generator.py
cargo_generator="$(mktemp --suffix=-flatpak-cargo-generator.py)"
trap 'rm -f "${cargo_generator}"' EXIT

curl -sSfL -o "${cargo_generator}" "${cargo_generator_url}"

# uv resolves the generator's inline dependency block on the fly; plain
# python3 needs aiohttp and tomlkit preinstalled
if command -v uv >/dev/null; then
    run_python=(uv run)
else
    python3 -c "import aiohttp, tomlkit" 2>/dev/null ||
        python3 -m pip install --user aiohttp tomlkit
    run_python=(python3)
fi

cd "${repo_root}"
"${run_python[@]}" "${cargo_generator}" Cargo.lock -o tools/editor/cargo-sources.json
python3 scripts/flatpak-skia-generator.py -o tools/editor/skia-sources.json
