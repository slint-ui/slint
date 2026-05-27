# Building slint-viewer for iOS

Producing an iOS build of `slint-viewer` for TestFlight happens in two stages: CI builds an
**unsigned** archive, and you **sign, export and upload** it locally. Keeping the signing local
means no Apple distribution credentials need to live in the CI secrets.

## Building the archive in CI

The [`slint_tool_binary.yaml`](../.github/workflows/slint_tool_binary.yaml) workflow has an opt-in
`ios` input (default `false`). Enable it to build the `build_ios` job, which produces an unsigned
`.xcarchive` and uploads it as a workflow artifact named `slint-viewer-ios-<version>-<short-sha>`.

Trigger it from the Actions tab ("Build slint-viewer or -lsp binary" → "Run workflow") with **ios**
checked, or:

```bash
gh workflow run slint_tool_binary.yaml -f program=viewer -f ios=true
```

The archive carries the workspace version as its marketing version and the commit count as its build
number. App Store Connect requires the build number to grow with every upload, so always archive
from a newer commit than the last upload.

The app icon is rendered from `logo/slint-logo-square-light-whitebg.svg` into the asset catalog at
build time by `scripts/render_ios_app_icon.bash` using `rsvg-convert`, so it is not checked in.

## Signing, exporting and uploading locally

### Prerequisites

 * An Apple Developer account that is a member of the team owning the `dev.slint.slint-viewer`
   App ID, with a matching **App Store Connect** app record.
 * An **Apple Distribution** signing certificate and an **App Store** provisioning profile for
   `dev.slint.slint-viewer` installed in your keychain.
 * An **App Store Connect API key** (`.p8` plus its Key ID and Issuer ID) for uploading.

### Sign and upload

Download and unzip the CI artifact, then create an `ExportOptions.plist`. The `upload` destination
makes `xcodebuild` upload to App Store Connect directly, so no separate upload tool is needed:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>app-store-connect</string>
    <key>destination</key>
    <string>upload</string>
    <key>teamID</key>
    <string>YOUR_TEAM_ID</string>
</dict>
</plist>
```

(Older Xcode versions use `app-store` instead of `app-store-connect` for the `method`.)

A single `xcodebuild -exportArchive` then signs the archive with your distribution certificate and
provisioning profile and uploads it using an App Store Connect API key (`.p8`):

```bash
xcodebuild -exportArchive \
    -archivePath slint-viewer.xcarchive \
    -exportPath export \
    -exportOptionsPlist ExportOptions.plist \
    -authenticationKeyPath ~/private_keys/AuthKey_<KEY_ID>.p8 \
    -authenticationKeyID <KEY_ID> \
    -authenticationKeyIssuerID <ISSUER_ID>
```

After processing, the build appears under TestFlight in App Store Connect.

To upload through a GUI instead, set `destination` to `export` so the signed
`export/slint-viewer.ipa` is written to disk, then drag it into the
[Transporter](https://apps.apple.com/app/transporter/id1450874784) app.
