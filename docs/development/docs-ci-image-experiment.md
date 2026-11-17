<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Docs CI Image Experiment

Compare the existing Linux docs jobs with a prebuilt Ubuntu 24.04 environment on Blacksmith.
Keep documentation sources, tests, runner size, and the existing Cargo cache policy unchanged.
Record container initialization separately from build and test execution.

The image includes native dependencies, fonts, both Rust toolchains, and browsers matching the repository's Playwright version.
Node and pnpm versions remain selected by the existing workflow actions.
Publish the image before switching the docs jobs, then consume its immutable digest.

Blacksmith's [container cache](https://docs.blacksmith.sh/blacksmith-caching/docker-container-caching) automatically retains pulled images.
Its [Docker builder](https://docs.blacksmith.sh/blacksmith-caching/docker-builds) separately retains BuildKit state under `docker/Dockerfile.docs-ci`.
Keep compilation sticky disks outside this comparison to isolate the environment change.

## Results

The initial PR revision measures the existing host jobs while the image builds independently.
Record the baseline and container PR runs here after they complete.
