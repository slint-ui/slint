#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Build the Slint Visual Editor flatpak, with Skia compiled from source inside
# the sandbox. Generates the source lists the manifest references, runs
# flatpak-builder, and exports a single-file bundle.
#
# Usage: build_visual_editor_flatpak.bash [OUTPUT.flatpak]
#
# The bundle lands in dist/ unless a path is given. A build that leaves nothing
# behind is only good for checking that the manifest still builds, and that is
# the rarer thing to want.
#
# Needs flatpak and flatpak-builder. The runtime, the SDK and the
# rust-stable/llvm20 SDK extensions come from flathub; the remote is added to
# the user installation here if it is not there yet.
#
# Compiling Skia takes a long time and tens of gigabytes under
# .flatpak-builder, which doubles as the cache that makes a second run quick.
#
# The build only packages committed state: the manifest uses the repository's
# HEAD, not the working tree.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/helpers.sh"

app_id=dev.slint.VisualEditor

usage() {
    cat <<EOF
Usage: $(basename "$0") [OUTPUT.flatpak]

Exports a single-file bundle, by default to

    dist/slint-visual-editor-<arch>.flatpak

Install it with \`flatpak install --user <bundle>\`, then run it with
\`flatpak run ${app_id}\`.
EOF
}

require_tools() {
    local tool
    for tool in "$@"; do
        command -v "$tool" >/dev/null || die "$tool is required but not on PATH"
    done
}

case "${1:-}" in
    -h | --help) usage; exit 0 ;;
esac
[ $# -le 1 ] || { usage; die "expected at most one argument"; }

require_tools git flatpak flatpak-builder

repo_root="$(git rev-parse --show-toplevel)"
manifest="${repo_root}/tools/editor/dev.slint.VisualEditor.yml"
state_dir="${repo_root}/.flatpak-builder"

# Named the way CI names its artifacts, so a locally built bundle and a
# downloaded one are told apart by nothing but where they came from.
bundle="${repo_root}/dist/slint-visual-editor-$(flatpak --default-arch).flatpak"
if [ $# -ge 1 ]; then
    [ -d "$(dirname "$1")" ] || die "no such directory: $(dirname "$1")"
    bundle="$(abs_path "$1")"
fi
mkdir -p "$(dirname "${bundle}")"

# flatpak-builder installs the runtime and the SDK extensions with --user, and
# a --user install only resolves remotes that installation knows about. A
# flathub that exists system-wide is not enough: the dependency install fails
# with "No remote refs found for 'flathub'".
if ! flatpak remotes --user --columns=name | grep -qx flathub; then
    log "Adding the flathub remote to the user installation"
    flatpak remote-add --if-not-exists --user flathub \
        https://dl.flathub.org/repo/flathub.flatpakrepo
fi

# The manifest builds this repository at HEAD, so uncommitted work is absent
# from the result. Cheaper to say so now than to debug why a change did nothing.
if [ -n "$(git -C "${repo_root}" status --porcelain)" ]; then
    log "note: building HEAD ($(git -C "${repo_root}" rev-parse --short HEAD)); uncommitted changes are not included"
fi

# flatpak-builder hardlinks cached sources into the build directory, so the
# build directory has to be on the same filesystem as the state directory.
# $TMPDIR is tmpfs on most distributions, and mktemp's default there fails with
# "The state dir is not on the same filesystem as the target dir", so both
# working directories live beside the cache instead.
work_dir="${state_dir}/local-build"
build_dir="${work_dir}/build"
repo_dir="${work_dir}/repo"

rm -rf "${work_dir}"
mkdir -p "${work_dir}"

# The build tree holds the logs that explain a failure, so it is only removed
# on success. The cache next to it survives either way.
keep_build_tree_on_failure() {
    local status=$?
    [ "${status}" -eq 0 ] && return 0
    echo "the build tree was kept for inspection: ${build_dir}" >&2
}
trap keep_build_tree_on_failure EXIT

"${repo_root}/scripts/generate_visual_editor_flatpak_sources.bash"

flatpak-builder \
    --force-clean \
    --user \
    --install-deps-from=flathub \
    --state-dir="${state_dir}" \
    --repo="${repo_dir}" \
    "${build_dir}" \
    "${manifest}"

flatpak build-bundle \
    "${repo_dir}" \
    "${bundle}" \
    "${app_id}" \
    --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo

log "Bundle written to ${bundle}"
log "Install it with: flatpak install --user ${bundle}"

rm -rf "${work_dir}"
