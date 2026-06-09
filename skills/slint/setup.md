# Project Setup

## Preview a `.slint` without host code

`slint-viewer ui/main.slint` hot-reloads on save. See `tools-install.md` to
install it, and `reference/debugging-and-mcp.md` for headless rendering and
screenshots.

## Rust

```toml
# Cargo.toml
[dependencies]
slint = "1.x"

[build-dependencies]
slint-build = "1.x"
```

```rust
// build.rs
fn main() { slint_build::compile("ui/main.slint").unwrap(); }
```

```rust
// main.rs
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = MainWindow::new()?;
    // set up globals, models, callbacks…
    app.run()
}
```

To track unreleased features, depend on git (do the same for `slint-build`):

```toml
slint = { git = "https://github.com/slint-ui/slint", branch = "master" }
```

## C++

```cmake
find_package(Slint)            # or FetchContent
slint_target_sources(my_app ui/main.slint)
target_link_libraries(my_app PRIVATE Slint::Slint)
```

## Node.js

```js
import * as slint from "slint-ui";
const ui = slint.loadFile("ui/main.slint");
const app = new ui.MainWindow();
app.run();
```

## Python

```python
import slint
ui = slint.load_file("ui/main.slint")
app = ui.MainWindow()
app.run()
```

Note: the `slint` wheel's `requires-python` tracks recent CPython releases and
advances with new Slint versions. If `uv add` / `pip install` picks an older Slint
than expected, check the latest wheel's `requires-python` on PyPI and bump your
project's `requires-python` (and `.python-version` for uv) to match before pinning
a Slint version.

See `reference/interop.md` for how to push data and handle callbacks from the host
language once the project builds.
