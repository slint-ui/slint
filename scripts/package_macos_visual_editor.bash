#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -euo pipefail

APP_NAME="Slint Visual Editor"
DMG_BASENAME="SlintVisualEditor"
SCHEME="Slint Visual Editor"

die() {
    echo "error: $*" >&2
    exit 1
}

require_env() {
    local name
    for name in "$@"; do
        if [ -z "${!name:-}" ]; then
            die "$name is required"
        fi
    done
}

abs_path() {
    local path="$1"
    local dir

    dir="$(cd "$(dirname "$path")" && pwd)" || return 1
    printf "%s/%s\n" "$dir" "$(basename "$path")"
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPEC_PATH="$ROOT_DIR/tools/lsp/macos-project.yml"
PROJECT_DIR="$(dirname "$SPEC_PATH")"
PROJECT_FILE="$PROJECT_DIR/$APP_NAME.xcodeproj"
VERSION="${VERSION:-}"

if [ -z "$VERSION" ]; then
    VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$PROJECT_DIR/Cargo.toml" | head -n 1)"
fi
VERSION="${VERSION:-0.0.0}"

BUILD_NUMBER="${BUILD_NUMBER:-${GITHUB_RUN_NUMBER:-$(git -C "$ROOT_DIR" rev-list --count HEAD 2>/dev/null || echo 1)}}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
BUILD_DIR="${BUILD_DIR:-$ROOT_DIR/target/macos-visual-editor-dmg}"
RUNNER_TEMP_DIR="${RUNNER_TEMP:-$BUILD_DIR/secrets}"
DERIVED_DATA_PATH="$BUILD_DIR/DerivedData"
ARCHIVE_PATH="$BUILD_DIR/$APP_NAME.xcarchive"
STAGE_DIR="$BUILD_DIR/dmg-stage"
MOUNT_DIR="$BUILD_DIR/dmg-mount"
APP_PATH="$ARCHIVE_PATH/Products/Applications/$APP_NAME.app"
STAGED_APP_PATH="$STAGE_DIR/$APP_NAME.app"
DMG_PATH="$DIST_DIR/$DMG_BASENAME-$VERSION-macos-universal.dmg"
NOTARY_LOG="$BUILD_DIR/notarization-log.json"
KEYCHAIN_PATH="$RUNNER_TEMP_DIR/visual-editor-signing.keychain-db"
CERTIFICATE_PATH="$RUNNER_TEMP_DIR/developer-id-application.p12"
NOTARY_KEY_PATH="$RUNNER_TEMP_DIR/AuthKey_${NOTARY_API_KEY_ID:-unset}.p8"
DMG_ATTACHED=0

cleanup() {
    if [ "$DMG_ATTACHED" -eq 1 ]; then
        hdiutil detach "$MOUNT_DIR" >/dev/null 2>&1 || true
    fi
    if [ -f "$KEYCHAIN_PATH" ]; then
        security delete-keychain "$KEYCHAIN_PATH" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

require_env \
    MACOS_CERTIFICATE_BASE64 \
    MACOS_CERTIFICATE_PASSWORD \
    MACOS_KEYCHAIN_PASSWORD \
    MACOS_DEVELOPER_ID \
    MACOS_DEVELOPMENT_TEAM \
    MACOS_BUNDLE_IDENTIFIER \
    NOTARY_API_KEY_BASE64 \
    NOTARY_API_KEY_ID \
    NOTARY_ISSUER_ID

mkdir -p "$DIST_DIR" "$BUILD_DIR" "$RUNNER_TEMP_DIR"
chmod 700 "$RUNNER_TEMP_DIR"

install_signing_material() {
    # Source: GitHub documents importing Apple signing certificates into a
    # temporary macOS runner keychain:
    # https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications
    printf "%s" "$MACOS_CERTIFICATE_BASE64" | base64 --decode -o "$CERTIFICATE_PATH"
    printf "%s" "$NOTARY_API_KEY_BASE64" | base64 --decode -o "$NOTARY_KEY_PATH"
    chmod 600 "$CERTIFICATE_PATH" "$NOTARY_KEY_PATH"

    # Source: security is the command line interface to macOS keychains:
    # https://keith.github.io/xcode-man-pages/security.1.html
    security create-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
    security set-keychain-settings -lut 21600 "$KEYCHAIN_PATH"
    security unlock-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
    security import "$CERTIFICATE_PATH" \
        -P "$MACOS_CERTIFICATE_PASSWORD" \
        -A \
        -t cert \
        -f pkcs12 \
        -k "$KEYCHAIN_PATH"
    security set-key-partition-list \
        -S apple-tool:,apple:,codesign: \
        -s \
        -k "$MACOS_KEYCHAIN_PASSWORD" \
        "$KEYCHAIN_PATH"
    security default-keychain -s "$KEYCHAIN_PATH"
}

archive_app() {
    # Source: XcodeGen generates an Xcode project from a YAML project spec:
    # https://yonaskolb.github.io/XcodeGen/Docs/ProjectSpec.html
    xcodegen generate --spec "$SPEC_PATH"

    rm -rf "$DERIVED_DATA_PATH" "$ARCHIVE_PATH"

    # Source: xcodebuild archive builds an archive at the provided archive path:
    # https://keith.github.io/xcode-man-pages/xcodebuild.1.html
    xcodebuild archive \
        -project "$PROJECT_FILE" \
        -scheme "$SCHEME" \
        -configuration Release \
        -destination "generic/platform=macOS" \
        -derivedDataPath "$DERIVED_DATA_PATH" \
        -archivePath "$ARCHIVE_PATH" \
        ARCHS="arm64 x86_64" \
        ONLY_ACTIVE_ARCH=NO \
        SKIP_INSTALL=NO \
        CODE_SIGNING_ALLOWED=NO \
        MARKETING_VERSION="$VERSION" \
        CURRENT_PROJECT_VERSION="$BUILD_NUMBER"

    [ -d "$APP_PATH" ] || die "archive did not produce $APP_PATH"
}

stage_and_sign_app() {
    rm -rf "$STAGE_DIR"
    mkdir -p "$STAGE_DIR"
    ditto "$APP_PATH" "$STAGED_APP_PATH"
    ln -s /Applications "$STAGE_DIR/Applications"

    local executable="$STAGED_APP_PATH/Contents/MacOS/$APP_NAME"
    [ -x "$executable" ] || die "app executable missing: $executable"

    # Source: codesign signs code and supports hardened runtime via
    # --options runtime and timestamping via --timestamp:
    # https://keith.github.io/xcode-man-pages/codesign.1.html
    codesign --force --options runtime --timestamp \
        --sign "$MACOS_DEVELOPER_ID" \
        "$executable"
    codesign --force --options runtime --timestamp \
        --sign "$MACOS_DEVELOPER_ID" \
        "$STAGED_APP_PATH"
    codesign --verify --deep --strict --verbose=2 "$STAGED_APP_PATH"
}

create_and_sign_dmg() {
    rm -f "$DMG_PATH"

    # Source: hdiutil create can build a UDIF disk image from a source folder:
    # https://keith.github.io/xcode-man-pages/hdiutil.1.html
    hdiutil create \
        -volname "$APP_NAME" \
        -srcfolder "$STAGE_DIR" \
        -format UDZO \
        -ov \
        "$DMG_PATH"

    codesign --force --timestamp --sign "$MACOS_DEVELOPER_ID" "$DMG_PATH"
    hdiutil verify "$DMG_PATH"
    codesign --verify --strict --verbose=2 "$DMG_PATH"
}

notarize_and_staple_dmg() {
    rm -f "$NOTARY_LOG"

    # Source: notarytool submits UDIF disk images to Apple's notary service and
    # supports App Store Connect API key authentication:
    # https://keith.github.io/xcode-man-pages/notarytool.1.html
    xcrun notarytool submit "$DMG_PATH" \
        --key "$NOTARY_KEY_PATH" \
        --key-id "$NOTARY_API_KEY_ID" \
        --issuer "$NOTARY_ISSUER_ID" \
        --wait \
        --output-format json \
        > "$NOTARY_LOG"

    local status
    status="$(plutil -extract status raw -o - "$NOTARY_LOG" 2>/dev/null || true)"
    if [ "$status" != "Accepted" ]; then
        cat "$NOTARY_LOG" >&2
        die "notarization status was ${status:-unknown}, expected Accepted"
    fi

    # Source: stapler attaches and validates the notarization ticket:
    # https://keith.github.io/xcode-man-pages/stapler.1.html
    xcrun stapler staple "$DMG_PATH"
    xcrun stapler validate "$DMG_PATH"
}

assess_stapled_app() {
    rm -rf "$MOUNT_DIR"
    mkdir -p "$MOUNT_DIR"

    hdiutil attach "$DMG_PATH" \
        -readonly \
        -nobrowse \
        -mountpoint "$MOUNT_DIR"
    DMG_ATTACHED=1

    # Source: spctl assesses code against system security policy:
    # https://keith.github.io/xcode-man-pages/spctl.8.html
    spctl -a -vv -t exec "$MOUNT_DIR/$APP_NAME.app"

    hdiutil detach "$MOUNT_DIR"
    DMG_ATTACHED=0
    rm -rf "$MOUNT_DIR"
}

install_signing_material
archive_app
stage_and_sign_app
create_and_sign_dmg
notarize_and_staple_dmg
assess_stapled_app

echo "Created $DMG_PATH"
