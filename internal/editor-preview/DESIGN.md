# Design: i-slint-editor-preview

Status: agreed design, not yet implemented.
This crate does not exist yet; the code described here currently lives in `tools/lsp`.
Reviewed against the current code; the corrections from that review are folded in below.

## Purpose

`internal/editor-preview` is the shared crate between the Slint LSP (`tools/lsp`) and the standalone visual editor (`tools/editor`).
It contains everything both applications need to represent, edit, and preview a Slint project:
the document model, the editing session, and the preview canvas engine.

It is one crate by decision, not an oversight.
The LSP builds the preview UI in anyway, so splitting the document model into its own crate would only linearize the dependency chain and hurt build parallelism.

## Contents

### Document Model

The compiler-level representation of a project as it is being edited.
Moves from `tools/lsp/common*`:

- `DocumentCache`, `CompilerConfiguration`, `OpenImportCallback` (from `common/document_cache.rs`).
- Text-edit application and diffing (from `common/text_edit.rs`).
- `common.rs` moves as a whole — core types (`ElementRcNode`, `ComponentInformation`, `SingleTextEdit`, `uri_to_file`, `ByteFormat`, `Result`) and the small helpers around them — except for the two items listed under "Stays in tools/lsp" below.
- Position/range mapping helpers (from `util.rs`).
  Note: the LSP's language features are the heavier consumer of `util.rs`; it moves because the engine needs it too and `tools/lsp` depends on this crate anyway.
- `component_catalog.rs` and `rename_element_id.rs` (used by the preview palette and inline rename).
- `rename_component.rs`, wholesale.
  The preview's in-place rename calls its full engine, and the visual editor will use it too.
  Only the `.slint`-side rename moves; the host-language follow-up (`host_language_search.rs` and its scheduling) stays in the LSP for now and may follow later.
  `rename_component` only classifies whether a host-language follow-up is needed; it does not call the search itself.
- `create_import_edit` (from `language/completion.rs`), together with its two private helpers (`create_import_edit_impl`, `find_import_locations`).
  The staying completion code keeps using them through this crate.

`lsp-types` is the edit currency of this layer (`Url`, `WorkspaceEdit`, `Range`).
That dependency is accepted; replacing the edit representation is out of scope.

### Editing Session

`EditorSession` takes over the six preview-related fields of the LSP's `language::Context`:

- `document_cache`
- `to_preview`
- `open_urls`
- `pending_recompile`
- `to_show`
- `preview_config`

The LSP's `Context` keeps its three server-only fields (`server_notifier`, `init_param`, `host_language_rename_dont_ask_again`) and embeds an `EditorSession`.
The six fields stay reachable (public or via accessors): the LSP request handlers and both applications' event loops read them outside the session methods.

The five document-lifecycle functions become methods on `EditorSession`:

- `reload_document`
- `show_preview`
- `trigger_file_watcher`
- `send_state_to_preview`
- `send_files_to_preview`

`load_document` and `drop_document` stay public as well; the LSP's `open_document`/`close_document` handlers call them directly.

`reload_document` and `trigger_file_watcher` **return** the compile diagnostics instead of publishing them.
The return type carries the version captured at the right moment — `Vec<(Url, SourceFileVersion, Vec<Diagnostic>)>` — because `delete_document` records the version *before* the drop; re-deriving it at publish time regresses the stale-diagnostics fix for VS Code.
The LSP forwards the returned diagnostics to its client; the editor discards them.
This removes the last `ServerNotifier` use from the shared code.
Two call sites need active work in `tools/lsp`, not just a signature change:
`main.rs` currently discards the `trigger_file_watcher` result, and the wasm `trigger_file_watcher` export has a fixed signature and must publish internally.

### Preview Engine

The canvas and its supporting analysis, moving from `tools/lsp/preview*`:

- `preview.rs` core: `PreviewState`, the compile/reload loop, `send_workspace_edit`.
  `run()` loses its editor-specific arm: the sparkle auto-updater and title-bar wiring move to `tools/editor`, which does its own window setup around a shared engine-start function.
- `element_selection`, `drop_location`, `outline`, `properties`, `ext`, `undo_redo`, `eval`, `preview_data`.
- `preview/remote.rs`: the remote-preview dialog wiring and `RemoteDiscovery` (a `PreviewState` field, coupled to the generated UI types).
  This brings `mdns-sd` along.
  Only the remote WebSocket *transport* (`connector/remote.rs`) stays in `tools/lsp`.
- The UI-facing models under `preview/ui/`, including `file_tree.rs` and `macos_titlebar.rs`.
  Both are earmarked for `tools/editor`, but they import `include_modules!()`-generated types and are called back from `ui.rs`, so they stay here until the UI split.
  The `objc2*`/`raw-window-handle` dependencies come along; `sparklers` does **not** (see "Must Not Contain").
