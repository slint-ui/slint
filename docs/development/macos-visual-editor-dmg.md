# macOS Visual Editor DMG

This will become part of the docs later, but for now, this is a placeholder.

## CI entry point

The dedicated workflow is `.github/workflows/visual_editor_macos_dmg.yaml`.

The workflow uses `macos-26-arm64` because GitHub documents it as an arm64
macOS hosted runner:
<https://github.com/actions/runner-images/blob/main/README.md> and
<https://github.com/actions/runner-images/blob/main/images/macos/macos-26-arm64-Readme.md>.

The macOS 26 arm64 image defaults to Xcode 26.5, so the workflow relies on the
image default instead of setting `DEVELOPER_DIR`.

## CI secrets and variables

The workflow reuses the repository's shared Apple signing and notarization
secrets and maps them onto the environment variables the packaging script
expects. GitHub documents repository and organization secrets here:
<https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets>.

- `MACOS_CERTIFICATE_BASE64` <- `APPLE_CERTIFICATE_P12`: base64-encoded
  Developer ID Application `.p12` certificate, the same one used by
  `.github/actions/codesign`. GitHub documents storing Apple signing
  certificates as base64 secrets here:
  <https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications>.
- `MACOS_CERTIFICATE_PASSWORD` <- `APPLE_CERTIFICATE_P12_PASSWORD`: password
  for the `.p12` certificate.
- `MACOS_KEYCHAIN_PASSWORD` <- `APPLE_KEYCHAIN_PASSWORD`: temporary CI
  keychain password.
- `MACOS_DEVELOPER_ID` <- `APPLE_DEV_ID`: Developer ID Application signing
  identity name or hash used by `codesign`.
- `NOTARY_API_KEY_BASE64` <- `APPLE_APPSTORE_PRIVATE_KEY_BASE64`:
  base64-encoded App Store Connect API key `.p8`. This is used only for
  `notarytool` authentication, not for Store/TestFlight upload.
- `NOTARY_API_KEY_ID` <- `APPLE_APPSTORE_CONNECT_KEY`: App Store Connect API
  key ID for `notarytool`.
- `NOTARY_ISSUER_ID` <- `APPLE_APPSTORE_ISSUER_ID`: issuer UUID for a Team API
  key.
- `EDITOR_SPARKLE_ED_PRIVATE_KEY`: exported Sparkle EdDSA private key for the
  Visual Editor update feed. Use it only for signing update archives.

Two values are not secrets and are not provisioned via GitHub Actions:

- The Apple Developer Team ID is derived by the packaging script from the
  imported Developer ID certificate. Set `MACOS_DEVELOPMENT_TEAM` to override.
- The bundle identifier defaults to `dev.slint.visual-editor` in the packaging
  script. Set `MACOS_BUNDLE_IDENTIFIER` to override.

Optional GitHub Actions variable:

- `EDITOR_SPARKLE_PUBLIC_ED_KEY`: public Sparkle EdDSA key for the app's
  `SUPublicEDKey`. The packaging script uses the checked-in default when this
  variable is not set.

## Sparkle Keys

Install Sparkle's framework and tools, then create or inspect the Visual Editor
key pair:

```sh
./scripts/download-sparkle.sh
./sparkle-bin/generate_keys --account slint-visual-editor
./sparkle-bin/generate_keys --account slint-visual-editor -p
./sparkle-bin/generate_keys --account slint-visual-editor -x /tmp/slint-visual-editor-sparkle-private-key
```

The `-p` command prints the public key. The `-x` command writes the private key
file silently; use that file's contents for `EDITOR_SPARKLE_ED_PRIVATE_KEY`.
When rotating keys, update both the public-key variable and private-key secret.

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
`create-dmg` with Homebrew. Homebrew documents these formulae here:
<https://formulae.brew.sh/formula/xcodegen> and
<https://formulae.brew.sh/formula/create-dmg>.

The CI workflow uses `L-Super/create-dmg-actions@v1.1.0` to create the DMG
layout from the signed app bundle. The action documents `dmg_name`, `src_dir`,
`background`, `window_size`, `icon_size`, `icon_position`, `app_drop_link`, and
the `dmg_path` output here:
<https://github.com/marketplace/actions/create-macos-dmg> and
<https://github.com/L-Super/create-dmg-actions>.

The package driver is `scripts/package_macos_visual_editor.bash`.

1. Validates that all signing and notary values are present in environment
   variables. The Team ID is derived from the imported certificate and the
   bundle ID has a checked-in default.
2. Frees unused macOS runner image space before cache restore/build work.
3. Decodes the Developer ID `.p12` and notary API `.p8` into `$RUNNER_TEMP`.
4. Creates and unlocks a temporary keychain with `security`.
5. Runs `xcodegen generate --spec tools/lsp/macos-project.yml`.
6. Runs `xcodebuild archive` with `ARCHS="arm64"` and `CODE_SIGNING_ALLOWED=NO`.
7. Lets Xcode call `scripts/build_macos_app_with_cargo.bash` from a build phase.
8. Builds Cargo's `slint-visual-editor` binary for `aarch64-apple-darwin` with
   `cargo build --timings`.
