# Deletion and simplification audit

Scope: manifests, MCP server, project snapshots, preview template, editor sources, validator, Wasm API change, build and packaging scripts, tests, and documentation.
Generated runtime binaries were checked through builds, version checks, hashes, and execution rather than treated as editable source.

## Deletion-focused findings and actions

- Removed the unused validator `--stdio` mode, per-document map, and `didChange` branch.
  The server invokes one saved-file check per process; no caller used the persistent mode.
- Removed the opaque compiled editor IIFE from the preview template.
  The build now consumes the existing readable editor, label-edit, and zoom sources.
- Removed duplicated Wasm/project chunk-loading loops in favor of one byte-reader.
- Removed duplicate source-text input from the file-backed render path.
  A request selects either saved source or explicit source text.
- Removed the unused test-client export.
- Kept source-string rendering for existing callers and the unchanged starter tool.
- Kept the existing preview/code/edit/zoom UI and syntax-highlighting dependencies because their call sites remain active.
  No pending visual-design removal was inferred from this audit.

## Simplification findings and actions

- Reused Slint's compiler URL mapper and font-registration API instead of rewriting Slint source or adding an asset web server.
- Kept dependency capture confined to literal imports and image references reachable from the entry file.
  No directory scan, watcher, include-path search, or arbitrary filesystem resource reader was added.
- Used immutable, content-addressed snapshots and one resource template for all dependency chunks.
  Reads survive MCP restarts and cannot request arbitrary paths from an encoded URI.
- Moved argument checks before capture and validation-hash rejection before snapshot publication.
- Canonicalized paths, checked symlink targets, checked declared-root boundaries before probing dependencies, and handled cache-eviction races.
- Preserved exact UTF-8 bytes in validation, including CRLF line endings.
- Centralized render identity and acknowledgements in `slintPreview` model context.
  Frontend errors retain the last valid instance and report actionable diagnostics with real source paths.
- Predecode images before compilation so invalid assets produce an error acknowledgement instead of a silently missing image.
- Extracted only an explicit team-package file list, with deterministic ZIP metadata and native executable permissions.
  Project snapshots, test files, credentials, and build directories are excluded.

## Validation and boundaries

- Integration tests cover exact-source rendering, validation statuses, same-file color/label/size edits, CRLF, stale hashes, restarts, nested dependencies, immutable snapshots, and root/symlink escapes.
- Browser checks cover the edit cycle, imported components, an SVG image, font registration, ready acknowledgements, and frontend errors.
- A ready acknowledgement records compilation, showing the instance, and a paint opportunity; it does not independently inspect host pixels.
- The archive is verified on its recorded build platform; this run prepares macOS ARM64.
- This prototype supports relative project imports and documented image/font formats, not remote dependencies or include-path aliases.
- Inline direct manipulation retains its existing single-Button literal-property scope.
