<!-- cspell:ignore actool appcast APPSTORE notarytool spctl -->

# macOS Visual Editor DMG

This will become part of the docs later, but for now, this is a placeholder.

## CI entry point

The dedicated workflow is `.github/workflows/visual_editor_macos_dmg.yaml`.
It runs on pull requests against the `visual-editor` branch, on manual dispatch,
and as a reusable workflow.

`.github/workflows/visual_editor_nightly.yaml` calls it once a night and
publishes the result. It's a separate workflow rather than a job in
`nightly_snapshot.yaml` because the editor publishes to its own host on its own
schedule.

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
- `EDITOR_SPARKLE_ED_PRIVATE_KEY`: the Sparkle EdDSA private key, base64 of the
  32-byte ed25519 seed, exactly as `generate_keys -x` writes it. Only the
  appcast step uses it, and pull request runs don't get it: same-repo pull
  requests do receive secrets, so keeping it out of their scope is the only
  thing stopping a branch from printing it.

The nightly workflow additionally needs write access to the R2 bucket:

- `VISUAL_EDITOR_R2_API_TOKEN` -> `CLOUDFLARE_API_TOKEN`: API token with R2 read
  and write on the `visual-editor-updates` bucket, and nothing else.
- `CLOUDFLARE_ACCOUNT_ID`: the account that owns the bucket. Shared with the
  other Cloudflare deploys in this repository.

Two values are not secrets and are not provisioned via GitHub Actions:

- The Apple Developer Team ID is derived by the packaging script from the
  imported Developer ID certificate. Set `MACOS_DEVELOPMENT_TEAM` to override.
- The bundle identifier defaults to `dev.slint.visual-editor` in the packaging
  script. Set `MACOS_BUNDLE_IDENTIFIER` to override.

The Sparkle public key is deliberately not provisioned. It's checked into
`scripts/package_macos_visual_editor.bash` and reaches `SUPublicEDKey` from
there, because every shipped app verifies updates against the copy it was built
with: change it and every installed copy stops updating, silently. A value with
that property belongs somewhere a diff shows it, not in a repository variable.

To check that the checked-in key really is the public half of the secret:

```sh
uv run --with cryptography python -c '
import base64, sys
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization
seed = base64.b64decode(sys.stdin.read().strip())
pub = Ed25519PrivateKey.from_private_bytes(seed).public_key().public_bytes(
    serialization.Encoding.Raw, serialization.PublicFormat.Raw)
print(base64.b64encode(pub).decode())
' < private-key-file
```

Sparkle has no key rotation protocol. Losing the private key means no installed
copy can ever be updated again, so it belongs in the team password manager as
well as in the secret. Replacing it means shipping an update signed with the old
key whose app carries the new public key, waiting for that to be picked up, and
only then switching what signs.

## Sparkle framework

The editor checks for its own updates with Sparkle on macOS.
There is no Cargo feature for it: the code is inactive unless the editor runs
from an application bundle, so a plain `cargo run` keeps its update chrome
hidden because the update state stays at `UpToDate`.

The build itself does not need `Sparkle.framework`: the classes are reached
through the Objective-C runtime, so the editor loads the framework from its own
application bundle at startup.
Packaging does need it, and so do the key tools below:

```sh
./scripts/download-sparkle.sh
```

The script pins the version and its checksum, and it also installs the
`sparkle-bin/` tools that the keys below need.
Both directories are ignored by git.
`scripts/package_macos_visual_editor.bash` copies it out of the repository root
unless `SPARKLE_FRAMEWORK_DIR` says otherwise.

Then run the editor with updates enabled:

```sh
cargo run -p slint-lsp --example slint-editor \
    --no-default-features \
    --features backend-winit,renderer-skia
```

Set `SLINT_SPARKLE_INTERACTIVE=1` to check for updates through Sparkle's own
dialog instead of the editor's chrome.
That is the only way to run a real download and install for as long as the
editor's own update buttons are stubs.

A build run outside an application bundle simply reports that updates are off.
When the framework is missing from a bundle that should have it, the editor
still starts and logs which path it looked at.