9. Copies the visual editor files into the app bundle resources so Finder
    launches can open a default project without command-line arguments.
10. Signs the app bundle with `codesign --deep --options runtime`.
11. Submits a temporary app ZIP with `xcrun notarytool submit --wait`.
12. Staples and validates the notarization ticket on the staged app bundle.
13. Copies Cargo's timing report from
    `target/xcode-cargo/slint-visual-editor/cargo-timings/` to
    `target/macos-visual-editor-dmg/cargo-timings/`.
14. Deletes Xcode and Cargo build intermediates after the signed app is staged.
    This is done to free up space on the runner image.
15. Creates `dist/cloudflare-root/` with `appcast.xml` and a Sparkle-signed
    update ZIP containing the notarized and stapled app.
16. Computes the versioned DMG name from the `slint-lsp` Cargo package
    version.
17. Creates the DMG with `L-Super/create-dmg-actions`, passing
    `tools/lsp/packaging/macos/dmg-background.svg`, the Finder window size, the
    app icon position, and the Applications drop-link position as action inputs.
18. Moves the action output to `dist/`, signs the DMG with `codesign`, then
    verifies the DMG and mounted app payload.
19. Submits the DMG with `xcrun notarytool submit --wait`.
20. Staples and validates the accepted ticket with `xcrun stapler`, then
    repeats the DMG and mounted app signature checks on the final artifact.
21. Mounts the DMG, verifies the mounted app with `codesign`, and checks it
    with `spctl`.
22. Uploads `dist/*.dmg` and notarization logs as the
    `slint-visual-editor-macos-dmg` artifact, `dist/cloudflare-root/*` as the
    `slint-visual-editor-cloudflare-root` artifact, and the Cargo timing report
    as the `slint-visual-editor-rust-build-report` artifact.

The app's marketing version, Sparkle `sparkle:shortVersionString`, and artifact
names use the `slint-lsp` version from `tools/lsp/Cargo.toml`.
The app build number and Sparkle `sparkle:version` use `SLINT_BUILD_NUMBER`,
which comes from `github.run_number`.

For local debugging, the same phases can be run individually:

```sh
./scripts/package_macos_visual_editor.bash validate-environment
./scripts/package_macos_visual_editor.bash install-signing-material
./scripts/package_macos_visual_editor.bash archive-app
./scripts/package_macos_visual_editor.bash stage-and-sign-app
./scripts/package_macos_visual_editor.bash notarize-and-staple-app
./scripts/package_macos_visual_editor.bash create-cloudflare-root
./scripts/package_macos_visual_editor.bash create-dmg
./scripts/package_macos_visual_editor.bash sign-dmg
./scripts/package_macos_visual_editor.bash notarize-and-staple-dmg
./scripts/package_macos_visual_editor.bash assess-stapled-app
./scripts/package_macos_visual_editor.bash cleanup
```

The command sources for these steps are:

- `security`: <https://keith.github.io/xcode-man-pages/security.1.html>
- `xcodebuild`: <https://keith.github.io/xcode-man-pages/xcodebuild.1.html>
- `cargo build --timings`: <https://doc.rust-lang.org/cargo/commands/cargo-build.html#compilation-options>
- `create-dmg`: <https://github.com/create-dmg/create-dmg>
- `L-Super/create-dmg-actions`: <https://github.com/L-Super/create-dmg-actions>
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
brew install xcodegen create-dmg
rustup target add aarch64-apple-darwin
./scripts/package_macos_visual_editor.bash
```

The expected artifact name is:

```text
dist/SlintVisualEditor-<version>-macos-arm64.dmg
```

The Cloudflare deploy artifact is `slint-visual-editor-cloudflare-root`.
Extract its contents to the root of `https://visual-editor.slint.dev/`.
It contains:

```text
appcast.xml
SlintVisualEditor-<version>-<build>-macos-arm64.zip
```

## Local Sparkle update test

To test the update path without production keys or Cloudflare, let the helper
build a local Visual Editor app:

```sh
./scripts/local_sparkle_update_test.sh --build-editor
```

This uses Cargo for the app binary and Xcode's `actool` for the app icon.

To test existing artifacts instead, pass two local `.app` bundles:

```sh
./scripts/local_sparkle_update_test.sh \
    --old-app "/path/to/old/Slint Visual Editor.app" \
    --new-app "/path/to/new/Slint Visual Editor.app"
```

The script generates or reuses a local Sparkle keychain account, patches only
the temp copies, serves a local `appcast.xml`, launches the old app with
`open -n`, and checks whether Sparkle replaced it with the newer build.

The Rust build report artifact is `slint-visual-editor-rust-build-report`.
Cargo documents that `--timings` writes `cargo-timing.html` and timestamped reports to
the target directory's `cargo-timings` directory:
<https://doc.rust-lang.org/cargo/commands/cargo-build.html#compilation-options>.

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
