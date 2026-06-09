# Connecting to Business Logic (Rust / C++ / JS / Python)

A per-concern `global` (mixing `in property` and `callback`) avoids threading
properties and callbacks through every component. In-tree examples typically
use one mixed global per concern; the two-global split below — state in,
actions out — is one organization that scales well, but a single `Globals`
holding both is equally idiomatic.

```slint
// globals.slint
export struct Row { id: int, name: string, tags: [int], selected: bool }
export global AppData {           // host pushes models/state in
    in property <[Row]> rows;
    in property <string> status;
}
export global Logic {            // UI calls these; host handles them
    callback row-clicked(int, bool, bool);   // index, ctrl, shift
    callback refresh();
}
```

Any component can read `AppData.rows` / call `Logic.row-clicked(...)` directly.

## Rust

```rust
slint::include_modules!();
use slint::{ModelRc, VecModel};

let ui = MainWindow::new()?;
ui.global::<AppData>().set_status("Ready".into());
ui.global::<AppData>().set_rows(ModelRc::new(VecModel::from(rows))); // Vec<Row>
let weak = ui.as_weak();
ui.global::<Logic>().on_row_clicked(move |i, ctrl, shift| { /* … */ });
ui.run()
```

Naming & type mapping (Rust):
- Kebab-case → snake_case: `is-folder` → `is_folder`; `row-clicked` → registrar
  `on_row_clicked`, setter `set_...`.
- `[T]` → `ModelRc<T>`; build with `ModelRc::new(VecModel::from(vec))`. For live
  updates keep a `Rc<VecModel<T>>` and mutate it (`push`/`set_row_data`) instead of
  replacing the whole model.
- `string` ↔ `SharedString`; `length`/`float` ↔ `f32`; `int` ↔ `i32`;
  `brush`/`color` ↔ `slint::Brush`/`Color`.
- Recommended split: keep the source of truth and logic in the host language,
  expose the current view as an already-sorted/filtered model, let `.slint` render
  and forward interactions. Replace prototype timers with real signals
  (`slint::Timer`; from other threads use `slint::invoke_from_event_loop` + a
  `Weak`).

## C++ / JS / Python

Same globals/structs via each language's idiomatic API (getters/setters + callback
registration); names follow each language's conventions (snake_case in Python,
camelCase in JS). The "logic in host, view-model into `.slint`" split applies
identically.
