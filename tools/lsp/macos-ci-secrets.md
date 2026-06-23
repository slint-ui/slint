# macOS Visual Editor CI Secrets

This document explains how to create the GitHub Actions secrets used by the
macOS visual editor DMG workflow. The Xcode project source is
`tools/lsp/macos-project.yml`; the GitHub Actions workflow that consumes these
secrets is `.github/workflows/visual_editor_macos_dmg.yaml`.

These secrets are for Developer ID distribution outside the Mac App Store. Do
not rename them to imply App Store, TestFlight, or Store upload behavior.
Apple documents Developer ID certificates for distributing Mac software outside
the Mac App Store:
<https://developer.apple.com/help/account/certificates/create-developer-id-certificates/>.
Apple documents notarization for macOS software distribution here:
<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>.

## Add the secrets in GitHub

Add each value as a repository secret:

1. Open the GitHub repository.
2. Go to Settings -> Secrets and variables -> Actions.
3. Choose New repository secret.
4. Add each secret name and value from the sections below.

GitHub documents repository secrets and the Actions UI here:
<https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets>.

GitHub also documents the pattern used here for Apple signing certificates:
store the certificate as a base64 secret, decode it during the workflow, import
it into a temporary keychain, and use that keychain for signing:
<https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications>.

## MACOS_CERTIFICATE_BASE64

Value: the base64-encoded contents of a `.p12` export containing the Developer
ID Application certificate and its private key.

Use a Developer ID Application certificate, not Developer ID Installer, Apple
Distribution, Mac Installer Distribution, or any App Store/TestFlight
certificate. Apple documents Developer ID Application certificates here:
<https://developer.apple.com/help/account/certificates/create-developer-id-certificates/>.

If the certificate is already installed on the Mac that owns the private key,
export it from Keychain Access:

1. Open Keychain Access.
2. Select the login keychain.
3. Select My Certificates.
4. Find the Developer ID Application certificate.
5. Confirm it expands to show a private key.
6. Export the certificate as a `.p12` file.

Apple documents exporting keychain items from Keychain Access here:
<https://support.apple.com/guide/keychain-access/export-and-import-keychain-items-kyca35961/mac>.

Encode the exported `.p12`:

```sh
base64 -i DeveloperIDApplication.p12 -o DeveloperIDApplication.p12.base64
```

Use the full contents of `DeveloperIDApplication.p12.base64` as the GitHub
secret value.

## MACOS_CERTIFICATE_PASSWORD

Value: the password chosen when exporting the Developer ID Application
certificate as `.p12`.

This is paired with `MACOS_CERTIFICATE_BASE64`: the workflow decodes the `.p12`
and imports it into a temporary keychain using this password. GitHub documents
this certificate-password pattern in its Xcode signing guide:
<https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications>.

## MACOS_KEYCHAIN_PASSWORD

Value: a new, long, random password used only by CI for the temporary keychain.

This is not an Apple account password, not the Mac login password, and not the
existing local keychain password. The workflow creates a temporary keychain,
unlocks it with this value, imports the `.p12`, signs the app, and discards the
keychain. GitHub documents creating a temporary keychain for Apple signing here:
<https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications>.

Apple's command-line keychain tool is `security`:
<https://developer.apple.com/legacy/library/documentation/Darwin/Reference/ManPages/man1/security.1.html>.

## MACOS_DEVELOPER_ID

Value: the Developer ID Application signing identity used by `codesign`.

Use the identity for the same certificate exported in
`MACOS_CERTIFICATE_BASE64`. It normally has this form:

```text
Developer ID Application: Example Organization (ABCDE12345)
```

You can copy the identity from Keychain Access by inspecting the Developer ID
Application certificate, or list code-signing identities on the Mac that has the
certificate:

```sh
security find-identity -v -p codesigning
```

Apple documents Developer ID certificates here:
<https://developer.apple.com/help/account/certificates/create-developer-id-certificates/>.
Apple documents the `security find-identity` command here:
<https://developer.apple.com/legacy/library/documentation/Darwin/Reference/ManPages/man1/security.1.html>.

## MACOS_DEVELOPMENT_TEAM

Value: the 10-character Apple Developer Team ID for the same team that owns the
Developer ID Application certificate.

If you have the certificate in Keychain Access, open it and check the
Organizational Unit field. For Apple Developer certificates, that value is the
Team ID. In the signing identity, it is also the final value in parentheses:

