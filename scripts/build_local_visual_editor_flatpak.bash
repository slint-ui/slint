# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#!/bin/bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"

if [ -z "${1}" ]; then
    echo -e "Usage: $(basename $0) <OUTPUT>"
    exit 1
fi

if [ -z "${CARGO_PROFILE:-}" ]; then
    echo -e "CARGO_PROFILE unset, defaulting to dev (--debug)"
fi

export FLATPAK_CARGO_GENERATOR_PATH="$(mktemp)"

export CARGO_SOURCES_PATH="${repo_root}/tools/lsp/cargo-sources.json"
export OUTPUT_FLATPAK_MANIFEST="${repo_root}/tools/lsp/org.sixtyfps.SlintVisualEditor.yml"

trap "rm ${CARGO_SOURCES_PATH} ${OUTPUT_FLATPAK_MANIFEST}" EXIT

cargo_generator_url=https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/737c0085912f9f7dabf9341d4608e2a77a51a73a/cargo/flatpak-cargo-generator.py

flatpak_repo_dir="$(mktemp -d)"
flatpak_build_dir="$(mktemp -d)"
flatpak_state_dir="${TMPDIR:-/tmp}/flatpak-build-state"

mkdir -p "${flatpak_state_dir}"

curl -o "${FLATPAK_CARGO_GENERATOR_PATH}" \
        "${cargo_generator_url}"
chmod +x "${FLATPAK_CARGO_GENERATOR_PATH}"

"${repo_root}/scripts/generate_visual_editor_flatpak_manifest.bash"

flatpak-builder \
    --force-clean \
    --user \
    --install-deps-from=flathub \
    --repo="${flatpak_repo_dir}" \
    --state-dir="${flatpak_state_dir}" \
    "${flatpak_build_dir}" \
    "${OUTPUT_FLATPAK_MANIFEST}"

flatpak_out_name="$(basename "${1}")"

flatpak build-bundle \
    "${flatpak_repo_dir}" \
    "${flatpak_out_name}" \
    org.sixtyfps.SlintVisualEditor \
    --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo

if [ "${flatpak_out_name}" != "${1}" ]; then
    mv "${flatpak_out_name}" "${1}"
fi

exit 0