The DMG workflow gets the framework from the `.github/actions/install-sparkle`
action. No other CI job needs it, because nothing links against Sparkle.

## Sparkle Keys

With the framework and tools installed, create or inspect the Visual Editor key
pair:

```sh
./sparkle-bin/generate_keys --account slint-visual-editor
./sparkle-bin/generate_keys --account slint-visual-editor -p
./sparkle-bin/generate_keys --account slint-visual-editor -x /tmp/slint-visual-editor-sparkle-private-key
```

The `-p` command prints the public key. The `-x` command writes the private key
file silently; use that file's contents for `EDITOR_SPARKLE_ED_PRIVATE_KEY`.
When rotating keys, update both the public-key variable and private-key secret.

## Generated Xcode project

The checked-in source of truth is `tools/editor/macos-project.yml`. XcodeGen
generates `tools/editor/Slint Visual Editor.xcodeproj/` and
`tools/editor/Info.plist` from that spec, and both generated paths are ignored.

XcodeGen documents YAML project specs and environment variable substitution
with `${VARIABLE}` here:
<https://yonaskolb.github.io/XcodeGen/Docs/ProjectSpec.html>.

XcodeGen installation is documented by the project and the Homebrew formula:
<https://github.com/yonaskolb/XcodeGen> and
<https://formulae.brew.sh/formula/xcodegen>.

The app icon source is `tools/editor/packaging/macos/AppIcon.icon`. It is an
Icon Composer project checked in as source and included in the generated Xcode
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
5. Runs `xcodegen generate --spec tools/editor/macos-project.yml`.
6. Runs `xcodebuild archive` with `ARCHS="arm64"` and `CODE_SIGNING_ALLOWED=NO`.
7. Lets Xcode call `scripts/build_macos_app_with_cargo.bash` from a build phase.
8. Builds Cargo's `slint-editor` binary for `aarch64-apple-darwin` with
   `cargo build --timings`.
9. Signs the app bundle with `codesign --deep --options runtime`.
10. Submits a temporary app ZIP with `xcrun notarytool submit --wait`.
11. Staples and validates the notarization ticket on the staged app bundle.
12. Copies Cargo's timing report from
    `target/xcode-cargo/slint-visual-editor/cargo-timings/` to
    `target/macos-visual-editor-dmg/cargo-timings/`.
13. Deletes Xcode and Cargo build intermediates after the signed app is staged.
    This is done to free up space on the runner image.
14. Creates the DMG with `L-Super/create-dmg-actions`, passing
    `tools/editor/packaging/macos/dmg-background.svg`, the Finder window size,
    the app icon position, and the Applications drop-link position as action
    inputs.
15. Moves the action output to `dist/`, signs the DMG with `codesign`, then
    verifies the DMG and mounted app payload.
16. Submits the DMG with `xcrun notarytool submit --wait`.
17. Staples and validates the accepted ticket with `xcrun stapler`, then
    repeats the DMG and mounted app signature checks on the final artifact.
18. Writes `dist/appcast.xml`, carrying a Sparkle EdDSA signature over the
    finished DMG. Sparkle installs from the DMG, so this can only run once the
    DMG is stapled and won't change again.
19. Mounts the DMG, verifies the mounted app with `codesign`, and checks it
    with `spctl`.
20. Uploads `dist/*.dmg` and `dist/appcast.xml` as the
    `slint-visual-editor-macos` artifact, the notarization logs as the
    `slint-visual-editor-notarization-logs` artifact, and the Cargo timing
    report as the `slint-visual-editor-rust-build-report` artifact.

For local debugging, the same phases can be run individually:

