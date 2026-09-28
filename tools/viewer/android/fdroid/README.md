# F-Droid

`dev.slint.viewer.yml` is the F-Droid build recipe for the Slint Viewer.
The copy that F-Droid uses lives in [fdroiddata](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/dev.slint.viewer.yml);
keep the two in sync and test changes here first.
F-Droid's copy has no license header: its canonical format strips comments.

F-Droid publishes our signed APK from GitHub releases rather than signing its own build,
so its rebuild has to be byte-identical to ours; see the header of `../build-native.sh`.
The `Android reproducible build` workflow runs this recipe in the F-Droid build server image
and compares the result with our own build.

The release run of the nightly workflow uploads `slint-viewer-<abi>.apk` per ABI to the GitHub release,
along with `slint-viewer-android-version.txt` naming the release version code.
F-Droid polls that file through the `releases/latest/download/` redirect, so it only sees a release once it's published,
then copies the recipe's three build entries for the new version (`VercodeOperation` gives each its version code)
and verifies each rebuild against the entry's `binary` URL.

The `AllowedAPKSigningKeys` fingerprint is that of the release keystore the nightly workflow signs with.
A different key means F-Droid rejects the APK, so treat the keystore as irreplaceable.
