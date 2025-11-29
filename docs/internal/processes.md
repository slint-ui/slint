<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Internal procedures

## Repositories

Almost all the developmnent of Slint is done in the https://github.com/slint-ui/slint mono-repository.

There are a few other repositories in the organization, they are either
 - Template repositories which are on their own as user are invited to use them as templates for their own projects
 - Forks of 3rd party repository which member of the slint organization have made pull requests
 - Experiments or short lived projects or example that are ont part of the product
 - Private repositories used for specific customer projects
 - Website repository
 - Other repositories for code that needs to be kept private as it is part of a proprietary product

All the code of the Slint product is in the mono-repository.
Having a single mono-repository makes it easier to do changes accross the whole product and

Different files in the Slint repository have different licenses, and all the license are tracked with [REUSE](https://reuse.software/).
 - Most files have headers that are compatible with the [SPDX](https://spdx.org/licenses/) license identifiers
 - Otherwise the license is tracked in the REUSE.toml in the root of the repository

## Organization members

The administrators of the slint-ui Github organization are the founders of SixtyFPS GmbH.
All employees of SixtyFPS GmbH are members of the organization.
External contributors may also be invited to become members of the Github organization.

## Code changes

The development happens in the `master` branch.
Developers can create a branch `<developer-name>/<feature-name>` in the slint-ui/slint repository to make a PR, although some developers prefer to work on their own forks.
Only member of the organization can push to a branch in `slint-ui/slint`, so outside contributors need to create a PR from a branch in their own fork.

For simple commits that do not need to be reviewed and are unlikely to break the CI, the commit can be directly pushed to the `master` branch without creating a PR.
The master branch is protected, only admins are able to push directly to the `master` branch.

A PR should be reviewed before being merged, unless the PR is trivial. Trivial PRs may be merged without review.

Reviewers can leave comments on the PR. Some comments are just nitpicks but some other comments should be addressed before merging the PR. Reviewers should make an effort to clearly indicate what needs to be addressed to obtain approval.

Once approved, the author of the PR can merge the PR if he has the rights to do so.
For external contribution, the reviewer must merge the PR.

Ideally, the PR should keep a clean history with self contained commits.
If the history of the PR is clean, the PR can be "Rebased and merged" so that the individual commits are merged into the `master` branch.
But some developer do not take care to keep a clean history and a PR may contain many commits or "autofix" commits.
In this case it is preferable to "Squash and merge" the PR in the Github UI.
The submitter can edit the commit message to make it nicer and removed the artifacts of all the "fixups".

## CI

The CI is driven by Github Actions.
The CI is triggered for any PR to the `master` branch, or to a branch that starts with `feature/`.
The CI is also triggered for every push to the `master` branch.
A new commit on a branch will cancel the previous CI run on that branch if it is still running.

## Nightly build

There is a nightly build that runs every night.
It will build release artifacts and upload them to the `nightly` Github release.
And will also generate the docs, wasm build of examples, and publish them on snapshots.slint.dev/master

It will also run a few extra tests.

## Release

New minor releases of Slint are done every couple of months.

About a week before the expected release, we issue a "call for testing" as a GitHub discussion.

During the release process, we ask people not to merge to the `master` branch.

The release is done from the `master` branch.

The process of releasing is done by following the steps in the release_checklist.md file. (currently on our internal wiki)

The ChangeLog is manually updated by looking at the commit history.

## Issues

Users are reporting bugs and feature requests on the [Github issue tracker](https://github.com/slint-ui/slint/issues).

The process to triage and assign label is documented in the the <../triage.md> file.





