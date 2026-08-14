# Design: slint-editor

Status: agreed design, not yet implemented.
This crate does not exist yet; the code described here currently lives in `tools/lsp` (`editor_main.rs`, `editor.rs`, and parts of `preview/`).
Reviewed against the current code; the corrections from that review are folded in below.

## Purpose

`tools/editor` is the standalone visual editor application.
It is a thin binary on top of `internal/editor-preview`: it owns the process, the event loops, the file watching, the auto-updater, and (eventually) the editor UI — nothing more.
"Thin" is the goal, not the starting point: today the editor binary compiles the entire language server and formatter, and this crate is what sheds that.

It is not a language server and must not look like one internally.
The in-process round trip through `lsp_server::Message` framing that `editor.rs` uses today is explicitly not carried over.

The crate is `publish = false` (precedent: `tools/compiler`, `tools/figma_import`).
This is mandatory while `sparklers` is a git dependency, and it keeps `slint-lsp` and `internal/editor-preview` publishable.

## Contents

- The binary entry point and CLI (current `editor_main.rs` and the startup/threading code from `editor.rs`).
- Window setup and editor chrome.
  The editor-specific arm of today's `preview::run()` moves here: creating the editor window, wiring the macOS unified title bar, and driving the `sparklers` auto-updater around the shared engine-start function.
  `sparklers` is a dependency of this crate only — it is a git dependency that would make any published crate unpublishable.
- The session driver: a tokio loop owning an `EditorSession` (from `internal/editor-preview`) and selecting over file-watcher events, preview messages, and the recompile idle timer.
  File watching uses `i_slint_live_preview::file_watcher` as today.
- A direct `LspToPreview` implementation: hand the `LspToPreviewMessage` to the UI thread via `slint::invoke_from_event_loop(|| lsp_to_preview(message))`.
  No serialization, no notifier, no reader thread.
- `PreviewToLsp` message handling: apply `SendWorkspaceEdit` to disk via the shared text-edit code, answer `RequestState`, log `DebugMessage`/`SendShowMessage`, ignore the client-facing rest (`Diagnostics`, `ShowDocument`, telemetry).
  Diagnostics returned by `EditorSession::reload_document` are discarded.
- Packaging and project files: `packaging/`, `dev.slint.VisualEditor.yml`, `macos-project.yml`.
- Eventually its own `build.rs` and the editor window UI (`ui/visual-editor/`), once the shared `global Api` is split.
  Interim state: the UI stays compiled inside `internal/editor-preview` and is selected at runtime via `PreviewUiKind::Editor`; the file tree (`preview/ui/file_tree.rs`) and the macOS title-bar code (`preview/macos_titlebar.rs`) stay there too, because they are coupled to the generated UI types.
  When the UI moves here, import the fonts explicitly (today they come in via the preview's `main.slint`) and fix the symlinked files under `ui/assets/`.

## Must Not Contain

- `ServerNotifier`, `lsp-server`, or any outgoing-request machinery.
  The editor never sends LSP requests.
- LSP feature code (completion, goto, hover, rename), the formatter, or configuration via `InitializeParams`.
- Preview-engine logic.
  Canvas behavior, selection, drop locations, and property editing live in `internal/editor-preview`; fixes belong there so the LSP preview benefits too.

## Build and Packaging Migration Checklist

Moving the binary from `[[example]] slint-editor` in `tools/lsp` to a `[[bin]]` here changes the artifact path
(`…/examples/slint-editor` becomes `…/slint-editor`) and the manifest paths.
Update in the same change:

- Workflows: `visual_editor_windows_msix.yaml`, `visual_editor_macos_dmg.yaml`, `visual_editor_linux_flatpak.yaml` (artifact paths, packaging paths, and the version lookups that key on package name `slint-lsp`).
- Scripts: `build_visual_editor_flatpak.bash`, `generate_visual_editor_flatpak_sources.bash`, `package_macos_visual_editor.bash`, `local_sparkle_update_test.sh`, `build_macos_app_with_cargo.bash`.
- Project files moving with this crate: `dev.slint.VisualEditor.yml`, `macos-project.yml` (binary paths and version lookups inside them).
- `docs/development/macos-visual-editor-dmg.md`, `REUSE.toml` (path-scoped license entries), `.gitignore`.
- `.github/ci_path_filters.yaml`: add `tools/editor/**`, otherwise editor-only changes skip CI.
- The macOS packaging script bundles `ui/visual-editor/example/` as app resources; that path points into `internal/editor-preview` during the interim state.

## How the Three Crates Differ

- `tools/editor`: the visual editor application — process, loops, transport, auto-updater, editor chrome. Not published.
- `internal/editor-preview`: everything the editor and the LSP preview share — document model, `EditorSession`, canvas engine, (initially) both windows' UI. Published.
- `tools/lsp`: the language server — LSP request handling, `ServerNotifier` (single copy, private), `lsp_to_editor` client helpers, formatter, host-language search, `LspToPreviews` and all preview transports (embedded, child-process, remote WebSocket, wasm). Published.