```text
Developer ID Application: Example Organization (ABCDE12345)
```

In this example, the secret value is:

```text
ABCDE12345
```

Apple documents Developer ID certificates here:
<https://developer.apple.com/help/account/certificates/create-developer-id-certificates/>.
Apple documents team membership details in Apple Developer account help:
<https://developer.apple.com/help/account/manage-your-team/view-membership-details/>.

## MACOS_BUNDLE_IDENTIFIER

Value: the app bundle identifier used by the generated Xcode project.

The XcodeGen spec reads this value into `PRODUCT_BUNDLE_IDENTIFIER`, and the
generated `Info.plist` uses it as `CFBundleIdentifier`. Use a reverse-DNS
identifier that belongs to the Apple Developer team, for example:

```text
org.slint.visual-editor
```

Apple documents bundle IDs and explicit App IDs here:
<https://developer.apple.com/help/account/identifiers/register-an-app-id/>.

Apple documents `CFBundleIdentifier` here:
<https://developer.apple.com/documentation/bundleresources/information-property-list/cfbundleidentifier>.

## NOTARY_API_KEY_BASE64

Value: the base64-encoded contents of an App Store Connect API private key
downloaded as a `.p8` file.

Create the API key in App Store Connect:

1. Open App Store Connect.
2. Go to Users and Access.
3. Open Integrations -> App Store Connect API.
4. Create a Team API key.
5. Choose the Developer access level for this notarization key.
6. Download the `.p8` private key.

Apple documents the App Store Connect API page and team keys here:
<https://developer.apple.com/help/app-store-connect/get-started/app-store-connect-api>.

Apple documents App Store Connect API key creation here:
<https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api>.

Apple documents role permissions, including the Developer role, here:
<https://developer.apple.com/help/app-store-connect/reference/account-management/role-permissions>.

Apple documents downloading keys here:
<https://developer.apple.com/help/account/keys/revoke-edit-and-download-keys>.

If the downloaded key is named `AuthKey_ABC123DEFG.p8`, encode it with:

```sh
base64 -i AuthKey_ABC123DEFG.p8 -o AuthKey_ABC123DEFG.p8.base64
```

Use the full contents of `AuthKey_ABC123DEFG.p8.base64` as the GitHub secret
value.

This key is used by `notarytool` authentication only. It is not an Apple ID
password, not an app-specific password, and not the Developer ID `.p12`.
Apple documents `notarytool` API-key authentication in the notarization workflow
documentation:
<https://developer.apple.com/documentation/security/customizing-the-notarization-workflow>.

## NOTARY_API_KEY_ID

Value: the App Store Connect API key ID for the `.p8` file stored in
`NOTARY_API_KEY_BASE64`.

The key ID is shown in App Store Connect for the API key. It is also usually in
the downloaded filename:

```text
AuthKey_ABC123DEFG.p8
```

In this example, the secret value is:

```text
ABC123DEFG
```

Apple documents getting a key identifier here:
<https://developer.apple.com/help/account/keys/get-a-key-identifier>.

## NOTARY_ISSUER_ID

Value: the issuer ID shown on the App Store Connect API page for the team key.
It is usually a UUID-shaped value.

Apple documents the App Store Connect API page and team keys here:
<https://developer.apple.com/help/app-store-connect/get-started/app-store-connect-api>.

Apple's App Store Connect API documentation explains that API requests use
issuer ID, key ID, and the private key when generating tokens:
<https://developer.apple.com/documentation/appstoreconnectapi/generating-tokens-for-api-requests>.

The workflow passes `NOTARY_ISSUER_ID`, `NOTARY_API_KEY_ID`, and the decoded
`.p8` key to `notarytool`. Apple documents API-key authentication for
notarization here:
<https://developer.apple.com/documentation/security/customizing-the-notarization-workflow>.

## Checklist

The repository must have all of these GitHub Actions secrets before the macOS
visual editor DMG workflow can sign and notarize:

- `MACOS_CERTIFICATE_BASE64`
- `MACOS_CERTIFICATE_PASSWORD`
- `MACOS_KEYCHAIN_PASSWORD`
- `MACOS_DEVELOPER_ID`
- `MACOS_DEVELOPMENT_TEAM`
- `MACOS_BUNDLE_IDENTIFIER`
- `NOTARY_API_KEY_BASE64`
- `NOTARY_API_KEY_ID`
- `NOTARY_ISSUER_ID`
