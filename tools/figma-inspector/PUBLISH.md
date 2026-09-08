# Publishing Figma to Slint

## Community release

1. Check out the Slint release revision you intend to publish.
2. Install dependencies and build tools as described in [README.md](README.md).
   Run `pnpm verify`, then `pnpm zip` from this directory.
3. Extract the release ZIP from `zip/` and import its manifest in Figma Desktop.
   Check preview, selection changes and clearing, pinning, diagnostics, copying,
   and project export. Check native codegen with Use Variables on and off in
   Figma Desktop and the Figma VS Code extension.
4. Check the archive's notices, dependency inventory, provenance and checksums.
5. Sign in with the Slint publisher account and select the SixtyFPS GmbH team.
   Open Plugins > Manage Plugins > Figma to Slint > Publish for the verified build.
   Confirm plugin ID `1474418299182276871`, publisher SixtyFPS GmbH, and support
   contact info@slint.dev.

## Nightly builds

```sh
pnpm build:slint
pnpm zip:nightly
```

This creates `zip/figma-plugin.zip` using the interpreter built from this checkout. Nightly archives record their channel and
runtime revision; they must not be submitted as Community releases.
