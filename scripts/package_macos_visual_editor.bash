#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
. "$SCRIPT_DIR/helpers.sh"

APP_NAME="Slint Visual Editor"
DMG_BASENAME="SlintVisualEditor"
SCHEME="Slint Visual Editor"

SPEC_PATH="$ROOT_DIR/tools/lsp/macos-project.yml"
PROJECT_DIR="$(dirname "$SPEC_PATH")"
PROJECT_FILE="$PROJECT_DIR/$APP_NAME.xcodeproj"
EXAMPLE_SOURCE_DIR="$PROJECT_DIR/ui/visual-editor/example"
VERSION="${VERSION:-}"

if [ -z "$VERSION" ]; then
    command -v cargo >/dev/null || die "cargo is required to determine the Visual Editor version"
    command -v jq >/dev/null || die "jq is required to determine the Visual Editor version"
    VERSION="$(cargo metadata --format-version 1 --no-deps --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r 'first(.packages[] | select(.name == "slint-lsp") | .version)')" \
        || die "could not determine Visual Editor version from Cargo metadata"
fi
[ -n "$VERSION" ] && [ "$VERSION" != "null" ] || die "could not determine Visual Editor version from Cargo metadata"

BUILD_NUMBER="${BUILD_NUMBER:-${SLINT_BUILD_NUMBER:-${GITHUB_RUN_NUMBER:-$(git -C "$ROOT_DIR" rev-list --count HEAD 2>/dev/null || echo 1)}}}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
BUILD_DIR="${BUILD_DIR:-$ROOT_DIR/target/macos-visual-editor-dmg}"
RUNNER_TEMP_DIR="${RUNNER_TEMP:-$BUILD_DIR/secrets}"
DERIVED_DATA_PATH="$BUILD_DIR/DerivedData"
ARCHIVE_PATH="$BUILD_DIR/$APP_NAME.xcarchive"
STAGE_DIR="$BUILD_DIR/dmg-stage"
MOUNT_DIR="$BUILD_DIR/dmg-mount"
APP_PATH="$ARCHIVE_PATH/Products/Applications/$APP_NAME.app"
STAGED_APP_PATH="$STAGE_DIR/$APP_NAME.app"
DMG_PATH="${DMG_PATH:-$DIST_DIR/$DMG_BASENAME-$VERSION-macos-arm64.dmg}"
DMG_BACKGROUND_SOURCE="$PROJECT_DIR/packaging/macos/dmg-background.svg"
NOTARY_LOG="$BUILD_DIR/notarization-log.json"
KEYCHAIN_PATH="$RUNNER_TEMP_DIR/visual-editor-signing.keychain-db"
CERTIFICATE_PATH="$RUNNER_TEMP_DIR/developer-id-application.p12"
NOTARY_KEY_PATH="$RUNNER_TEMP_DIR/AuthKey_${NOTARY_API_KEY_ID:-unset}.p8"
CARGO_XCODE_TARGET_DIR="$ROOT_DIR/target/xcode-cargo/slint-visual-editor"
CARGO_TIMINGS_REPORT_DIR="$BUILD_DIR/cargo-timings"
CLOUDFLARE_ROOT_DIR="$DIST_DIR/cloudflare-root"
SPARKLE_UPDATE_BASENAME="$DMG_BASENAME-$VERSION-$BUILD_NUMBER-macos-arm64.zip"
SPARKLE_UPDATE_PATH="$CLOUDFLARE_ROOT_DIR/$SPARKLE_UPDATE_BASENAME"
SPARKLE_APPCAST_PATH="$CLOUDFLARE_ROOT_DIR/appcast.xml"
SPARKLE_FEED_BASE_URL="https://visual-editor.slint.dev"
export EDITOR_SPARKLE_PUBLIC_ED_KEY="${EDITOR_SPARKLE_PUBLIC_ED_KEY:-Ncon335q8qNLM0D+L2my+HRIAXmNtNb6uGNmUR0yG2o=}"
# Consumed by xcodegen via tools/lsp/macos-project.yml.
export MACOS_BUNDLE_IDENTIFIER="${MACOS_BUNDLE_IDENTIFIER:-dev.slint.visual-editor}"
APP_NOTARY_ZIP_PATH="$BUILD_DIR/$DMG_BASENAME-$VERSION-$BUILD_NUMBER-macos-arm64-notary.zip"
APP_NOTARY_LOG="$BUILD_DIR/app-notarization-log.json"
DMG_ATTACHED=0

