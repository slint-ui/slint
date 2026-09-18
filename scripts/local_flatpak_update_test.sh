#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore assumeyes flatpakref ostree

# Exercise the Flatpak updater against a repository on this machine, so that
# the whole path - banner at startup, portal install, restart into the new
# deployment - can be watched without publishing anything.
#
# The second commit is made from identical content with
# `flatpak build-commit-from --force`, so the app only has to be built once.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/helpers.sh"

APP_ID="dev.slint.VisualEditor"
REMOTE_NAME="slint-visual-editor-local-test"
WORK_DIR="${WORK_DIR:-${TMPDIR:-/tmp}/slint-flatpak-local-test}"
PORT="8766"
BUNDLE=""
REPO=""
CLEAN=0

usage() {
    cat <<EOF
Usage:
  $0 --bundle path/to/slint-visual-editor.flatpak
  $0 --repo path/to/exported/repo
  $0 --clean

Options:
  --bundle FILE   Single-file bundle, from scripts/build_visual_editor_flatpak.bash
                  or from https://visual-editor.slint.dev/nightly/
  --repo DIR      An already exported OSTree repository, used instead of a bundle
  --port PORT     Port for the local server. Default: $PORT
  --work-dir DIR  Where the repository and the server live. Default: $WORK_DIR
  --clean         Uninstall the test app, drop its remote, stop the server

The app is installed --user, which takes precedence over a system-wide
install of the same id for as long as the test runs. --clean puts that back.
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --bundle) BUNDLE="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        --port) PORT="$2"; shift 2 ;;
        --work-dir) WORK_DIR="$2"; shift 2 ;;
        --clean) CLEAN=1; shift ;;
        -h | --help) usage; exit 0 ;;
        *) usage; die "unknown argument: $1" ;;
    esac
done

SERVE_DIR="$WORK_DIR/serve"
SERVER_PID_FILE="$WORK_DIR/server.pid"

require_tools() {
    local tool
    for tool in "$@"; do
        command -v "$tool" >/dev/null || die "$tool is required but not on PATH"
    done
}

stop_server() {
    [ -f "$SERVER_PID_FILE" ] || return 0
    kill "$(cat "$SERVER_PID_FILE")" 2>/dev/null || true
    rm -f "$SERVER_PID_FILE"
}

clean() {
    log "Uninstalling the test app"
    flatpak uninstall --user --assumeyes "$APP_ID" 2>/dev/null || true
    flatpak remote-delete --user "$REMOTE_NAME" 2>/dev/null || true
    # Whatever the portal remembered about "the application wants to update
    # itself", so the next run asks again.
    flatpak permission-remove flatpak updates "$APP_ID" 2>/dev/null || true
    stop_server
    rm -rf "$WORK_DIR"
    log "Done"
}

if [ "$CLEAN" = 1 ]; then
    clean
    exit 0
fi

[ -n "$BUNDLE" ] || [ -n "$REPO" ] || { usage; die "--bundle or --repo is required"; }
require_tools flatpak ostree python3 curl

stop_server
rm -rf "$WORK_DIR"
mkdir -p "$SERVE_DIR"

STAGE_REPO="$WORK_DIR/repo"
if [ -n "$BUNDLE" ]; then
    [ -f "$BUNDLE" ] || die "no such bundle: $BUNDLE"
    log "Importing $(basename "$BUNDLE") into a repository"
    ostree init --repo="$STAGE_REPO" --mode=archive
    flatpak build-import-bundle "$STAGE_REPO" "$(abs_path "$BUNDLE")"
else
    [ -d "$REPO" ] || die "no such repository: $REPO"
    log "Copying $REPO"
    cp -r "$REPO" "$STAGE_REPO"
fi

# The branch is whatever the build used - CI passes the channel, a local build
# leaves it at master - and it decides where the app looks, so the layout under
# the server is built around it rather than around a guess.
REF="$(ostree --repo="$STAGE_REPO" refs | grep "^app/$APP_ID/" | head -1)"
[ -n "$REF" ] || die "$STAGE_REPO has no ref for $APP_ID"
BRANCH="${REF##*/}"
CHANNEL_DIR="$SERVE_DIR/$BRANCH"
SERVE_REPO="$CHANNEL_DIR/flatpak"
BASE_URL="http://127.0.0.1:$PORT/$BRANCH"

log "Serving $REF as $BASE_URL/flatpak"
mkdir -p "$CHANNEL_DIR"
mv "$STAGE_REPO" "$SERVE_REPO"
flatpak build-update-repo "$SERVE_REPO" >/dev/null

python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$SERVE_DIR" \
    >"$WORK_DIR/server.log" 2>&1 &
echo $! > "$SERVER_PID_FILE"
sleep 1
curl -sS --fail "$BASE_URL/flatpak/refs/heads/$REF" >/dev/null ||
    die "the local server is not answering; see $WORK_DIR/server.log"

# Installing from a flatpakref is what points the origin remote at the local
# server: the portal updates from wherever the app came from. Unsigned is fine
# for a --user install; only system installs insist on a signature.
FLATPAKREF="$WORK_DIR/$APP_ID.flatpakref"
cat > "$FLATPAKREF" <<EOF
[Flatpak Ref]
Title=Slint Visual Editor (local test)
Name=$APP_ID
Branch=$BRANCH
Url=$BASE_URL/flatpak
SuggestRemoteName=$REMOTE_NAME
IsRuntime=false
RuntimeRepo=https://flathub.org/repo/flathub.flatpakrepo
EOF

log "Installing the app"
flatpak install --user --assumeyes --reinstall "$FLATPAKREF"
INSTALLED="$(flatpak info --user "$APP_ID" | awk '/^ *Commit:/ { print $2 }')"

# Same content, new commit: enough for the app to see something waiting, and it
# saves building the editor a second time.
log "Publishing a second commit"
flatpak build-commit-from \
    --src-repo="$SERVE_REPO" --src-ref="$REF" \
    --force --timestamp=NOW --subject="local update test" \
    "$SERVE_REPO" "$REF" >/dev/null
flatpak build-update-repo "$SERVE_REPO" >/dev/null
REMOTE_COMMIT="$(cat "$SERVE_REPO/refs/heads/$REF")"

[ "$INSTALLED" != "$REMOTE_COMMIT" ] || die "the second commit matches the first"

cat <<EOF

Installed  ${INSTALLED:0:12}
Remote     ${REMOTE_COMMIT:0:12}

Run the editor against the local server:

  flatpak run --env=SLINT_FLATPAK_BASE_URL=$BASE_URL $APP_ID

What to expect:

  1. "Update" in the banner within a second of startup, from the ref above
  2. clicking it: the portal asks once whether the app may update itself,
     then the banner counts through Downloading and Installing
  3. "Restart to update", and clicking that comes back on the new commit

To skip the portal's question:  flatpak permission-set flatpak updates $APP_ID yes
When finished:                  $0 --clean
EOF
