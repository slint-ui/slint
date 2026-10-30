---
name: slint-setup
description: Set up slint-viewer when the user runs Slint plugin setup or asks to install or configure the viewer.
---

# Slint Viewer Setup

Check for `slint-viewer` on PATH and run `slint-viewer --version` and `slint-viewer --help`.
If it supports `--check` and `--screenshot`, report the version and finish.

Otherwise, explain that the optional viewer provides local compiler checks, screenshots, and live preview.
Offer to install or upgrade it, or skip setup.
Respect any choice the user has already made.
If they skip, finish without further installation prompts.

If they accept, follow [Installing Slint CLI tools](../slint/tools-install.md) for their operating system and architecture.
Prefer a prebuilt binary matching the project's Slint version when available.
Ensure the install directory is on PATH in the Codex environment.

Verify `slint-viewer --version` and compile a minimal temporary `.slint` file with `slint-viewer --check`.
Report whether setup succeeded.