cleanup() {
    if [ "$DMG_ATTACHED" -eq 1 ]; then
        hdiutil detach "$MOUNT_DIR" >/dev/null 2>&1 || true
    fi
    if [ -f "$KEYCHAIN_PATH" ]; then
        security delete-keychain "$KEYCHAIN_PATH" >/dev/null 2>&1 || true
    fi
}

detach_dmg() {
    if [ "$DMG_ATTACHED" -eq 1 ]; then
        log "Detaching DMG"
        hdiutil detach "$MOUNT_DIR"
        DMG_ATTACHED=0
    fi
    rm -rf "$MOUNT_DIR"
}

collect_rust_build_report() {
    rm -rf "$CARGO_TIMINGS_REPORT_DIR"

    if [ -d "$CARGO_XCODE_TARGET_DIR/cargo-timings" ]; then
        log "Collecting Cargo timing report"
        mkdir -p "$CARGO_TIMINGS_REPORT_DIR"
        ditto "$CARGO_XCODE_TARGET_DIR/cargo-timings" "$CARGO_TIMINGS_REPORT_DIR"
    elif [ "${MACOS_CARGO_TIMINGS:-0}" != "0" ]; then
        die "Cargo timings were enabled, but no report was found at $CARGO_XCODE_TARGET_DIR/cargo-timings"
    fi
}

mkdir -p "$DIST_DIR" "$BUILD_DIR" "$RUNNER_TEMP_DIR"
chmod 700 "$RUNNER_TEMP_DIR"

validate_environment() {
    require_env \
        MACOS_CERTIFICATE_BASE64 \
        MACOS_CERTIFICATE_PASSWORD \
        MACOS_KEYCHAIN_PASSWORD \
        MACOS_DEVELOPER_ID \
        NOTARY_API_KEY_BASE64 \
        NOTARY_API_KEY_ID \
        NOTARY_ISSUER_ID
}

install_signing_material() {
    log "Installing signing material into temporary keychain"
    # Source: GitHub documents importing Apple signing certificates into a
    # temporary macOS runner keychain:
    # https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications
    log "Decoding Developer ID certificate and notary API key"
    printf "%s" "$MACOS_CERTIFICATE_BASE64" | base64 --decode -o "$CERTIFICATE_PATH"
    printf "%s" "$NOTARY_API_KEY_BASE64" | base64 --decode -o "$NOTARY_KEY_PATH"
    chmod 600 "$CERTIFICATE_PATH" "$NOTARY_KEY_PATH"

    # Source: security is the command line interface to macOS keychains:
    # https://keith.github.io/xcode-man-pages/security.1.html
    log "Creating temporary keychain: $KEYCHAIN_PATH"
    security create-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
    log "Configuring keychain timeout"
    security set-keychain-settings -lut 21600 "$KEYCHAIN_PATH"
    log "Unlocking temporary keychain"
    security unlock-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
    log "Importing Developer ID certificate"
    security import "$CERTIFICATE_PATH" \
        -P "$MACOS_CERTIFICATE_PASSWORD" \
        -A \
        -t cert \
        -f pkcs12 \
        -k "$KEYCHAIN_PATH"
    log "Allowing codesign to access imported key"
    security set-key-partition-list \
        -S apple-tool:,apple:,codesign: \
        -s \
        -k "$MACOS_KEYCHAIN_PASSWORD" \
        "$KEYCHAIN_PATH"
    log "Selecting temporary keychain as default"
    security default-keychain -s "$KEYCHAIN_PATH"
    log "Signing material installed"
}

