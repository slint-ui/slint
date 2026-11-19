---
name: slint-plugin
description: Use the Slint plugin in Claude Code to look up Slint APIs, check .slint diagnostics, and render local previews.
---

# Slint Plugin for Claude Code

## Find the Slint Project

Look beside the target `.slint` file and up through the workspace for the applicable `slint-project.json`.
Resolve `main`, include paths, and library paths relative to the directory containing the project file.
Use `main` for an app preview when the project file selects the entry point.

For Rust, check the owning crate's `Cargo.toml` and any inherited workspace dependencies.
If the crate uses `slint_build`, read its build script for the entry point and compiler settings.
Use `Cargo.lock` for the resolved Slint version. The project file's `edition` controls language behavior.

## Look Up Slint APIs

Find the project's Slint version in its build configuration or lockfile.
Call `slint-docs` `search` for the element, property, widget, or API you need.
Pass `version` when the project uses an older release.
Call `fetch` with the result's URL exactly as returned to read the page for that release.

If the documentation tools are unavailable, open the matching release at
`https://releases.slint.dev/<version>/docs` or use the [current Slint docs](https://docs.slint.dev/).

## Check and Preview Slint Files

Use the plugin's `slint-lsp` diagnostics after editing when the language server is installed.
Use `slint-viewer --check` for a local compile check and `slint-viewer --screenshot` for a local render.
Pass `--project-root <directory>` to apply project settings to a specific `.slint` file.
To use the project file's `main`, pass `slint-project.json` as the viewer input.
Read the PNG with Claude Code's file-reading tool and inspect the UI before judging its appearance.
When sharing a local screenshot in a terminal session, give its absolute path.
Use the running app or its tests to verify host-language callbacks, models, and application state.

- [Install Slint tools](https://github.com/slint-ui/ai-plugins/blob/master/skills/slint/tools-install.md)
- [Local rendering and runtime debugging](https://github.com/slint-ui/ai-plugins/blob/master/skills/slint/reference/debugging-and-mcp.md)

## Find Slint Development Guidance

For language and layout, common errors, host integration, and visual polish,
use the [Slint development reference index](https://github.com/slint-ui/ai-plugins/blob/master/skills/slint/SKILL.md#reference-files).
