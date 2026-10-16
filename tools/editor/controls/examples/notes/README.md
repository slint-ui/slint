<!-- cspell:words Fieldnotes -->

# Fieldnotes

Fieldnotes is a small desktop note app built with the existing headless and basic controls.
It supports notebooks, search, pinned notes, archiving, plain text export, and reading preferences.
See [the experiment report](REPORT.md) for control usage, validation, and improvement suggestions.

![Fieldnotes](preview.png)

## Run

From the repository root:

```sh
cargo run --manifest-path tools/editor/controls/examples/notes/Cargo.toml
```

After building on Windows, run `.\target\debug\fieldnotes.exe` directly.

The build enables experimental Slint interfaces and slots used by the controls.
The package has its own Cargo workspace.
The Slint viewer provides a layout preview; persistence and note actions require the Rust host.

## Field Styling

Search uses the basic LineEdit through `AppLineEdit`, with a leading icon and trailing clear action.
Density selects `LineEditSizes.small`, `medium`, or `large` alongside the other control presets.
The title uses custom `LineEditStyle` and `LineEditMetrics` values from `NotesTheme` to retain its heading appearance.
`AppLineEdit` explicitly forwards the basic control's optional slots.

## Data

Edits save after a 600 ms pause.
Use Save to write immediately.
Closing the window saves pending changes; a save failure keeps the window open and displays the error.
Delete requires confirmation in the app.

The default data file is:

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%/Fieldnotes/notes.json` |
| macOS | `$HOME/Library/Application Support/Fieldnotes/notes.json` |
| Linux | `$XDG_DATA_HOME/Fieldnotes/notes.json`, or `$HOME/.local/share/Fieldnotes/notes.json` |

Set `FIELDNOTES_DATA` or pass `--data-file PATH` to use another JSON file.
The first run creates sample notes.
An unreadable or malformed data file opens the app read-only and remains untouched.
Repair the file or choose another path before restarting.
The app writes a temporary file before replacing the data file.
Use one app instance per data file; concurrent edits aren't merged.
Reading preferences apply to the current session.

## Export

Export writes the selected note as plain text and displays its full path.
Files go into an `exports` directory beside the data file.
Pass `--export-directory PATH` to select another directory:

```sh
cargo run --manifest-path tools/editor/controls/examples/notes/Cargo.toml -- --export-directory ./exported-notes
```

Export creates a new file each time and doesn't overwrite existing exports.
