# Connecting to Business Logic (Rust / C++ / JS / Python)

Use `export global`s for host interop instead of threading properties and
callbacks through every component — e.g. one global for state the host pushes
in and one for actions the host handles (a single mixed global is equally
idiomatic).

```slint
// globals.slint
export struct Row { id: int, name: string, selected: bool }
export global AppData {           // host pushes models/state in
    in property <[Row]> rows;
}
export global Logic {             // UI calls these; host handles them
    callback row-clicked(int);
    callback refresh();
}
```

Any component can read `AppData.rows` / call `Logic.row-clicked(...)` directly.

## Rust

```rust
slint::include_modules!();
use slint::{ModelRc, VecModel};

let ui = MainWindow::new()?;
ui.global::<AppData>().set_rows(ModelRc::new(VecModel::from(rows))); // Vec<Row>
ui.global::<Logic>().on_row_clicked(move |i| { /* … */ });
ui.run()
```

- Kebab-case → snake_case: `row-clicked` → registrar `on_row_clicked`,
  setter `set_...`.
- `[T]` → `ModelRc<T>`. For live updates keep an `Rc<VecModel<T>>` and mutate
  it (`push`/`set_row_data`) instead of replacing the whole model.
- `string` ↔ `SharedString`; `length`/`float` ↔ `f32`; `int` ↔ `i32`;
  `brush`/`color` ↔ `slint::Brush`/`Color`.
- Keep logic and the source of truth in the host language; expose an already
  sorted/filtered model and let `.slint` render and forward interactions. Use
  `slint::Timer` for timers; from other threads,
  `slint::invoke_from_event_loop` + a `Weak`.

## C++ / JS / Python

Same globals/structs via each language's API (getters/setters + callback
registration); names follow the language's convention (snake_case in Python,
camelCase in JS). The same host-logic/view split applies.
