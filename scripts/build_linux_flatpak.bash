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

output_flatpak_yml="${PWD}/org.sixtyfps.SlintVisualEditor.yml"

echo -e 'Generated flatpak-builder file:'
sed \
    org.sixtyfps.SlintVisualEditor.template.yml \
    -e 's/\$\$CURRENT_COMMIT\$\$/'${current_commit}'/g; s/\$\$CARGO_PROFILE\$\$/'${CARGO_PROFILE}'/g; s/\$\$CARGO_PROFILE_DIR\$\$/'${cargo_profile_dir}'/g' \
    | tee "${output_flatpak_yml}" 1>&2

trap "rm ${output_flatpak_yml}" EXIT

flatpak-builder \
    --force-clean \
    --user \
    --install-deps-from=flathub \
    --repo=repo \
    builddir \
    org.sixtyfps.SlintVisualEditor.yml

flatpak build-bundle \
    repo \
    "slint-visual-editor_$version.flatpak" \
    org.sixtyfps.SlintVisualEditor \
    --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo

# TODO: Append version to metainfo