# The team ID is not secret (it is embedded in every signed app), so it is
# derived from the imported Developer ID certificate instead of being
# provisioned as a separate secret. MACOS_DEVELOPMENT_TEAM overrides.
resolve_development_team() {
    if [ -n "${MACOS_DEVELOPMENT_TEAM:-}" ]; then
        return
    fi
    [ -f "$KEYCHAIN_PATH" ] || die "MACOS_DEVELOPMENT_TEAM is not set and there is no signing keychain to derive it from; run install-signing-material first"
    local identity
    identity="$(security find-identity -v -p codesigning "$KEYCHAIN_PATH" | grep -m 1 "Developer ID Application:")" \
        || die "no Developer ID Application identity found in $KEYCHAIN_PATH"
    MACOS_DEVELOPMENT_TEAM="$(printf "%s" "$identity" | sed -n 's/.*(\([A-Z0-9]\{10\}\))".*/\1/p')"
    [ -n "$MACOS_DEVELOPMENT_TEAM" ] || die "could not extract a team ID from identity: $identity"
    export MACOS_DEVELOPMENT_TEAM
    log "Derived development team from signing certificate: $MACOS_DEVELOPMENT_TEAM"
}

archive_app() {
    resolve_development_team

    log "Generating Xcode project from $SPEC_PATH"
    # Source: XcodeGen generates an Xcode project from a YAML project spec:
    # https://yonaskolb.github.io/XcodeGen/Docs/ProjectSpec.html
    xcodegen generate --spec "$SPEC_PATH"

    rm -rf "$DERIVED_DATA_PATH" "$ARCHIVE_PATH"

    log "Archiving app with xcodebuild into $ARCHIVE_PATH"
    # Source: xcodebuild archive builds an archive at the provided archive path:
    # https://keith.github.io/xcode-man-pages/xcodebuild.1.html
    export NSUnbufferedIO=YES
    xcodebuild archive \
        -project "$PROJECT_FILE" \
        -scheme "$SCHEME" \
        -configuration Release \
        -destination "generic/platform=macOS" \
        -derivedDataPath "$DERIVED_DATA_PATH" \
        -archivePath "$ARCHIVE_PATH" \
        ARCHS="arm64" \
        ONLY_ACTIVE_ARCH=NO \
        SKIP_INSTALL=NO \
        CODE_SIGNING_ALLOWED=NO \
        MARKETING_VERSION="$VERSION" \
        CURRENT_PROJECT_VERSION="$BUILD_NUMBER" \
        -showBuildTimingSummary

    [ -d "$APP_PATH" ] || die "archive did not produce $APP_PATH"
    log "Archive produced $APP_PATH"
}

stage_and_sign_app() {
    log "Staging app bundle for DMG"
    rm -rf "$STAGE_DIR"
    mkdir -p "$STAGE_DIR"
    ditto "$APP_PATH" "$STAGED_APP_PATH"
    mkdir -p "$STAGED_APP_PATH/Contents/Resources/visual-editor-example"
    ditto "$EXAMPLE_SOURCE_DIR" "$STAGED_APP_PATH/Contents/Resources/visual-editor-example"
    ditto "$SPARKLE_FRAMEWORK_DIR/Sparkle.framework" "$STAGED_APP_PATH/Contents/Frameworks/Sparkle.framework"

    local executable="$STAGED_APP_PATH/Contents/MacOS/$APP_NAME"
    [ -x "$executable" ] || die "app executable missing: $executable"

    # Source: codesign signs code and supports hardened runtime via
    # --options runtime and timestamping via --timestamp:
    # https://keith.github.io/xcode-man-pages/codesign.1.html
    log "Signing app bundle"
    codesign --force --deep --options runtime --timestamp \
        --sign "$MACOS_DEVELOPER_ID" \
        "$STAGED_APP_PATH"
    log "Verifying app bundle signature"
    codesign --verify --deep --strict --verbose=2 "$STAGED_APP_PATH"
    log "App bundle signed and verified"

    log "Freeing Xcode and Cargo build intermediates before DMG creation"
    df -h "$ROOT_DIR"
    collect_rust_build_report
    rm -rf "$ARCHIVE_PATH" "$DERIVED_DATA_PATH" "$CARGO_XCODE_TARGET_DIR"
    df -h "$ROOT_DIR"
}

