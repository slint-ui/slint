#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -euo pipefail

APP_NAME="Slint Visual Editor"
ACCOUNT="slint-visual-editor-local-test"
HOST="127.0.0.1"
PORT="8765"
WORK_DIR="/private/tmp/slint-sparkle-local-test"
OLD_BUILD="100"
NEW_BUILD="101"
VERSION=""
OLD_APP=""
NEW_APP=""
SERVER_PID=""

usage() {
    cat <<EOF
Usage:
  $0 --old-app "/path/to/old/Slint Visual Editor.app" --new-app "/path/to/new/Slint Visual Editor.app"

Options:
  --version VERSION       Marketing version for both temp apps. Defaults to tools/lsp/Cargo.toml.
  --old-build BUILD      CFBundleVersion for the installed old app. Default: $OLD_BUILD
  --new-build BUILD      CFBundleVersion for the update app. Default: $NEW_BUILD
  --account ACCOUNT      Sparkle keychain account. Default: $ACCOUNT
  --port PORT            Local HTTP port. Default: $PORT
  --work-dir DIR         Temp working directory. Default: $WORK_DIR
  -h, --help             Show this help.

The script patches only temporary app copies, signs them ad-hoc, serves a local
appcast, launches the old app via LaunchServices, and waits while you click
through Sparkle's UI.
EOF
}

die() {
    echo "error: $*" >&2
    exit 1
}

log() {
    echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"
}

abs_path() {
    local path="$1"
    local dir

    dir="$(cd "$(dirname "$path")" && pwd)" || return 1
    printf "%s/%s\n" "$dir" "$(basename "$path")"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --old-app)
            OLD_APP="${2:-}"
            shift 2
            ;;
        --new-app)
            NEW_APP="${2:-}"
            shift 2
            ;;
        --version)
            VERSION="${2:-}"
            shift 2
            ;;
        --old-build)
            OLD_BUILD="${2:-}"
            shift 2
            ;;
        --new-build)
            NEW_BUILD="${2:-}"
            shift 2
            ;;
        --account)
            ACCOUNT="${2:-}"
            shift 2
            ;;
        --port)
            PORT="${2:-}"
            shift 2
            ;;
        --work-dir)
            WORK_DIR="${2:-}"
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

[ "$(uname -s)" = "Darwin" ] || die "this script requires macOS"
[ -n "$OLD_APP" ] || die "--old-app is required"
[ -n "$NEW_APP" ] || die "--new-app is required"
[ -d "$OLD_APP" ] || die "old app not found: $OLD_APP"
[ -d "$NEW_APP" ] || die "new app not found: $NEW_APP"
[ -n "$WORK_DIR" ] || die "--work-dir must not be empty"
case "$WORK_DIR" in
    /tmp/* | /private/tmp/*) ;;
    *) die "--work-dir must be under /tmp or /private/tmp" ;;
esac

for tool in codesign ditto plutil python3; do
    command -v "$tool" >/dev/null || die "$tool is required"
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_DIR="$ROOT_DIR/tools/lsp"
OLD_APP="$(abs_path "$OLD_APP")"
NEW_APP="$(abs_path "$NEW_APP")"

if [ -z "$VERSION" ]; then
    VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$PROJECT_DIR/Cargo.toml" | head -n 1)"
fi
[ -n "$VERSION" ] || die "could not determine version from $PROJECT_DIR/Cargo.toml"

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" >/dev/null 2>&1; then
        kill "$SERVER_PID" >/dev/null 2>&1 || true
        wait "$SERVER_PID" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

ensure_sparkle_tools() {
    if [ ! -d "$ROOT_DIR/Sparkle.framework" ] ||
        [ ! -x "$ROOT_DIR/sparkle-bin/sign_update" ] ||
        [ ! -x "$ROOT_DIR/sparkle-bin/generate_keys" ]; then
        "$ROOT_DIR/scripts/download-sparkle.sh"
    fi
}

sparkle_public_key() {
    "$ROOT_DIR/sparkle-bin/generate_keys" --account "$ACCOUNT" >/dev/null

    local output
    output="$("$ROOT_DIR/sparkle-bin/generate_keys" --account "$ACCOUNT" -p)"
    printf "%s\n" "$output" |
        sed -n 's/.*SUPublicEDKey="\([^"]*\)".*/\1/p; s/.*SUPublicEDKey: *\([^ ]*\).*/\1/p; s|.*<string>\([^<]*\)</string>.*|\1|p; /^[A-Za-z0-9+\/][A-Za-z0-9+\/=]*=$/p' |
        head -n 1
}

patch_app() {
    local app="$1"
    local version="$2"
    local build="$3"
    local public_key="$4"
    local feed_url="$5"
    local plist="$app/Contents/Info.plist"

    [ -f "$plist" ] || die "Info.plist missing: $plist"
    mkdir -p "$app/Contents/Frameworks"
    ditto "$ROOT_DIR/Sparkle.framework" "$app/Contents/Frameworks/Sparkle.framework"

    plutil -replace CFBundleShortVersionString -string "$version" "$plist"
    plutil -replace CFBundleVersion -string "$build" "$plist"
    plutil -replace SUFeedURL -string "$feed_url" "$plist"
    plutil -replace SUPublicEDKey -string "$public_key" "$plist"

    codesign --force --deep --sign - "$app"
}

reset_sparkle_defaults() {
    local app="$1"
    local bundle_id

    bundle_id="$(plutil -extract CFBundleIdentifier raw -o - "$app/Contents/Info.plist")"
    for key in SULastCheckTime SUSkippedVersion SULastRemindLaterDate SUUpdateRelaunchingMarker; do
        defaults delete "$bundle_id" "$key" >/dev/null 2>&1 || true
    done
}

