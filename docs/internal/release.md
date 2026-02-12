# Release process

This document describes the Slint release process

## Before a release

* Check that the dependencies are up-to-date:
  - `cargo update --verbose` gives some hint on what rust dependency to update
  - Corrosion in api/cpp/CMakeLists.txt
  - Tree sitter: in `.github/workflows/ci.yaml` for the `tree-sitter` job, bump the `tag`
    to the latest release as per https://github.com/tree-sitter/tree-sitter/releases

* Verify that the list of supported platforms in docs/astro/src/content/docs/guide/platforms/desktop.mdx matches what we * Publish the helper_crates, if needed

* Update version number in the documentation  (Only for major release)
  - Crate documentation have sample .toml files (api/rs/lib.rs, api/rs/build/lib.rs, api/rs/README)
      - `sed --follow-symlinks -i 's/^\(slint.*\) = ".*"$/\1 = "1.16.0"/' **/*.rs **/*.md`
  - The `[dependencies.slint]` in mcu.md

* Check the `zed` extension has the latest sha-1 for the tree-sitter repo in editors/zed/extension.toml
  in the `grammars.slint` section

## Branching

About a week before the expected release, create a temporary `pre-release/<major.minor>` branch based on the latest `master`.
Use this branch to collect fixes and run testing (apply cherry-picks here).

 1. Create the branch

    ```sh
    git push origin origin/master:pre-release/<major.minor>
    ```

 2. Change .github/workflows/schedule_nightly_snapshot.yaml to include that pre-release branch in the matrix.
    This commit needs to be done in the `master` branch.

 3. Send a discussion in the ["Show And Tell" category](https://github.com/slint-ui/slint/discussions/categories/show-and-tell)
    with "Call for testing for Slint \<version\>"  with a link to the testing instructions
    (the [nightly release tag](https://github.com/slint-ui/slint/releases/tag/nightly))
    and a link to the ChangeLog.
    The discussion can also include the highlights of the release.

 4. Forward links to this call for testing on social media (Twitter/BlueSky/Mastodon/...)

During that week more testing can be done and we monitor new issue and report from user closely in order to address regressions in time.
From the point of branching, only non-destabilizing bugfixes should be included in the branch.

Bugfixes should first get submitted to the `master` branch using the normal process.
PR and issue that should be backported can be tagged with the `candidate-for-bugfix-release` tag.

The commits can then be cherry-picked into the branch by the release manager with the `-x` option
to include a reference to the original sha1.

```sh
git cherry-pick -x <sha1>
```

In the mean time, the version in the master branch can be updated
 - On the master branch, run https://github.com/slint-ui/slint/actions/workflows/upgrade_version.yaml
 - Set the version number to the next minor release `1.y.0`

## Release

 - Make sure the ChangeLog and its date are accurate.
   The ChangeLog updates also need to be done in the pre-release branch.

 - Check that the CI is green for the top of the branch commit

 - **Trigger a build of binary artifacts** (docs, demos, etc.) on https://github.com/slint-ui/slint/actions/workflows/nightly_snapshot.yaml
    Select the right `pre-release/x.y` branch, and choose false for private and true for release.
    As a result artifacts will be built and made available for download and a new VS code extension be built and uploaded to the market places (open-vsx.org and microsoft).

 - **Publish to crates.io** using the `./scripts/publish.sh`.
    (This can be done in parallel to the nightly_snapshot build)
    Before running the script, make sure that your working directory is clean and that you are checked out on the same commit as the one for which the nightly_snapshot.
    - If new crates were uploaded to crates.io, go to the crates.io settings and send permission invitations

 - **Publish to npm:** Trigger a build on https://github.com/slint-ui/slint/actions/workflows/publish_npm_package.yaml
    Select the right `pre-release/x.y` branch, and choose false for private and true for release.

 - **Publish to PyPi:** Trigger a build on the following workflows on the right branch and choose `true` for release.
   The deployments will also need to be approved
  - https://github.com/slint-ui/slint/actions/workflows/upload_pypi.yaml
  - https://github.com/slint-ui/slint/actions/workflows/upload_pypi_briefcase.yaml
  - https://github.com/slint-ui/slint/actions/workflows/upload_pypi_slint_compiler.yaml

 - **Publish the blog post** (if any). Remove the `DRAFT: ` from the title, check the date, and push to `prod` on the `slint/website` repo.

- **Create the GitHub release**: The nightly_snapshot will create a draft release on GitHub.
  Publishing a release with the right text is important as it will be send by mail notification for these watching the repository for release or discussions.
   - Edit the beginning of the description to include link to the blog post (if any).
   - Edit the changelog link in the draft to point to the actual release
   - For minor releases with blog post, select **Create a discussion for this release** (remember the id)
   - Publish the release
   - Add the discussion ID in the published blog post
   - Edit the discussion text to remove links to artifacts

- **Publish to https://components.espressif.com**: Trigger a build of https://github.com/slint-ui/slint/actions/workflows/upload_esp_idf_component.yaml from the right branch and choose false for private and true for release.
   (This needs to be done after the creation of the tag, otherwise th build would be broken for users until the tag is created)

- Update the `release/x` and `release/x.y` branches
  ```bash
  git fetch
  git push origin v1.y.z:refs/heads/release/1.y
  git push origin v1.y.z:refs/heads/release/1
  ```

- Ask Simon to create a new release in `meta-slint`

- Publish the **figma extension**: https://github.com/slint-ui/slint/blob/master/tools/figma-inspector/PUBLISH.md
  (If there was changes in the figma extension)

- Release the **zed extension**:
  Fork the repository https://github.com/zed-industries/extensions (need to be in your personal account because PR from organization won't allow maintainer to update it)
  ```bash
  git clone https://github.com/zed-industries/extensions && cd extensions
  git pull --rebase
  git submodule update --init extensions/slint
  git -C extensions/slint fetch && git -C extensions/slint checkout v1.y.z
  # update the extensions.toml to the same version as Slint
  bash -c 'VER=$(grep -m1 "^version" extensions/slint/editors/zed/extension.toml); sed -i "/^\[slint\]/,/^\[/{s/^version = \".*\"/$VER/}" extensions.toml'
  git commit -a -m "Update slint extension"
  git push git@github.com:ogoffart/zed-extensions-fork HEAD:update-slint  # (replace the url with your fork)
  # Open a PR with the given URL
  ```

 - Send **Social media** posts to announce the release with link to the blog post for minor release, or to the release tag on github for patch releases.

 - **Release Party**: Use the next office hours call to celebrate the release. 🥳


## Post-release checks

* Check that the build of https://docs.rs/crate/slint/latest and https://docs.rs/crate/slint-interpreter/latest succeeded

* Check that the [`versions.json`](https://github.com/slint-ui/www-releases/blob/master/releases/versions.json) is accurate.
  (Version of the nightly build and no duplicated version)
  FIXME: the release scripts or version upgrade scripts might need fixes

* Notify Torizon guys to update the base images that contain Slint or create PR for https://github.com/commontorizon/Containerfiles

* [Update tree-sitter configurations for editors](https://github.com/slint-ui/wiki/blob/309a3b0327731ba2cfb229595e0fa7209ba868c6/infrastructure/release_checklist.md?plain=1#L91)

## Patch Releases

After the release, monitor for bug report about regressions or critical issue.
These can be tagged with `candidate-for-bugfixes-release`.
Only non-destabilizing bugfixes and regressions should go in the branch.

If it is decided that there are enough reason to make a patch release, one can make a patch release.

 - Update the version in the branch with https://github.com/slint-ui/slint/actions/workflows/upgrade_version.yaml

 - Cherry-pick commits with `-x`

 - Follow the instructions from the `Release` section