create_dmg() {
    [ -d "$STAGED_APP_PATH" ] || die "staged app missing: $STAGED_APP_PATH"
    [ -f "$DMG_BACKGROUND_SOURCE" ] || die "DMG background missing: $DMG_BACKGROUND_SOURCE"
    command -v create-dmg >/dev/null || die "create-dmg is required; install it with Homebrew"

    log "Creating DMG at $DMG_PATH"
    rm -f "$DMG_PATH"

    create-dmg \
        --volname "$APP_NAME" \
        --background "$DMG_BACKGROUND_SOURCE" \
        --window-pos 200 120 \
        --window-size 660 420 \
        --icon-size 112 \
        --icon "$APP_NAME.app" 184 278 \
        --app-drop-link 474 278 \
        --no-internet-enable \
        "$DMG_PATH" \
        "$STAGED_APP_PATH"
}

sign_dmg() {
    [ -f "$DMG_PATH" ] || die "DMG missing: $DMG_PATH"
    log "Signing DMG"
    codesign --force --timestamp --sign "$MACOS_DEVELOPER_ID" "$DMG_PATH"
    verify_dmg_payload "signed"
    log "DMG signed and verified"
}

verify_dmg_payload() {
    local label="$1"
    local verify_status=0

    log "Verifying $label DMG structure"
    hdiutil verify "$DMG_PATH"
    log "Verifying $label DMG signature"
    codesign --verify --strict --verbose=2 "$DMG_PATH"

    log "Mounting $label DMG to verify app payload"
    rm -rf "$MOUNT_DIR"
    mkdir -p "$MOUNT_DIR"
    hdiutil attach "$DMG_PATH" \
        -readonly \
        -nobrowse \
        -mountpoint "$MOUNT_DIR"
    DMG_ATTACHED=1

    log "Verifying $label mounted app code signature"
    codesign --verify --deep --strict --verbose=2 "$MOUNT_DIR/$APP_NAME.app" || verify_status=$?
    detach_dmg
    return "$verify_status"
}

notarize_and_staple_dmg() {
    rm -f "$NOTARY_LOG"

    log "Submitting DMG for notarization"
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

    log "Reading notarization result from $NOTARY_LOG"
    local status
    status="$(plutil -extract status raw -o - "$NOTARY_LOG" 2>/dev/null || true)"
    if [ "$status" != "Accepted" ]; then
        cat "$NOTARY_LOG" >&2
        die "notarization status was ${status:-unknown}, expected Accepted"
    fi

    # Source: stapler attaches and validates the notarization ticket:
    # https://keith.github.io/xcode-man-pages/stapler.1.html
    log "Stapling notarization ticket"
    xcrun stapler staple "$DMG_PATH"
    log "Validating stapled notarization ticket"
    xcrun stapler validate "$DMG_PATH"
    verify_dmg_payload "stapled"
    log "DMG notarized and stapled"
}

