# Model & Repeater System

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/core/model.rs`,
> `internal/core/model/adapters.rs`, repeater-related code generation,
> list views, or debugging data binding issues in `for` loops.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

The Model system provides data for repeated elements in Slint's `for` expressions. It's a reactive data source with change notifications that allow efficient UI updates when data changes.

**Key concepts:**
- **Model**: Trait providing data rows with change notifications
- **ModelRc**: Reference-counted wrapper for models (used in array properties)
- **Repeater**: Runtime component that instantiates item trees based on model data
- **Adapters**: Transforms like `map`, `filter`, `sort`, `reverse`

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/model.rs` | Model trait, VecModel, ModelRc |
| `internal/core/model/repeater.rs` | Repeater, RepeaterTracker, RepeaterInner, Conditional |
| `internal/core/model/adapters.rs` | MapModel, FilterModel, SortModel, ReverseModel |
| `internal/core/model/model_peer.rs` | Change notification system |

## Core Types

### The Model Trait

A `Model` has an associated `Data` type and three methods with no default. `row_count()` and
`row_data()` supply the data — the latter returns `None` past the end and does *not* register a
dependency, `ModelExt::row_data_tracked()` being the tracking equivalent — and `model_tracker()`
returns the model's `ModelNotify`, or `&()` for a model that never changes.

The mutating methods — `set_row_data()`, `push_row()`, `remove_row()`, `insert_row()` — all
default to printing a warning, so a read-only model can leave them out. An implementation that
does support them must call the matching `ModelNotify` method afterwards.

`as_any()` defaults to `&()` and should return `self` so a concrete model can be recovered from a
`ModelRc`. `iter()` is provided.
See `Model` in `internal/core/model.rs`.

### ModelTracker

The interface for dependency tracking: `attach_peer()` registers a peer for change
notifications, and `track_row_count_changes()` / `track_row_data_changes(row)` register the row
count or one row's data as a dependency of the binding currently being evaluated.
`track_any_change(row_count)` registers both; its default implementation loops over every row,
but `ModelNotify` overrides it with a single dependency whose cost does not grow with the model.
See `ModelTracker` in `internal/core/model.rs`.

### ModelNotify

The standard implementation of change notifications. A model owning one calls `row_changed()`,
`row_added(index, count)`, `row_removed(index, count)` or `reset()` after mutating its data.
See `ModelNotify` in `internal/core/model/model_peer.rs`.

### ModelRc

The standard wrapper for models in Slint's public API.
`ModelRc<T>` holds an `Option<Rc<dyn Model<Data = T>>>` — `None` is the empty model, so
`ModelRc::default()` allocates nothing. See `internal/core/model.rs`.

```rust
// Construction
ModelRc::default()                    // Empty model
ModelRc::new(vec_model)               // From any Model impl
ModelRc::from(&[1, 2, 3])            // From slice (creates VecModel)
ModelRc::from(rc_model)              // From Rc<Model>

// Array properties in Slint become ModelRc<T>
// property<[string]> items;  ->  ModelRc<SharedString>
```

## Change Notification Flow

```
┌──────────────┐    notify     ┌───────────────┐    callback    ┌──────────────┐
│   VecModel   │──────────────>│  ModelNotify  │───────────────>│   Repeater   │
│  .push(x)    │               │               │                │  (UI peer)   │
└──────────────┘               │  row_added()  │                │              │
                               │  row_changed()│                │  creates/    │
                               │  row_removed()│                │  updates     │
                               │  reset()      │                │  instances   │
                               └───────────────┘                └──────────────┘
                                      │
                                      │ also marks dirty
                                      ▼
                               ┌───────────────┐
                               │  Properties   │
                               │  (bindings)   │
                               └───────────────┘
```

### ModelChangeListener

Interface implemented by peers such as `RepeaterTracker`: the same four notifications
`ModelNotify` sends, each taking `self: Pin<&Self>`. A `ModelChangeListenerContainer<T>` wraps an
implementation and hands out the `ModelPeer` for it.
See `internal/core/model/model_peer.rs`.

## Built-in Model Implementations

### VecModel

The most common mutable model: a `RefCell<Vec<T>>` plus a `ModelNotify`. It offers `push()`,
`insert()`, `remove()`, `swap()`, `clear()`, `set_vec()`, `extend()` and `extend_from_slice()`,
each notifying as needed. `from_slice()` builds a `ModelRc` directly.
See `VecModel` in `internal/core/model.rs`.

### SharedVectorModel

A `SharedVector<T>` plus a `ModelNotify`. Its own API is much smaller than `VecModel`'s: `push()`
to append, and `shared_vector()` to get a clone of the backing vector out.
See `SharedVectorModel` in `internal/core/model.rs`.

