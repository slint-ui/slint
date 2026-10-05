# Slint Codex plugin

Develop Slint source with Codex, validate it with the native LSP, and show Slint Wasm previews inline in compatible hosts.
The plugin source lives in this monorepo.
The validator and preview interpreter are built from the same checkout, without a release version pin.

## Build and install

Use Node.js 20 or newer, Python 3, Rust, and `wasm-pack`.
Install the Wasm target with `rustup target add wasm32-unknown-unknown`.
From the monorepo root, run:

```sh
node tools/codex-plugin/scripts/build-runtime.mjs
codex plugin marketplace add .
codex plugin add slint@slint-prototype
```

Restart Codex and select Slint from the Slint Prototype marketplace in a new chat.
The CLI supports the development skill and tools but does not display an inline preview.

The build uses the existing native LSP and Wasm interpreter crates.
Generated binaries and their source revision are stored in the ignored `runtime/` directory.
The server embeds the generated Wasm and JavaScript in the preview resource, without fetching a released interpreter.
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

The preview resolves the bundled `slint-button.slint` import.
Use the project's existing tooling for previews that need other project imports.
The preview UI includes the existing editing, code display, and zoom features.
Third-party notices are in `THIRD_PARTY_NOTICES.txt`.
