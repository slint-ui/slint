# Releasing Material Components

Use this checklist when publishing a new Material Components version.

## Prepare the Release

1. Choose the version according to semantic versioning.
   Use a minor version for backward-compatible additions to public components, properties, callbacks, or functions.
2. Update `MATERIAL_ZIP_VERSION` in `.github/workflows/material.yaml`.
   Add any previously published versions missing from `release/released-versions.txt` so their ZIPs remain available.
   Keep the new version out of that list; the workflow builds its ZIP from source.
3. Update every versioned download and import example in the Material documentation.
4. Document the required minimum Slint version in the getting-started guide and packaged `README.md`.
5. Update `docs/src/content/docs/changelog.mdx` with the version, release date, changes, and migration notes.

## Validate the Release

1. Run the Material gallery tests.
2. Build the documentation.
3. Build and test the web gallery.
4. Build and test the Android APK and AAB artifacts.
5. Run `scripts/package-release.sh VERSION OUTPUT_DIRECTORY` from `ui-libraries/material` and verify the ZIP and checksum.
6. Confirm that every version in `release/released-versions.txt` is still available in the deployment preview.

## Publish the Release

1. Merge the release changes into `master` after CI succeeds.
2. Run the Material workflow from `master` with **Deploy production** enabled.
3. Verify the documentation, web gallery, Android downloads, new ZIP, checksum, and every retained ZIP on `material.slint.dev`.

## Update the Templates

Update each template from the published ZIP, adjust its Slint dependency if needed, and test it:

- `slint-ui/material-rust-template`
- `slint-ui/material-cpp-template`
- `slint-ui/material-nodejs-template`
- `slint-ui/material-python-template`

## Announce the Release

Publish an announcement that links to the documentation and changelog and summarizes the notable additions and migration requirements.
