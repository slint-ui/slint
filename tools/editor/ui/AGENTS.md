# Visual Editor Agent Notes

## Runtime

Use the real `slint-editor` with winit and Skia to exercise the embedded editor/LSP plumbing.
Do not substitute `slint-viewer`, a headless run, or the software renderer.
From the repository root:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
SLINT_BACKEND=winit-skia \
cargo run -p slint-editor -- examples/7guis/counter.slint
```

When launching, use `cargo run`; a preceding `cargo build` is unnecessary.
Confirm that the editor window is visible; a running process alone is insufficient.
Use MCP only when the user requests it.
Do not modify core or backend code to enable MCP for editor UI work.

## macOS Development App

For GUI control, reuse one development `.app` at a stable path with a stable `CFBundleIdentifier`.
Update its executable from the current build instead of creating a new app identity for each PR or verification run.
Keep this development app separate from the installed release app.
Stable identity avoids presenting each build as a different app to computer-control permission systems.

## Reloading

The editor reloads changes to the opened document automatically.
The editor shell under `tools/editor/ui/` is compiled into the executable and needs rebuilding unless Slint live preview is enabled.

## Pointer And Keyboard Handling

- Compare pointer positions in parent or window coordinates for move and resize interactions.
  For rotated resize, transform the handle-local pointer into that space, then convert its delta into item axes using the press-time rotation.
- Focus the editor `FocusScope` when an interaction depends on keyboard state.
  Track both `Key.Shift` and `Key.ShiftR` in `capture-key-pressed` and `capture-key-released` for live Shift state.
