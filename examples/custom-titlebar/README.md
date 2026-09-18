# Custom Title Bar

A frameless window (`no-frame: true`) with rounded corners and a fully custom title bar,
all built from ordinary Slint elements: drag the bar to move the window (`WindowMoveArea`), drag the window
edges to resize it (`resize-border-width`), double-click the bar to maximize, and use the minimize / maximize /
close buttons.

The window background is transparent so the rounded corners show through, and the corners square off while the
window is maximized, like native decorations do.

![Screenshot of the custom title bar example](https://github.com/user-attachments/assets/eaaf24d4-892e-49b0-aef7-6271df05f7d4 "Custom Title Bar")

Run it natively to see the real window behavior:

```sh
cargo run -p slint-viewer -- examples/custom-titlebar/custom-titlebar.slint
```

Moving the window requires a backend and platform with support for it (winit on Windows, macOS, X11, and Wayland;
Qt), and `resize-border-width` is winit-only for now. In the browser preview the window-management features do
nothing.

[Online code editor](https://slint.dev/snapshots/master/editor/index.html?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/custom-titlebar/custom-titlebar.slint)
