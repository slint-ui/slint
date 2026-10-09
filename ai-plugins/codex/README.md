# Slint Codex plugin

Develop Slint source with Codex, validate it with the native LSP, and show Slint Wasm previews inline in compatible hosts.
Connect a separately installed Visual Editor to exchange annotation feedback and inspect its canvas.
The Codex implementation extends the official plugin rooted at `ai-plugins/`.
Its shared skill and documentation MCP connection remain available to other assistants.
The validator and preview interpreter are built from the same checkout, without a release version pin.

## Build and install

Use Node.js 20 or newer, pnpm, Python 3, Rust, and `wasm-pack`.
Install the Wasm target with `rustup target add wasm32-unknown-unknown`.
From the monorepo root, run:

```sh
pnpm install --filter slint.codex.plugin --frozen-lockfile
node ai-plugins/codex/scripts/build-runtime.mjs
codex plugin marketplace add ./ai-plugins
codex plugin add slint@slint
```

Restart Codex and select Slint from the Slint marketplace in a new chat.
The CLI supports the development skill and tools but does not display an inline preview.

The build uses the existing native LSP, editor MCP bridge, and Wasm interpreter crates.
The preview template and JavaScript modules are bundled with esbuild into `runtime/preview.html`.
Syntax highlighting uses the Figma inspector themes and the shared Slint grammar.
Run `pnpm --filter slint.codex.plugin build` to rebuild the preview without rebuilding Slint.
The repository logo is symlinked; the build resolves it into `runtime/slint.svg` for installation and ZIP packaging.
Runtime builds are staged and replace the previous runtime only after all assets are ready.
Missing or incomplete runtimes expose no preview tools and do not prevent server startup.
Generated binaries and their source revision are stored in the ignored `runtime/` directory.
The preview bundles the required Wasm JavaScript glue and reads compressed Wasm through the host's MCP resource bridge.
The HTML stays below 1 MiB, and each Wasm resource holds at most 256 KiB of compressed bytes.
The preview verifies the reconstructed Wasm hash before initializing it.
A browser cache retains one runtime, containing verified Wasm bytes.
Changing the runtime replaces that entry; unavailable browser storage falls back to MCP resource reads.
Run `pnpm --filter slint.codex.plugin test` after building to check resource size and restart behavior.
Install Chromium with `pnpm exec playwright install chromium --only-shell`.
Run `pnpm --filter slint.codex.plugin test:layout` to check fullscreen and inline dimensions in a browser.
After installation, run `node ai-plugins/codex/tests/desktop-resources.mjs` to check resource delivery through Codex's backend.
Pass the desktop app's bundled Codex executable as its first argument to test that backend version.
This checks the installed package and resource transport; visible inline rendering still needs a check in the app.
The bundled native LSP supplies validation directly; agents do not need to find an LSP executable.

## Follow master

Update this branch from upstream master, rerun the build command, and reinstall the plugin.
The build reads Slint's version from generated artifacts and records the checkout's Git revision.
An installed plugin uses that build until it is rebuilt and reinstalled.
Commit runtime source changes before building so the recorded revision identifies the source.

## Team testing

Teammates can check out this branch and run the same commands on their machine.
Each developer builds their native LSP for their platform.
No Site account, invitation, or hosted plugin identity is required.

## File-backed previews

Call `render_slint` with an absolute `path`, the next `revision`, and `validatedProjectHash` returned by `validate_slint`.
Validation checks project fingerprints before and after the LSP operation.
Rendering rejects changes to the entry, imports, assets, or runtime revision.
Supply `projectRoot` when relative imports or assets need a broader root than the entry file's directory.
The preview reports source identity and ready/error acknowledgements through model context.
These acknowledge the renderer, not independent visual inspection of the host window.

Relative Slint imports and re-exports, PNG/JPEG/SVG/WebP images, and TTF/OTF fonts are supported.
Empty image URLs remain empty images and do not create file dependencies.
Dependencies stay inside the declared root, including symlink targets.
Network dependencies, include-path aliases, and absolute dependency references are unsupported.
Snapshots are immutable, private to the local user, and retain the latest 32 submissions in the system temporary directory.
Each snapshot allows 128 files, 8 MiB per file, and 16 MiB total; the entry allows 64 KiB and imported Slint sources allow 1 MiB.
An expired snapshot needs a fresh render submission.

## Publishing and team packages

