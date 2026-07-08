# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#!/bin/bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"

cd "${repo_root}/tools/lsp"

version=$(
    cargo metadata --offline --format-version 1 --no-deps |
        jq -r 'first(.packages[] | select(.name == "slint-lsp") | .version)'
)
current_commit=$(git rev-parse --verify HEAD)

CARGO_PROFILE="${CARGO_PROFILE:-dev}"

cargo_profile_dir="${CARGO_PROFILE}"

if [ "${cargo_profile_dir}" = dev ]; then
    cargo_profile_dir=debug
fi

if [ -z "${FLATPAK_CARGO_GENERATOR_PATH:-}" ] && ! [ -f "${flatpak_cargo_generator}" ]; then
    echo -e 'Please download flatpak-cargo-generator.py from github.com/flatpak/flatpak-builder-tools and set FLATPAK_CARGO_GENERATOR_PATH to point to it'
    exit 1
fi

"${FLATPAK_CARGO_GENERATOR_PATH}" "${repo_root}/Cargo.lock" -o "${CARGO_SOURCES_PATH:-cargo-sources.json}"

output_flatpak_manifest_path="${OUTPUT_FLATPAK_MANIFEST:-${PWD}/org.sixtyfps.SlintVisualEditor.yml}"

echo -e 'Generated flatpak manifest:'
sed \
    org.sixtyfps.SlintVisualEditor.template.yml \
    -e 's/\$\$GIT_COMMIT\$\$/'${current_commit}'/g;
        s:\$\$GIT_CHECKOUT_PATH\$\$:'${repo_root}':g;
        s/\$\$CARGO_PROFILE\$\$/'${CARGO_PROFILE}'/g;
        s/\$\$CARGO_PROFILE_DIR\$\$/'${cargo_profile_dir}'/g' \
    | tee "${output_flatpak_manifest_path}" 1>&2

echo -e
echo -e "Path: ${output_flatpak_manifest_path}"
