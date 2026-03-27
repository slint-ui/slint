---
title: Development Cycle
description: Tools, processes, and infrastructure used for Slint SC development.
---

This section describes what tools we use and processes we follow, to develop slint. (ISO 26262-8 sections 11, 12 and 13)

## Language / Compiler

Slint SC is written in Rust and requires rustc+cargo version 1.88 or newer to build it.

## Dependencies

Because Slint SC is intended for embedded systems, there are very few internal
dependencies, and no external dependencies for the runtime portion of Slint SC.

At build-time, there may be dependencies on certain external libraries, for the purposes
of image decoding.

## Procedure for Version Control/Configuration Management

Slint is an open-source project hosted on GitHub. <https://github.com/slint-ui/slint>

## Code Reviews

Admins can push directly to the master branch, but all other
changes must go through a code review process, involving Pull Requests (PRs).
PRs enable developers to easily review each other's work, add specific comments,
ask questions, and allow/deny changes.

## CI system used

Continuous Integration (CI) is driven by Github Actions. The actions are triggered for any PR (including drafts) to the master branch, or to a branch that starts with feature/. Actions are also triggered for every push to the master branch. A new commit on a branch will cancel the previous action on that branch if it is still running.

## Nightly Builds

There is a nightly build that runs every night. It will build release artifacts and upload them to the nightly Github release. It will also generate the docs and examples, and publish them on snapshots.slint.dev/master

It will also run a few extra tests (like running cargo update to check for broken dependencies).

## Reporting Bugs

Issues are tracked in github: <https://github.com/slint-ui/slint/issues>
