# Slint Codex plugin

Develop Slint source with Codex, validate it with the native LSP, and show Slint Wasm previews inline in compatible hosts.
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

The build uses the existing native LSP and Wasm interpreter crates.
The preview template and JavaScript modules are bundled with esbuild into `runtime/preview.html`.
Syntax highlighting uses the Figma inspector themes and the shared Slint grammar.
Run `pnpm --filter slint.codex.plugin build` to rebuild the preview without rebuilding Slint.
The repository logo is symlinked; the build resolves it into `runtime/slint.svg` for installation and ZIP packaging.
Generated binaries and their source revision are stored in the ignored `runtime/` directory.
The preview reads generated JavaScript and compressed Wasm through the host's MCP resource bridge.
The HTML stays below 1 MiB, and each Wasm resource holds at most 256 KiB of compressed bytes.
The preview verifies the reconstructed Wasm hash before initializing it.
A browser cache retains one runtime, including its JavaScript and verified Wasm bytes.
Changing the runtime replaces that entry; unavailable browser storage falls back to MCP resource reads.
Loading phases are recorded in console logs and the preview acknowledgement, without additional UI.
Run `pnpm --filter slint.codex.plugin test` after building to check resource size and restart behavior.
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

Call `render_slint` with an absolute `path`, the next `revision`, and `validatedSourceHash` returned by `validate_slint`.
The tool reads saved source and rejects a mismatched validation hash.
Supply `projectRoot` when relative imports or assets need a broader root than the entry file's directory.
The preview reports source identity and ready/error acknowledgements through model context.
These acknowledge the renderer, not independent visual inspection of the host window.

Relative Slint imports and re-exports, PNG/JPEG/SVG/WebP images, and TTF/OTF fonts are supported.
Empty image URLs remain empty images and do not create file dependencies.
Dependencies stay inside the declared root, including symlink targets.
Network dependencies, include-path aliases, and absolute dependency references are unsupported.
Snapshots are immutable, private to the local user, and retain the latest 32 submissions in the system temporary directory.
Each snapshot allows 128 files, 8 MiB per file, and 16 MiB total; each Slint file allows 64 KiB.
An expired snapshot needs a fresh render submission.

## Publishing and team packages

Run `node ai-plugins/codex/scripts/package-plugin.mjs /absolute/path/Slint.zip` after building.
The archive includes the official assistant manifests, shared skill, matching native LSP, Wasm, source revision, and marketplace catalog.
The generated `slint-ui/ai-plugins` repository currently mirrors source from `release/1` without a build step.
A source-only install keeps the shared skill and docs connection but exposes no preview tools.
Public runtime artifact delivery requires a separate publishing change; native binaries remain excluded from the monorepo.
It contains only an explicit package file list, without project snapshots, credentials, build caches, or test files.
The generated ZIP has deterministic file ordering, timestamps, and executable permissions.
The included native runtime works on the recorded platform and architecture; other platforms build from this checkout.

The preview surface is transparent and follows the host's colour scheme.
Use the hamburger menu to switch between Preview and Code.
Keyboard zoom remains available; edits happen through the saved Slint source in chat.
Third-party notices are in `THIRD_PARTY_NOTICES.txt`.