### Primitive Models

- `usize` implements Model: produces rows 0..n with data = row index
- `bool` implements Model: produces 0 or 1 rows

## Model Adapters

Adapters wrap existing models to transform their data without copying.

### MapModel

Transform each row's data:

```rust
let model = VecModel::from(vec![1, 2, 3]);
let mapped = MapModel::new(model, |x| x * 2);  // [2, 4, 6]

// Or using extension trait:
let mapped = model.map(|x| x * 2);
```

**Key behavior:**
- Same row count as source
- Changes propagate through directly
- No internal state - transformation applied on each access

### FilterModel

Filter rows based on predicate:

```rust
let model = VecModel::from(vec![1, 2, 3, 4, 5]);
let filtered = FilterModel::new(model, |x| *x > 2);  // [3, 4, 5]

// Or using extension trait:
let filtered = model.filter(|x| *x > 2);
```

**Key behavior:**
- Maintains internal mapping (source index → filtered index)
- `row_changed` may cause row to appear/disappear from filtered view
- Call `reset()` to re-evaluate filter for all rows

### SortModel

Sort rows by comparison function:

```rust
let model = VecModel::from(vec![3, 1, 4, 1, 5]);
let sorted = SortModel::new(model, |a, b| a.cmp(b));  // [1, 1, 3, 4, 5]

// Or ascending sort (requires Ord):
let sorted = model.sort();

// Or using extension trait:
let sorted = model.sort_by(|a, b| a.cmp(b));
```

**Key behavior:**
- Maintains sorted index mapping
- Source changes trigger re-sort
- Call `reset()` to force full re-sort

### ReverseModel

Reverse row order:

```rust
let model = VecModel::from(vec![1, 2, 3]);
let reversed = ReverseModel::new(model);  // [3, 2, 1]

// Or using extension trait:
let reversed = model.reverse();
```

### Adapter Chaining

Adapters can be chained:

```rust
let result = VecModel::from(vec![5, 2, 8, 1, 9])
    .filter(|x| *x > 2)     // [5, 8, 9]
    .map(|x| x * 10)        // [50, 80, 90]
    .sort();                // [50, 80, 90]
```

## Repeater

The `Repeater<C>` manages instantiation of item trees based on model data. It (along with
`RepeaterTracker`, `RepeaterInner`, and `Conditional`) is defined in
`internal/core/model/repeater.rs`, not `model.rs`.

### Structure

- `Repeater<C>` is a newtype over a `ModelChangeListenerContainer<RepeaterTracker<C>>`.
- `RepeaterTracker<C>` is what actually listens: the instances, the model property, an `is_dirty`
  property set when the model becomes dirty, a separate `instance_generation` property marked
  dirty by `ensure_updated()` once instances have actually been added or removed (layout and
  visit code depend on that one, so they re-evaluate after the update pass rather than when the
  model first changes), and a `PropertyTracker` for the ListView geometry.
- `RepeaterInner<C>` holds the instance vector — each entry a `RepeatedInstanceState` and an
  optional item tree — plus the `RepeaterLayoutState`.
- `RepeaterLayoutState` is the persistent ListView layout state: the model row index of the first
  instance, the cached average item height, the content y from the previous pass (used to tell
  scroll direction), and the y position of the item at `offset`.

All in `internal/core/model/repeater.rs`.

### RepeatedItemTree Trait

`RepeatedItemTree` extends `ItemTree` with an associated `Data` type and `update(index, data)`,
called whenever the model data for that instance changes. `init()` runs once after instantiation
and the first `update()`. `listview_layout()` places the item and advances `offset_y` to the next
position, returning the minimum item width for the ListView's content width. `layout_item_info()`
returns what a surrounding layout needs, taking a child index for the repeated-`Row` case. All
but `update()` have default implementations.
See `RepeatedItemTree` in `internal/core/model/repeater.rs`.

### Update Flow

1. **Model changes** → `ModelChangeListener` callbacks called on `RepeaterTracker`
2. **RepeaterTracker** marks `is_dirty` and updates instance states
3. **During rendering** → `ensure_updated()` called
4. **Repeater** creates/updates/removes instances as needed

`Repeater::ensure_updated()` takes a closure that instantiates one item tree and brings all
instances up to date; `ensure_updated_listview()` does the same for the virtualized case, also
taking the ListView's content size and y properties and its own width and height. Both return
whether anything changed. `ensure_updated_listview_callback()` is the trait-object variant the
interpreter uses, for consumers that cannot hand out the content properties directly.
See `internal/core/model/repeater.rs`.

### ListView Virtualization

For `ListView`, only visible items are instantiated:

