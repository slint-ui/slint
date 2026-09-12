# Releasing Material Components

Use this checklist when publishing a new Material Components version.

## Prepare the Release

1. Choose the version according to semantic versioning.
   Use a minor version when the release adds public components, properties, callbacks, or functions.
2. Update `MATERIAL_ZIP_VERSION` in `.github/workflows/material.yaml`.
3. Update every versioned download and import example in the Material documentation.
4. Document the required minimum Slint version in the getting-started guide and packaged `README.md`.
5. Complete `docs/src/content/docs/changelog.mdx` with the version, release date, new APIs, fixes, accessibility changes, and migration notes.
6. Run `scripts/package-release.sh VERSION OUTPUT_DIRECTORY` and inspect the ZIP contents and generated checksum.

Do not add the version being prepared to `release/released-versions.txt` yet.
That file lists releases that are already published and must be copied into the next deployment.

## Validate the Release

1. Run the Material gallery tests.
2. Build the documentation.
3. Build and test the web gallery.
4. Build and test the Android APK and AAB artifacts.
5. Confirm that the new archive contains `material.slint`, `README.md`, and `LICENSE.md` under its versioned directory.
6. Confirm that every version in `release/released-versions.txt` is still available in the deployment preview.

## Publish the Release

1. Merge the release changes into `master` after CI succeeds.
2. Run the Material workflow from `master` with **Deploy production** enabled.
3. Verify the documentation, web gallery, Android downloads, new ZIP, checksum, and every retained ZIP on `material.slint.dev`.
4. Add the published version to `release/released-versions.txt` in a follow-up change so future deployments retain it.

## Update the Templates

Each template vendors a copy of the published Material archive.
Replace that copy with the exact contents of the new ZIP, update its Slint dependency when required, and test the template in a separate branch and pull request:

- `slint-ui/material-rust-template`
- `slint-ui/material-cpp-template`
- `slint-ui/material-nodejs-template`
- `slint-ui/material-python-template`

Do not copy the working tree directly into a template.
Using the published ZIP ensures that template users receive the same files that were validated and released.

## Announce the Release

Publish an announcement that links to the documentation and changelog and summarizes the notable additions and migration requirements.
Use the usual Slint news and social channels.
