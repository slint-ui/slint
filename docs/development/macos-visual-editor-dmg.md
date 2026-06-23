# macOS Visual Editor DMG

This document describes the CI path that builds the Slint Visual Editor as a
signed, notarized macOS DMG artifact. The workflow is for Developer ID
distribution outside the App Store; it does not upload to App Store Connect,
TestFlight, or the Mac App Store.

## CI entry point

The dedicated workflow is `.github/workflows/visual_editor_macos_dmg.yaml`.
It runs only for:

- `workflow_dispatch`
- pushes to `deploy-macos`
- pull requests targeting `visual-editor`

This keeps the packaging work isolated from the broad repository CI while the
DMG pipeline is being developed. The trigger syntax and `pull_request` branch
filters are from GitHub's workflow syntax documentation:
<https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>.

The workflow uses `macos-15` because GitHub documents it as an arm64 macOS
hosted runner. The packaging script still builds both Apple Silicon and Intel
Rust targets for a universal app:
<https://docs.github.com/en/actions/reference/runners/github-hosted-runners>.

## Required CI secrets

Set these as GitHub Actions secrets. GitHub documents repository and
organization secrets here:
<https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets>.

- `MACOS_CERTIFICATE_BASE64`: base64-encoded Developer ID Application `.p12`
  certificate. GitHub documents storing Apple signing certificates as base64
  secrets here:
  <https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications>.
- `MACOS_CERTIFICATE_PASSWORD`: password for the `.p12` certificate.
- `MACOS_KEYCHAIN_PASSWORD`: temporary CI keychain password.
- `MACOS_DEVELOPER_ID`: Developer ID Application signing identity name or hash
  used by `codesign`.
- `MACOS_DEVELOPMENT_TEAM`: Apple Developer Team ID. This is deliberately a
  secret, even though Xcode accepts it as the `DEVELOPMENT_TEAM` build setting.
- `MACOS_BUNDLE_IDENTIFIER`: app bundle identifier used by the generated Xcode
  project.
- `NOTARY_API_KEY_BASE64`: base64-encoded App Store Connect API key `.p8`.
  This is used only for `notarytool` authentication, not for Store/TestFlight
  upload.
- `NOTARY_API_KEY_ID`: App Store Connect API key ID for `notarytool`.
- `NOTARY_ISSUER_ID`: issuer UUID for a Team API key.

Apple's `notarytool` documentation lists API key authentication for notary
submissions and states that notarization is a malware/signing check for
Developer ID-distributed software:
<https://keith.github.io/xcode-man-pages/notarytool.1.html>.

## Generated Xcode project

The checked-in source of truth is `tools/lsp/macos-project.yml`. XcodeGen
generates `tools/lsp/Slint Visual Editor.xcodeproj/` and `tools/lsp/Info.plist`
from that spec, and both generated paths are ignored.

XcodeGen documents YAML project specs and environment variable substitution
with `${VARIABLE}` here:
<https://yonaskolb.github.io/XcodeGen/Docs/ProjectSpec.html>.

XcodeGen installation is documented by the project and the Homebrew formula:
<https://github.com/yonaskolb/XcodeGen> and
<https://formulae.brew.sh/formula/xcodegen>.

## Build flow

The workflow installs Rust once through the repository's existing
`.github/actions/setup-rust` action, then adds the two macOS Rust targets with
`rustup target add aarch64-apple-darwin x86_64-apple-darwin` before installing
XcodeGen with Homebrew.

The package driver is `scripts/package_macos_visual_editor.bash`:

1. Validates that all signing, Team ID, bundle ID, and notary values are present
   in environment variables.
2. Decodes the Developer ID `.p12` and notary API `.p8` into `$RUNNER_TEMP`.
3. Creates and unlocks a temporary keychain with `security`.
4. Runs `xcodegen generate --spec tools/lsp/macos-project.yml`.
5. Runs `xcodebuild archive` with `ARCHS="arm64 x86_64"` and
   `CODE_SIGNING_ALLOWED=NO`.
6. Lets Xcode call `scripts/build_macos_app_with_cargo.bash` from a build phase.
7. Builds Cargo's `slint-editor` example for `aarch64-apple-darwin` and
   `x86_64-apple-darwin`.
8. Combines both executables with `lipo`.
9. Signs the executable and app bundle with `codesign --options runtime`.
10. Creates and signs a compressed DMG with `hdiutil`.
11. Submits the DMG with `xcrun notarytool submit --wait`.
12. Staples and validates the accepted ticket with `xcrun stapler`.
13. Mounts the DMG and checks the app with `spctl`.
14. Uploads `dist/*.dmg` as a GitHub Actions artifact.

The command sources for these steps are:

- `security`: <https://keith.github.io/xcode-man-pages/security.1.html>
- `xcodebuild`: <https://keith.github.io/xcode-man-pages/xcodebuild.1.html>
- `lipo`: <https://keith.github.io/xcode-man-pages/lipo.1.html>
- `codesign`: <https://keith.github.io/xcode-man-pages/codesign.1.html>
- `hdiutil`: <https://keith.github.io/xcode-man-pages/hdiutil.1.html>
- `notarytool`: <https://keith.github.io/xcode-man-pages/notarytool.1.html>
- `stapler`: <https://keith.github.io/xcode-man-pages/stapler.1.html>
- `spctl`: <https://keith.github.io/xcode-man-pages/spctl.8.html>
- GitHub artifacts:
  <https://docs.github.com/en/actions/tutorials/store-and-share-data>
- `actions/upload-artifact`:
  <https://github.com/actions/upload-artifact>

## Local reproduction

Set the same environment variables as the CI secrets, then run:

```sh
brew install xcodegen
rustup target add aarch64-apple-darwin x86_64-apple-darwin
./scripts/package_macos_visual_editor.bash
```

The expected artifact name is:

```text
dist/SlintVisualEditor-<version>-macos-universal.dmg
```

For Xcode project generation only:

```sh
xcodegen generate --spec tools/lsp/macos-project.yml
```

For app archive debugging only:

```sh
xcodebuild archive \
    -project "tools/lsp/Slint Visual Editor.xcodeproj" \
    -scheme "Slint Visual Editor" \
    -configuration Release \
    -destination "generic/platform=macOS" \
    -archivePath "target/macos-visual-editor-dmg/Slint Visual Editor.xcarchive" \
    ARCHS="arm64 x86_64" \
    ONLY_ACTIVE_ARCH=NO \
    SKIP_INSTALL=NO \
    CODE_SIGNING_ALLOWED=NO
```

## Verification commands

The packaging script runs these checks automatically:

```sh
codesign --verify --deep --strict --verbose=2 "Slint Visual Editor.app"
hdiutil verify "SlintVisualEditor-<version>-macos-universal.dmg"
codesign --verify --strict --verbose=2 "SlintVisualEditor-<version>-macos-universal.dmg"
xcrun stapler validate "SlintVisualEditor-<version>-macos-universal.dmg"
spctl -a -vv -t exec "/Volumes/Slint Visual Editor/Slint Visual Editor.app"
```

Apple's notarization overview is here:
<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>.