```
Model rows: [0] [1] [2] [3] [4] [5] [6] [7] [8] [9]
                     ↑                   ↑
                   offset         offset + len

Instances:          [2] [3] [4] [5] [6]
                   (only visible rows instantiated)
```

The `offset` tracks which model row corresponds to `instances[0]`.

## Conditional

For `if` expressions in Slint (0 or 1 instances): a `bool` property standing in for the model,
the single optional instance, and the same `instance_generation` property `RepeaterTracker` uses.
Unlike a repeater it keeps the existing instance while the condition stays true, so no spurious
re-init happens. See `Conditional` in `internal/core/model/repeater.rs`.

## Row Data Tracking

Two levels of dependency tracking:

### Row Count Tracking

```rust
// In binding, tracks when row count changes:
model.model_tracker().track_row_count_changes();
let count = model.row_count();  // Binding re-evaluates when count changes
```

### Row Data Tracking

```rust
// In binding, tracks when specific row changes:
model.model_tracker().track_row_data_changes(row);
let data = model.row_data(row);  // Binding re-evaluates when row changes

// Convenience method:
let data = model.row_data_tracked(row);  // Combines both calls
```

## Common Patterns

### Creating a Custom Model

```rust
pub struct MyModel {
    data: RefCell<Vec<MyData>>,
    notify: ModelNotify,
}

impl Model for MyModel {
    type Data = MyData;

    fn row_count(&self) -> usize {
        self.data.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.data.borrow().get(row).cloned()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        self.data.borrow_mut()[row] = data;
        self.notify.row_changed(row);  // Important!
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl MyModel {
    pub fn push(&self, value: MyData) {
        self.data.borrow_mut().push(value);
        self.notify.row_added(self.data.borrow().len() - 1, 1);
    }

    pub fn remove(&self, index: usize) {
        self.data.borrow_mut().remove(index);
        self.notify.row_removed(index, 1);
    }
}
```

### Modifying Model from UI Callback

```rust
// Keep Rc to model for later modification
let model: Rc<VecModel<SharedString>> = Rc::new(VecModel::default());
ui.set_items(model.clone().into());

ui.on_add_clicked({
    let model = model.clone();
    move || {
        model.push("New Item".into());
    }
});
```

### Downcasting to Modify

```rust
// Get model from property, downcast to concrete type
let items = ui.get_items();
if let Some(vec_model) = items.as_any().downcast_ref::<VecModel<SharedString>>() {
    vec_model.push("Added".into());
}
```

### Updating from Background Thread

```rust
let ui_weak = ui.as_weak();
std::thread::spawn(move || {
    let new_data = fetch_data();  // Background work

    // Must update UI on main thread
    ui_weak.upgrade_in_event_loop(move |ui| {
        let model = ui.get_items();
        let vec_model = model.as_any()
            .downcast_ref::<VecModel<String>>()
            .unwrap();
        vec_model.set_vec(new_data);
    });
});
```

## Debugging Tips

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| UI not updating | Missing `notify.row_changed()` | Call appropriate notify method after data change |
| Downcast fails | Type mismatch | Check actual model type (often wrapped in adapter) |
| Performance issues | Recreating model on every change | Modify existing model, don't replace |
| Index out of bounds | Stale row index after model change | Use model's notification to update indices |

### Inspecting Model State

```rust
// Check row count
println!("Rows: {}", model.row_count());

// Iterate all data
for data in model.iter() {
    println!("{:?}", data);
}

// Check if model is empty
if model.row_count() == 0 {
    println!("Empty model");
}
```

### Testing Models

```rust
#[test]
fn test_model_notifications() {
    let model = Rc::new(VecModel::from(vec![1, 2, 3]));
    let tracker = Box::pin(PropertyTracker::default());

    // Track row count changes
    tracker.as_ref().evaluate(|| {
        model.model_tracker().track_row_count_changes();
        model.row_count()
    });

    assert!(!tracker.is_dirty());
    model.push(4);
    assert!(tracker.is_dirty());  // Notified of change
}
```

## Performance Considerations

1. **Prefer modify over replace**: Calling `set_row_data()` is more efficient than replacing the entire model
2. **Use adapters lazily**: MapModel doesn't copy data - transformation happens on access
3. **ListView virtualization**: Only visible rows are instantiated
4. **Batch changes**: Multiple `push()` calls trigger multiple notifications; use `extend()` for bulk inserts
5. **Filter/Sort caching**: These adapters maintain index mappings; call `reset()` sparingly

## Testing

```sh
# Run model tests
cargo test -p i-slint-core model

# Run adapter tests
cargo test -p i-slint-core adapters

# Run with specific test
cargo test -p i-slint-core test_vecmodel_set_vec
```