- The message dispatch entry point `lsp_to_preview(LspToPreviewMessage)` as a public function.
  The four `preview.rs` functions it calls (`invalidate_contents`, `delete_document`, `set_contents`, `config_changed`) and `get_current_style` become public.
  The current `connector.rs` does not move: it is the parent module of the three transports, which stay in `tools/lsp` under a new `connector` module there.
  `resource_url_mapper` is injected by the application instead of called across the boundary.
- Initially the whole `.slint` UI tree (`tools/lsp/ui/`), with the runtime `PreviewUiKind` switch selecting the preview or editor window.
  Splitting the monolithic `global Api` (~129 members, ~20 genuinely shared) and moving each window's UI to its application is planned follow-up work, not part of the initial extraction.
  Moving `ui/` breaks the seven symlinks under `ui/assets/` (into `logo/` and `tools/viewer/remote/assets/`); fix them in the same change.
  The crate's `build.rs` must set `SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` like the LSP's does.

### Test Support

Shared test fixtures move behind a `testing` feature:
`common/test.rs` and the document-cache fixtures from `language/test.rs` (`loaded_document_cache`, `complex_document_cache`, `load`), which moving code uses pervasively.
This is a sanctioned exception to the boundary rule below.
The dev-dependencies they need (`i-slint-backend-testing`, `spin_on`) are declared here as well.

## Dependencies and Features

- `i-slint-compiler`, `lsp-types`, `i-slint-live-preview` (feature `protocol`).
  The `LspToPreview` and `PreviewToLsp` **traits** move from `tools/lsp/common` into `i-slint-live-preview`'s `protocol` feature, next to the message enums they wrap; drop the unused `std::any::Any` bound on `LspToPreview` while moving.
  The `LspToPreviews` fan-out does **not** move — it owns the remote transport, which stays in `tools/lsp`, and moving it would create a dependency cycle.
  The `wasm_prelude` (`UrlWasm`) currently defined in `wasm_main.rs` also hoists into live-preview's `protocol`, next to the re-exported `lsp_types`.
- `slint`, `slint-interpreter`, `i-slint-core`, and `i-slint-backend-selector` only behind an `engine` feature.
  Note: `tools/lsp` keeps its own `slint`/`slint-interpreter`/`i-slint-core` dependencies regardless, because its transports use them; the lean LSP build remains the existing no-default-features formatter path.
- The backend/renderer selection ladder (`backend-*`, `renderer-*` passthrough features on `tools/lsp`) is duplicated on this crate and re-forwarded by `slint-lsp`, because VS Code, SlintPad, and CI select those features by name.
- Dependencies that move here from `tools/lsp`: `nucleo-matcher`, `clru`, `by_address`, `rfd` (file tree dialogs), `mdns-sd` (remote discovery), `i-slint-backend-selector`.
- The engine must keep compiling for wasm32; SlintPad's preview runs through it.
- Moving code must not rely on the implicit crate-root re-exports (`crate::Result`, `crate::Url`, `crate::test`); use real imports.

## Stays in tools/lsp

- `ServerNotifier` (collapsing to a single definition there) and everything `lsp-server`.
- The `lsp_to_editor` helpers (publishing diagnostics, `showDocument` requests) — they talk to an LSP client.
- `LspToPreviews` and the message transports: `EmbeddedLspToPreview`, `ChildProcessLspToPreview`, the remote WebSocket transport, and the wasm connector, in a new `connector` module.
- LSP request/notification handling (`language/` feature modules), the formatter (`fmt/`), `host_language_search`, `token_info`.

## Must Not Contain

- `sparklers`.
  It is a git dependency, which `cargo publish` rejects, and this crate sits in the middle of the publish chain.
  This crate must stay publishable; the auto-updater lives only in `tools/editor`, which is not published.
- Anything from the "Stays in tools/lsp" list.

## Boundary Rule

Admit only types and operations on the document model, the editing session, and the preview canvas.
No transport plumbing, no LSP request handling, no application chrome.
This crate must not become a new `common` grab bag.

## Publishing and Workspace Plumbing

Internal crates are published with exact-version pins.
This crate needs: a root workspace `members` entry, a `[workspace.dependencies]` line with `version = "=<current>"`, a `LICENSES/` symlink directory, and a `scripts/publish.sh` entry between `internal/live-preview` and `tools/lsp`.

## Known Debt

- `PREVIEW_STATE` moves as-is: a thread-local singleton, so one preview instance per thread.
  Making the engine state injectable is future work.
- The `global Api` split (per-window `.slint` compilation with a small Rust trait for the engine's write-backs) is deferred.
- `host_language_search` may move here later, together with a decision on where host-language rename belongs.
