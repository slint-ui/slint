# The Slint Editor Preview

Internal crate shared between the Slint language server and the visual editor.
It holds the document model of a Slint project as it is being edited, the
editing session that keeps the preview in sync with it, and the preview engine
with its user interface.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

## Visual Editor Project Run Target

The Visual Editor reads `slint.toml` from the project root when you select Run.
Set `entry` to a project-relative `.slint` file and `component` to an exported component in that file:

```toml
entry = "ui/app-window.slint"
component = "AppWindow"
```

When the file is missing, Run asks for the entry file.
If that file exports multiple components, choose the app component from the dialog.
The editor then creates `slint.toml` and launches the companion viewer.