```sh
./scripts/package_macos_visual_editor.bash validate-environment
./scripts/package_macos_visual_editor.bash install-signing-material
./scripts/package_macos_visual_editor.bash archive-app
./scripts/package_macos_visual_editor.bash stage-and-sign-app
./scripts/package_macos_visual_editor.bash notarize-and-staple-app
./scripts/package_macos_visual_editor.bash create-dmg
./scripts/package_macos_visual_editor.bash sign-dmg
./scripts/package_macos_visual_editor.bash notarize-and-staple-dmg
./scripts/package_macos_visual_editor.bash create-appcast
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

The expected artifacts are:

```text
dist/SlintVisualEditor-<version>-<build>-macos-arm64.dmg
dist/appcast.xml
```

## Versions and channels

`SLINT_EDITOR_CHANNEL` picks the prefix under `visual-editor.slint.dev`, and
with it the feed the build points at. It defaults to `nightly`; `stable` is
wired through the packaging script but nothing publishes it yet.

| | `nightly` | `stable` |
|---|---|---|
| `CFBundleVersion`, `sparkle:version` | `2026.0825.0300` | same |
| `CFBundleShortVersionString`, `sparkle:shortVersionString` | `1.18.0+2026.0825.0300` | `1.18.0` |
| DMG name | `SlintVisualEditor-1.18.0-2026.0825.0300-macos-arm64.dmg` | `SlintVisualEditor-1.18.0-macos-arm64.dmg` |
| `SUFeedURL` | `.../nightly/appcast.xml` | `.../stable/appcast.xml` |

Sparkle compares `sparkle:version` against the installed `CFBundleVersion` and
ignores the short version string, which is there for people to read.
The build number is a UTC timestamp rather than `github.run_number` because a
run number restarts at 1 when a workflow is renamed, and a build number that
goes backwards freezes updates for everyone with no visible error.

`SUFeedURL` is baked into every shipped `Info.plist`, so the channel a build was
made with is permanent for that copy. `EDITOR_SPARKLE_FEED_URL` overrides the
whole URL, and `SPARKLE_FEED_BASE_URL` overrides just the base.

## Publishing

`visual_editor_nightly.yaml` writes three objects into the
`visual-editor-updates` bucket, which serves `visual-editor.slint.dev`:

```text
nightly/builds/SlintVisualEditor-<version>-<build>-macos-arm64.dmg
nightly/SlintVisualEditor.dmg
nightly/appcast.xml
```

`nightly/SlintVisualEditor.dmg` is the same bytes under a fixed key, so the
website has a download link that never needs updating. It carries a
`Content-Disposition` naming the stamped file, so what lands on disk still says
which nightly it is.

R2 writes are per object, so nothing else in the bucket is touched. That's the
reason for R2 rather than Pages or a Worker with `[assets]`: those deploy a
whole directory tree, and a deploy carrying only `nightly/` would take
`stable/` down with it.

Three rules come with that layout:

- The DMG is uploaded before the appcast, so the feed never names an object
  that isn't there yet.
- The DMG name carries the build stamp and no later build reuses it, so it's
  served `immutable` with a one-year max-age. The appcast is a stable URL whose
  content changes nightly, so it's served `no-cache`. Without that, Cloudflare
  would keep handing out yesterday's feed.
- The DMGs sit under `builds/` because R2 lifecycle rules filter by prefix
  only, with no suffix or glob. Expiring old builds means expiring a prefix, so
  the appcast and the fixed-key DMG have to live outside the one being expired.
- The fixed-key DMG is served `no-cache` for the same reason the appcast is, and
  the appcast enclosure points at the stamped copy rather than at it. Sparkle
  checks its signature against exact bytes, so a stale cached copy behind a
  mutable URL would read as tampering rather than as a stale download.

Old nightly DMGs are expired by this lifecycle rule:

```sh
npx wrangler r2 bucket lifecycle add visual-editor-updates \
    expire-nightly-builds nightly/builds/ --expire-days 14
```

Keep the window comfortably longer than the longest expected gap between
nightly runs. The appcast always names the newest build, so if the nightly
stops running for longer than the retention window, that build expires and the
feed points at a missing object.

## Local Sparkle update test

To test the update path without production keys or a published feed, let the helper
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
xcodegen generate --spec tools/editor/macos-project.yml
```

For app archive debugging only:

```sh
xcodebuild archive \
    -project "tools/editor/Slint Visual Editor.xcodeproj" \
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
