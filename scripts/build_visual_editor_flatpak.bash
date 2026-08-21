#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Build the Slint Visual Editor flatpak, with Skia compiled from source inside
# the sandbox. Generates the source lists the manifest references, runs
# flatpak-builder, and optionally exports a single-file bundle.
#
# Usage: build_visual_editor_flatpak.bash [OUTPUT.flatpak]
#
# Requires flatpak-builder and the flathub remote (for the runtime, SDK and
# the rust-stable/llvm20 SDK extensions):
#   flatpak remote-add --if-not-exists --user flathub https://dl.flathub.org/repo/flathub.flatpakrepo
#
# The build only packages committed state: the manifest uses the repository's
# HEAD, not the working tree.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
manifest="${repo_root}/tools/editor/dev.slint.VisualEditor.yml"
app_id=dev.slint.VisualEditor

build_dir="$(mktemp -d)"
repo_dir="$(mktemp -d)"
trap 'rm -rf "${build_dir}" "${repo_dir}"' EXIT

"${repo_root}/scripts/generate_visual_editor_flatpak_sources.bash"

flatpak-builder \
    --force-clean \
    --user \
    --install-deps-from=flathub \
    --state-dir="${repo_root}/.flatpak-builder" \
    --repo="${repo_dir}" \
    "${build_dir}" \
    "${manifest}"

if [ $# -ge 1 ]; then
    flatpak build-bundle \
        "${repo_dir}" \
        "${1}" \
        "${app_id}" \
        --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
    echo "Bundle written to ${1}"
fi