notarize_and_staple_app() {
    [ -d "$STAGED_APP_PATH" ] || die "staged app missing: $STAGED_APP_PATH"
    rm -f "$APP_NOTARY_ZIP_PATH" "$APP_NOTARY_LOG"

    log "Creating temporary app ZIP for notarization"
    ditto -c -k --sequesterRsrc --keepParent "$STAGED_APP_PATH" "$APP_NOTARY_ZIP_PATH"

    log "Submitting app ZIP for notarization"
    xcrun notarytool submit "$APP_NOTARY_ZIP_PATH" \
        --key "$NOTARY_KEY_PATH" \
        --key-id "$NOTARY_API_KEY_ID" \
        --issuer "$NOTARY_ISSUER_ID" \
        --wait \
        --output-format json \
        > "$APP_NOTARY_LOG"

    log "Reading app notarization result from $APP_NOTARY_LOG"
    local status
    status="$(plutil -extract status raw -o - "$APP_NOTARY_LOG" 2>/dev/null || true)"
    if [ "$status" != "Accepted" ]; then
        cat "$APP_NOTARY_LOG" >&2
        die "app notarization status was ${status:-unknown}, expected Accepted"
    fi

    log "Stapling notarization ticket to app"
    xcrun stapler staple "$STAGED_APP_PATH"
    log "Validating stapled app notarization ticket"
    xcrun stapler validate "$STAGED_APP_PATH"

    log "Verifying stapled app code signature"
    codesign --verify --deep --strict --verbose=2 "$STAGED_APP_PATH"

    log "Assessing stapled app with spctl"
    spctl -a -vv -t exec "$STAGED_APP_PATH"

    rm -f "$APP_NOTARY_ZIP_PATH"
    log "App notarized and stapled"
}

assess_stapled_app() {
    log "Mounting DMG for Gatekeeper assessment"
    rm -rf "$MOUNT_DIR"
    mkdir -p "$MOUNT_DIR"

    hdiutil attach "$DMG_PATH" \
        -readonly \
        -nobrowse \
        -mountpoint "$MOUNT_DIR"
    DMG_ATTACHED=1

    log "Verifying mounted app code signature"
    codesign --verify --deep --strict --verbose=2 "$MOUNT_DIR/$APP_NAME.app"

    # Source: spctl assesses code against system security policy:
    # https://keith.github.io/xcode-man-pages/spctl.8.html
    log "Assessing mounted app with spctl"
    local assess_status=0
    spctl -a -vv -t exec "$MOUNT_DIR/$APP_NAME.app" || assess_status=$?

    detach_dmg
    if [ "$assess_status" -eq 0 ]; then
        log "Gatekeeper assessment completed"
    else
        log "Gatekeeper assessment failed"
    fi
    return "$assess_status"
}

smoke_test_app_launch() {
    [ -f "$DMG_PATH" ] || die "DMG missing: $DMG_PATH"

    local log_path="$BUILD_DIR/app-launch-smoke-test.log"
    local app_path="$MOUNT_DIR/$APP_NAME.app"
    local executable="$app_path/Contents/MacOS/$APP_NAME"
    local wait_secs="${SMOKE_TEST_WAIT_SECS:-5}"

    log "Mounting DMG for app launch smoke test"
    rm -rf "$MOUNT_DIR"
    mkdir -p "$MOUNT_DIR"

    hdiutil attach "$DMG_PATH" \
        -readonly \
        -nobrowse \
        -mountpoint "$MOUNT_DIR"
    DMG_ATTACHED=1

    [ -x "$executable" ] || die "app executable missing: $executable"

    rm -f "$log_path"
    log "Launching mounted app without arguments"
    "$executable" >"$log_path" 2>&1 &
    local app_pid=$!

    sleep "$wait_secs"

    if kill -0 "$app_pid" >/dev/null 2>&1; then
        log "App stayed running for $wait_secs seconds"
        kill "$app_pid" >/dev/null 2>&1 || true
        wait "$app_pid" >/dev/null 2>&1 || true
        detach_dmg
        return 0
    fi

    local launch_status=0
    wait "$app_pid" || launch_status=$?
    if [ -s "$log_path" ]; then
        cat "$log_path" >&2
    fi

    detach_dmg
    die "app exited during launch smoke test with status $launch_status"
}

