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

echo -e 'Generated flatpak-builder file:'
sed -e 's/\$\$CURRENT_COMMIT\$\$/TEST/g' org.sixtyfps.SlintVisualEditor.template.yml | tee org.sixtyfps.SlintVisualEditor.yml 1>&2

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