write_appcast() {
    local output_zip="$1"
    local signature="$2"
    local length="$3"
    local appcast="$4"
    local pub_date

    pub_date="$(date -uR)"
    cat > "$appcast" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Slint Visual Editor Local Updates</title>
    <link>http://$HOST:$PORT/appcast.xml</link>
    <description>Local Slint Visual Editor Sparkle updates</description>
    <item>
      <title>Slint Visual Editor $VERSION ($NEW_BUILD)</title>
      <pubDate>$pub_date</pubDate>
      <sparkle:version>$NEW_BUILD</sparkle:version>
      <sparkle:shortVersionString>$VERSION</sparkle:shortVersionString>
      <enclosure
        url="http://$HOST:$PORT/$(basename "$output_zip")"
        sparkle:edSignature="$signature"
        length="$length"
        type="application/octet-stream" />
    </item>
  </channel>
</rss>
EOF
}

ensure_sparkle_tools

PUBLIC_KEY="$(sparkle_public_key)"
[ -n "$PUBLIC_KEY" ] || die "could not read Sparkle public key for account $ACCOUNT"

OLD_STAGE="$WORK_DIR/old/$APP_NAME.app"
NEW_STAGE="$WORK_DIR/new/$APP_NAME.app"
INSTALL_APP="$WORK_DIR/install/$APP_NAME.app"
WEB_DIR="$WORK_DIR/web"
UPDATE_ZIP="$WEB_DIR/SlintVisualEditor-$VERSION-$NEW_BUILD-macos-arm64.zip"
APPCAST="$WEB_DIR/appcast.xml"
SERVER_LOG="$WORK_DIR/http-server.log"
FEED_URL="http://$HOST:$PORT/appcast.xml"

log "Preparing $WORK_DIR"
rm -rf "$WORK_DIR/old" "$WORK_DIR/new" "$WORK_DIR/install" "$WORK_DIR/web"
mkdir -p "$WORK_DIR/old" "$WORK_DIR/new" "$WORK_DIR/install" "$WEB_DIR"

log "Copying app bundles"
ditto "$OLD_APP" "$OLD_STAGE"
ditto "$NEW_APP" "$NEW_STAGE"

log "Patching and ad-hoc signing old app as $VERSION ($OLD_BUILD)"
patch_app "$OLD_STAGE" "$VERSION" "$OLD_BUILD" "$PUBLIC_KEY" "$FEED_URL"

log "Patching and ad-hoc signing update app as $VERSION ($NEW_BUILD)"
patch_app "$NEW_STAGE" "$VERSION" "$NEW_BUILD" "$PUBLIC_KEY" "$FEED_URL"

log "Creating update ZIP"
ditto -c -k --sequesterRsrc --keepParent "$NEW_STAGE" "$UPDATE_ZIP"

log "Signing update ZIP"
SIGNATURE_OUTPUT="$("$ROOT_DIR/sparkle-bin/sign_update" --account "$ACCOUNT" "$UPDATE_ZIP" 2>&1)"
ED_SIGNATURE="$(echo "$SIGNATURE_OUTPUT" | sed -n 's/.*sparkle:edSignature="\([^"]*\)".*/\1/p')"
LENGTH="$(echo "$SIGNATURE_OUTPUT" | sed -n 's/.*length="\([0-9][0-9]*\)".*/\1/p')"
[ -n "$ED_SIGNATURE" ] || die "sign_update did not print sparkle:edSignature"
[ -n "$LENGTH" ] || die "sign_update did not print length"

log "Writing local appcast"
write_appcast "$UPDATE_ZIP" "$ED_SIGNATURE" "$LENGTH" "$APPCAST"

log "Installing old app copy"
ditto "$OLD_STAGE" "$INSTALL_APP"
reset_sparkle_defaults "$INSTALL_APP"

log "Starting local HTTP server on http://$HOST:$PORT/"
python3 -m http.server "$PORT" --bind "$HOST" --directory "$WEB_DIR" > "$SERVER_LOG" 2>&1 &
SERVER_PID="$!"
sleep 1
kill -0 "$SERVER_PID" >/dev/null 2>&1 || die "HTTP server failed to start; see $SERVER_LOG"

cat <<EOF

Local Sparkle feed is ready:
  $APPCAST
  $UPDATE_ZIP

Installed old app copy:
  $INSTALL_APP

Launching with LaunchServices now. This is the path closest to double-clicking
the app in Finder:

  open "$INSTALL_APP"

When Sparkle appears, click Install and Relaunch.
Press Return here after the app relaunches, or after you decide the update UI
did not appear.
EOF

open "$INSTALL_APP"
read -r _

INSTALLED_VERSION="$(plutil -extract CFBundleShortVersionString raw -o - "$INSTALL_APP/Contents/Info.plist")"
INSTALLED_BUILD="$(plutil -extract CFBundleVersion raw -o - "$INSTALL_APP/Contents/Info.plist")"

cat <<EOF

Installed app version after test:
  CFBundleShortVersionString=$INSTALLED_VERSION
  CFBundleVersion=$INSTALLED_BUILD

Expected after a successful update:
  CFBundleShortVersionString=$VERSION
  CFBundleVersion=$NEW_BUILD

HTTP server log:
  $SERVER_LOG
EOF

if [ "$INSTALLED_VERSION" = "$VERSION" ] && [ "$INSTALLED_BUILD" = "$NEW_BUILD" ]; then
    log "Sparkle update succeeded"
else
    die "Sparkle update did not replace the installed app"
fi