create_cloudflare_root() {
    require_env EDITOR_SPARKLE_ED_PRIVATE_KEY
    [ -d "$STAGED_APP_PATH" ] || die "staged app missing: $STAGED_APP_PATH"
    [ -x "$ROOT_DIR/sparkle-bin/sign_update" ] || die "sparkle-bin/sign_update is required; run scripts/download-sparkle.sh"

    log "Creating Cloudflare root at $CLOUDFLARE_ROOT_DIR"
    rm -rf "$CLOUDFLARE_ROOT_DIR"
    mkdir -p "$CLOUDFLARE_ROOT_DIR"

    log "Creating Sparkle update ZIP at $SPARKLE_UPDATE_PATH"
    ditto -c -k --sequesterRsrc --keepParent "$STAGED_APP_PATH" "$SPARKLE_UPDATE_PATH"

    log "Signing Sparkle update ZIP"
    local sparkle_key_path="$RUNNER_TEMP_DIR/sparkle-ed-private-key"
    printf "%s" "$EDITOR_SPARKLE_ED_PRIVATE_KEY" > "$sparkle_key_path"
    chmod 600 "$sparkle_key_path"

    local signature_output
    local sign_status=0
    signature_output="$("$ROOT_DIR/sparkle-bin/sign_update" --ed-key-file "$sparkle_key_path" "$SPARKLE_UPDATE_PATH" 2>&1)" || sign_status=$?
    rm -f "$sparkle_key_path"
    if [ "$sign_status" -ne 0 ]; then
        die "sign_update failed with status $sign_status"
    fi

    local pub_date
    pub_date="$(date -uR)"

    log "Writing Sparkle appcast at $SPARKLE_APPCAST_PATH"
    cat > "$SPARKLE_APPCAST_PATH" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Slint Visual Editor Updates</title>
    <link>$SPARKLE_FEED_BASE_URL/appcast.xml</link>
    <description>Slint Visual Editor daily updates</description>
    <item>
      <title>Slint Visual Editor $VERSION ($BUILD_NUMBER)</title>
      <pubDate>$pub_date</pubDate>
      <sparkle:version>$BUILD_NUMBER</sparkle:version>
      <sparkle:shortVersionString>$VERSION</sparkle:shortVersionString>
      <enclosure
        url="$SPARKLE_FEED_BASE_URL/$SPARKLE_UPDATE_BASENAME"
        $signature_output
        type="application/octet-stream" />
    </item>
  </channel>
</rss>
EOF

    log "Cloudflare root contains:"
    ls -p1 "$CLOUDFLARE_ROOT_DIR"
}

full_package() {
    trap cleanup EXIT
    validate_environment
    install_signing_material
    archive_app
    stage_and_sign_app
    notarize_and_staple_app
    create_cloudflare_root
    create_dmg
    sign_dmg
    notarize_and_staple_dmg
    assess_stapled_app
    smoke_test_app_launch
    cleanup
    trap - EXIT
}

COMMAND="${1:-full}"

case "$COMMAND" in
    validate-environment)
        validate_environment
        ;;
    install-signing-material)
        validate_environment
        install_signing_material
        ;;
    archive-app)
        validate_environment
        archive_app
        ;;
    stage-and-sign-app)
        validate_environment
        stage_and_sign_app
        ;;
    notarize-and-staple-app)
        validate_environment
        notarize_and_staple_app
        ;;
    create-cloudflare-root)
        create_cloudflare_root
        ;;
    create-dmg)
        create_dmg
        ;;
    sign-dmg)
        validate_environment
        sign_dmg
        ;;
    create-and-sign-dmg)
        validate_environment
        create_dmg
        sign_dmg
        ;;
    notarize-and-staple-dmg)
        validate_environment
        notarize_and_staple_dmg
        ;;
    assess-stapled-app)
        validate_environment
        assess_stapled_app
        ;;
    smoke-test-app-launch)
        smoke_test_app_launch
        ;;
    cleanup)
        cleanup
        ;;
    full)
        full_package
        ;;
    *)
        die "unknown command: $1"
        ;;
esac

case "$COMMAND" in
    full | create-dmg | sign-dmg | create-and-sign-dmg | notarize-and-staple-dmg | assess-stapled-app | smoke-test-app-launch)
        log "DMG path: $DMG_PATH"
        ;;
    create-cloudflare-root)
        log "Cloudflare root path: $CLOUDFLARE_ROOT_DIR"
        ;;
    notarize-and-staple-app)
        log "App notarization log path: $APP_NOTARY_LOG"
        ;;
esac
