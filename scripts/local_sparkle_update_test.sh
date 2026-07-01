#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
. "$SCRIPT_DIR/helpers.sh"

APP_NAME="Slint Visual Editor"
ACCOUNT="slint-visual-editor-local-test"
BUNDLE_IDENTIFIER="dev.slint.visual-editor.sparkle-test"
ICON_SOURCE="$ROOT_DIR/tools/lsp/packaging/macos/AppIcon.icon"
RUST_TARGET="aarch64-apple-darwin"
CARGO_FEATURES="backend-winit,renderer-skia"
HOST="127.0.0.1"
PORT="8765"
WORK_DIR="/private/tmp/slint-sparkle-local-test"
OLD_BUILD="100"
NEW_BUILD="101"
VERSION=""
OLD_APP=""
NEW_APP=""
BUILD_EDITOR=0
SERVER_PID=""

usage() {
    cat <<EOF
Usage:
  $0 --build-editor
  $0 --old-app "/path/to/old/Slint Visual Editor.app" --new-app "/path/to/new/Slint Visual Editor.app"

Options:
  --build-editor         Build a local Visual Editor app first.
  --version VERSION       App version. Defaults to tools/lsp/Cargo.toml.
  --old-build BUILD      CFBundleVersion for the installed old app. Default: $OLD_BUILD
  --new-build BUILD      CFBundleVersion for the update app. Default: $NEW_BUILD
  --account ACCOUNT      Sparkle keychain account. Default: $ACCOUNT
  --bundle-id ID         Bundle id for temp app copies. Default: $BUNDLE_IDENTIFIER
  --port PORT            Local HTTP port. Default: $PORT
  --work-dir DIR         Temp working directory. Default: $WORK_DIR
  -h, --help             Show this help.

Creates a local appcast, launches the old app copy, and waits while you trigger
the update in the Visual Editor.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --build-editor)
            BUILD_EDITOR=1
            shift
            ;;
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
        --bundle-id)
            BUNDLE_IDENTIFIER="${2:-}"
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
[ -n "$WORK_DIR" ] || die "--work-dir must not be empty"
[ -n "$BUNDLE_IDENTIFIER" ] || die "--bundle-id must not be empty"
case "$WORK_DIR" in
    /tmp/* | /private/tmp/*) ;;
    *) die "--work-dir must be under /tmp or /private/tmp" ;;
esac

if [ "$BUILD_EDITOR" -eq 1 ]; then
    [ -z "$OLD_APP" ] || die "--build-editor cannot be combined with --old-app"
    [ -z "$NEW_APP" ] || die "--build-editor cannot be combined with --new-app"
else
    [ -n "$OLD_APP" ] || die "--old-app is required unless --build-editor is used"
    [ -n "$NEW_APP" ] || die "--new-app is required unless --build-editor is used"
    [ -d "$OLD_APP" ] || die "old app not found: $OLD_APP"
    [ -d "$NEW_APP" ] || die "new app not found: $NEW_APP"
fi

for tool in codesign ditto open plutil python3; do
    command -v "$tool" >/dev/null || die "$tool is required"
done

if [ -z "$VERSION" ]; then
    command -v cargo >/dev/null || die "cargo is required to determine the Visual Editor version"
    command -v jq >/dev/null || die "jq is required to determine the Visual Editor version"
    VERSION="$(cargo metadata --format-version 1 --no-deps --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r 'first(.packages[] | select(.name == "slint-lsp") | .version)')" \
        || die "could not determine Visual Editor version from Cargo metadata"
fi
[ -n "$VERSION" ] && [ "$VERSION" != "null" ] || die "could not determine Visual Editor version from Cargo metadata"

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

build_editor_app() {
    [ "$(uname -m)" = "arm64" ] || die "--build-editor currently builds and runs the arm64 macOS app"

    command -v cargo >/dev/null || die "cargo is required for --build-editor"
    command -v xcrun >/dev/null || die "xcrun is required for --build-editor"
    if command -v rustup >/dev/null &&
        ! rustup target list --installed | grep -qx "$RUST_TARGET"; then
        die "Rust target $RUST_TARGET is required; run: rustup target add $RUST_TARGET"
    fi

    local build_dir="$WORK_DIR/build"
    local app="$build_dir/$APP_NAME.app"
    local executable="$ROOT_DIR/target/$RUST_TARGET/release/slint-visual-editor"
    local plist="$app/Contents/Info.plist"

    log "Building local Visual Editor binary for $RUST_TARGET"
    env \
        SPARKLE_FRAMEWORK_DIR="$ROOT_DIR" \
        RUSTFLAGS="-Clink-args=-Wl,-rpath,@loader_path/../Frameworks" \
        cargo build \
            --release \
            --target "$RUST_TARGET" \
            --bin slint-visual-editor \
            --no-default-features \
            --features "$CARGO_FEATURES"

    [ -x "$executable" ] || die "built executable missing: $executable"

    log "Assembling temporary app bundle at $app"
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$executable" "$app/Contents/MacOS/$APP_NAME"
    chmod +x "$app/Contents/MacOS/$APP_NAME"
    compile_app_icon "$app"

    cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>$APP_NAME</string>
  <key>CFBundleExecutable</key>
  <string>$APP_NAME</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIconName</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_IDENTIFIER</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleVersion</key>
  <string>$OLD_BUILD</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>SUFeedURL</key>
  <string>https://visual-editor.slint.dev/appcast.xml</string>
  <key>SUPublicEDKey</key>
  <string>$PUBLIC_KEY</string>
</dict>
</plist>
EOF

    BUILT_EDITOR_APP="$app"
}

compile_app_icon() {
    local app="$1"
    local icon_dir="$WORK_DIR/icon"
    local out_dir="$icon_dir/out"
    local partial_dir="$icon_dir/partial"

    [ -d "$ICON_SOURCE" ] || die "app icon source missing: $ICON_SOURCE"

    log "Compiling app icon"
    rm -rf "$icon_dir"
    mkdir -p "$out_dir" "$partial_dir"
    xcrun actool \
        --compile "$out_dir" \
        --platform macosx \
        --minimum-deployment-target 12.0 \
        --app-icon AppIcon \
        --output-partial-info-plist "$partial_dir/Info.plist" \
        "$ICON_SOURCE" >/dev/null

    [ -f "$out_dir/AppIcon.icns" ] || die "actool did not produce AppIcon.icns"
    [ -f "$out_dir/Assets.car" ] || die "actool did not produce Assets.car"
    ditto "$out_dir/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
    ditto "$out_dir/Assets.car" "$app/Contents/Resources/Assets.car"
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

    plutil -replace CFBundleIdentifier -string "$BUNDLE_IDENTIFIER" "$plist"
    plutil -replace CFBundleShortVersionString -string "$version" "$plist"
    plutil -replace CFBundleVersion -string "$build" "$plist"
    plutil -replace SUFeedURL -string "$feed_url" "$plist"
    plutil -replace SUPublicEDKey -string "$public_key" "$plist"

    xattr -dr com.apple.quarantine "$app" >/dev/null 2>&1 || true
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
    local signature_output="$2"
    local appcast="$3"
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
        $signature_output
        type="application/octet-stream" />
    </item>
  </channel>
</rss>
EOF
}

ensure_sparkle_tools

PUBLIC_KEY="$(sparkle_public_key)"
[ -n "$PUBLIC_KEY" ] || die "could not read Sparkle public key for account $ACCOUNT"

if [ "$BUILD_EDITOR" -eq 1 ]; then
    build_editor_app
    OLD_APP="$BUILT_EDITOR_APP"
    NEW_APP="$BUILT_EDITOR_APP"
fi

OLD_APP="$(abs_path "$OLD_APP")"
NEW_APP="$(abs_path "$NEW_APP")"

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

log "Writing local appcast"
write_appcast "$UPDATE_ZIP" "$SIGNATURE_OUTPUT" "$APPCAST"

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

Launching:

  open -n "$INSTALL_APP"

In the Visual Editor, click Update once to download and again to install.
Press Return here after the app relaunches.
EOF

open -n "$INSTALL_APP"
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