Run `node ai-plugins/codex/scripts/package-plugin.mjs /absolute/path/Slint.zip` after building.
The archive includes the official assistant manifests, Slint skills, native LSP, editor MCP bridge, Wasm, source revision, and marketplace catalog.
The generated `slint-ui/ai-plugins` repository currently mirrors source from `release/1` without a build step.
A source-only install keeps the shared skill and docs connection but exposes no preview tools.
The editor bridge also requires a built runtime or platform package.
Public runtime artifact delivery requires a separate publishing change; native binaries remain excluded from the monorepo.
It contains only an explicit package file list, without project snapshots, credentials, build caches, or test files.
The generated ZIP has deterministic file ordering, timestamps, and executable permissions.
The included native runtime works on the recorded platform and architecture; other platforms build from this checkout.

## Visual Editor Annotations

Install the Visual Editor separately from a compatible checkout supporting chat registration and annotation tools.
The plugin packages `slint-editor-mcp` alongside the LSP and launches it independently of the Wasm preview server.
The documentation, preview, and editor bridge are separate MCP connections within the Slint plugin.
The editor bridge needs only its native executable; it does not require Wasm preview assets.

Explicitly ask Codex to connect this chat to the Visual Editor.
Selecting the Slint plugin or editing source does not register a chat automatically.
Codex discovers editors within this chat's working directory and asks which instance to use if several match.
Registration replaces the project's previous destination chat.

Send annotations from the editor to queue feedback to the registered chat.
Feedback includes source context, chronological conversation, and IDs for new user messages.
Codex can inspect the canvas, reply in the annotation thread, and resolve an addressed thread.
Canvas screenshots wait for current source compilation and include selection and annotation popovers.
Resolution permanently removes the thread and replies; history and reopening are unavailable.

After verifying this integration, disable the separate Slint Visual Editor plugin and any standalone editor MCP entry.
Use the editor tools bundled in Slint to avoid duplicate connections.

The preview surface is transparent and follows the host's colour scheme.
Use the hamburger menu to switch between Preview and Code.
Keyboard zoom remains available; edits happen through the saved Slint source in chat.
Opening the preview in a separate tab fits it to the available width and height, preserving its proportions.
Inline previews retain their compact chat dimensions.
Third-party notices are in `THIRD_PARTY_NOTICES.txt`.

## Preview screenshots

Codex can call `get_preview_screenshot` after rendering to inspect the actual inline Slint pixels.
Pass the `previewId`, revision, and source hash returned by `render_slint`.
The tool returns a PNG image content block when ready, or a pending, unavailable, or error status.
It rejects source identity mismatches.
The preview captures once after drawing a source revision and publishes through an app-only tool.
For later runtime state, call the screenshot tool once with `fresh: true`, then retrieve using the returned `captureId` without repeating `fresh`.
The open preview redraws its existing instance without recompiling source or resetting interaction state.
A waiting resource request delivers the capture command without continuous screenshot recording or blocking other MCP calls.
Fresh capture requests require an active preview; a closed preview remains pending until the caller stops waiting.
Captures exclude host controls and CSS zoom.
The cache retains 32 submissions with a maximum PNG size of 4 MiB and dimensions of 4096 pixels.
Captures are private local files and survive MCP server restarts.

## Model and preview payloads

Render results give the model submission status, canonical paths, canvas dimensions, and preview, source, project, and runtime identity.
The complete source and dependency resource manifest are delivered to the preview in tool-result `_meta.preview`.
Preview and Code views use that metadata; agents read canonical source files when editing.
The screenshot tool still returns model-visible image content for visual verification.

## Slint file viewer

The file entrypoint opens `.slint` host resources in the Slint view.
It uses the path granted by the host to resolve project dependencies and validates the current buffer with the native LSP.
File-resource subscriptions reload the preview when the host reports a change.
Unsaved entry contents are rendered without overwriting the saved file; imported files are read from disk.
Hosts must support file entrypoints, path grants, and resource subscriptions for this workflow.

## Persistent side preview

`open_slint_preview` registers a thread entrypoint and opens a saved, validated source in the side view.
Its fullscreen display preference is a host hint; the view requests that mode once when supported and accepts the returned placement.
`update_slint_preview` updates the same view ID and returns no UI opener, avoiding repeated inline previews.
The shared skill moves work to the side on the third follow-up edit to the same item.
Initial creation does not count as an edit, and explicit user presentation preferences take precedence.
Source payloads remain in UI-only metadata and the existing screenshot workflow verifies each revision.
