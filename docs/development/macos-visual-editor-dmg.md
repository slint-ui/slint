# macOS Visual Editor DMG

This will become part of the docs later, but for now, this is a placeholder.

## CI entry point

The dedicated workflow is `.github/workflows/visual_editor_macos_dmg.yaml`.

The workflow uses `macos-15` because GitHub documents it as an arm64 macOS
hosted runner

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

The app icon source is `tools/lsp/packaging/macos/AppIcon.icon`. It is an Icon
Composer project checked in as source and included in the generated Xcode
project as a resource through the XcodeGen `sources` list.

## Build flow

The workflow installs Rust once through the repository's existing
`.github/actions/setup-rust` action, then adds the macOS Rust target with
`rustup target add aarch64-apple-darwin` before installing XcodeGen and
`librsvg` with Homebrew. Homebrew documents the `librsvg` formula here:
<https://formulae.brew.sh/formula/librsvg>.

The package driver is `scripts/package_macos_visual_editor.bash`.

1. Validates that all signing, Team ID, bundle ID, and notary values are present
   in environment variables.
2. Frees unused macOS runner image space before cache restore/build work.
3. Decodes the Developer ID `.p12` and notary API `.p8` into `$RUNNER_TEMP`.
4. Creates and unlocks a temporary keychain with `security`.
5. Runs `xcodegen generate --spec tools/lsp/macos-project.yml`.
6. Runs `xcodebuild archive` with `ARCHS="arm64"` and `CODE_SIGNING_ALLOWED=NO`.
7. Lets Xcode call `scripts/build_macos_app_with_cargo.bash` from a build phase.
8. Builds Cargo's `slint-visual-editor` binary for `aarch64-apple-darwin`.
9. Copies the visual editor files into the app bundle resources so Finder
    launches can open a default project without command-line arguments.
10. Signs the app bundle with `codesign --deep --options runtime`.
11. Deletes Xcode and Cargo build intermediates after the signed app is staged.
    This is done to free up space on the runner image.
12. Renders `tools/lsp/packaging/macos/dmg-background.svg` to
    `target/macos-visual-editor-dmg/dmg-background.png`.
13. Creates a read-write DMG with `hdiutil`, configures the Finder background,
    icon size, app position, and Applications symlink position, then converts it
    to the compressed DMG that is signed and verified.
14. Submits the DMG with `xcrun notarytool submit --wait`.
15. Staples and validates the accepted ticket with `xcrun stapler`, then
    repeats the DMG and mounted app signature checks on the final artifact.
16. Mounts the DMG, verifies the mounted app with `codesign`, and checks it
    with `spctl`.
17. Uploads `dist/*.dmg` as a GitHub Actions artifact.

For local debugging, the same phases can be run individually:

```sh
./scripts/package_macos_visual_editor.bash validate-environment
./scripts/package_macos_visual_editor.bash install-signing-material
./scripts/package_macos_visual_editor.bash archive-app
./scripts/package_macos_visual_editor.bash stage-and-sign-app
./scripts/package_macos_visual_editor.bash create-and-sign-dmg
./scripts/package_macos_visual_editor.bash notarize-and-staple-dmg
./scripts/package_macos_visual_editor.bash assess-stapled-app
./scripts/package_macos_visual_editor.bash cleanup
```

The command sources for these steps are:

- `security`: <https://keith.github.io/xcode-man-pages/security.1.html>
- `xcodebuild`: <https://keith.github.io/xcode-man-pages/xcodebuild.1.html>
- `librsvg` / `rsvg-convert`: <https://formulae.brew.sh/formula/librsvg>
- `codesign`: <https://keith.github.io/xcode-man-pages/codesign.1.html>
- `hdiutil`: <https://keith.github.io/xcode-man-pages/hdiutil.1.html>
- `sips`: <https://keith.github.io/xcode-man-pages/sips.1.html>
- `osascript`: <https://keith.github.io/xcode-man-pages/osascript.1.html>
- `SetFile`: <https://keith.github.io/xcode-man-pages/SetFile.1.html>
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
brew install xcodegen librsvg
rustup target add aarch64-apple-darwin
./scripts/package_macos_visual_editor.bash
```

The expected artifact name is:

```text
dist/SlintVisualEditor-<version>-macos-arm64.dmg
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
    ARCHS="arm64" \
    ONLY_ACTIVE_ARCH=NO \
    SKIP_INSTALL=NO \
    CODE_SIGNING_ALLOWED=NO
```

## Verification commands

The packaging script runs these checks automatically:

```sh
codesign --verify --deep --strict --verbose=2 "Slint Visual Editor.app"
hdiutil verify "SlintVisualEditor-<version>-macos-arm64.dmg"
codesign --verify --strict --verbose=2 "SlintVisualEditor-<version>-macos-arm64.dmg"
xcrun stapler validate "SlintVisualEditor-<version>-macos-arm64.dmg"
spctl -a -vv -t exec "/Volumes/Slint Visual Editor/Slint Visual Editor.app"
```

Apple's notarization overview is here:
<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>.
