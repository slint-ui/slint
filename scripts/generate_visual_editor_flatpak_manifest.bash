# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#!/bin/bash
set -xeuo pipefail

repo_root="$(git rev-parse --show-toplevel)"

required_commands=(jq)
missing_commands=()

for required_command in "${required_commands[@]}"; do
    if ! command -v "${required_command}" >/dev/null; then
        missing_commands+=("${required_command}")
    fi
done

if [ ${#missing_commands[@]} -ne 0 ]; then
    echo "This script requires the following tools to be installed: ${missing_commands[@]}" >&2
    exit 1
fi

version=$(
    cargo metadata --manifest-path tools/lsp/Cargo.toml --offline --format-version 1 --no-deps |
        jq -r 'first(.packages[] | select(.name == "slint-lsp") | .version)'
)
current_commit=$(git rev-parse --verify HEAD)

CARGO_PROFILE="${CARGO_PROFILE:-dev}"

if [ -z "${FLATPAK_CARGO_GENERATOR_PATH:-}" ] || ! [ -f "${FLATPAK_CARGO_GENERATOR_PATH}" ]; then
    echo 'Please download flatpak-cargo-generator.py from github.com/flatpak/flatpak-builder-tools and set FLATPAK_CARGO_GENERATOR_PATH to point to it' >&2
    exit 1
fi

CARGO_SOURCES_PATH="${CARGO_SOURCES_PATH:-${repo_root}/tools/lsp/cargo-sources.json}"

python3 "${FLATPAK_CARGO_GENERATOR_PATH}" "${repo_root}/Cargo.lock" -o "${CARGO_SOURCES_PATH}"

output_flatpak_manifest_path="${OUTPUT_FLATPAK_MANIFEST:-${repo_root}/tools/lsp/org.sixtyfps.SlintVisualEditor.yml}"

relative_sources_path="$(python3 -c "import os.path; print(os.path.relpath('${CARGO_SOURCES_PATH}', '${repo_root}/tools/lsp'))")"

echo 'Generated flatpak manifest:' >&2
"${repo_root}/scripts/handlebars.rs" \
    -i "${repo_root}/tools/lsp/org.sixtyfps.SlintVisualEditor.yml.hbs" \
    -v "git.commit=${current_commit}" \
    -v "git.local=${repo_root}" \
    -v "cargo.profile=${CARGO_PROFILE}" \
    -v "cargo.sources=${relative_sources_path}" \
    | tee "${output_flatpak_manifest_path}" 1>&2

echo -e
echo -e "Path: ${output_flatpak_manifest_path}"
