# Model & Repeater System

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
| `internal/core/model.rs` | Model trait, VecModel, ModelRc, Repeater |
| `internal/core/model/adapters.rs` | MapModel, FilterModel, SortModel, ReverseModel |
| `internal/core/model/model_peer.rs` | Change notification system |

## Core Types

### The Model Trait

```rust
pub trait Model {
    type Data;

    /// Number of rows in the model
    fn row_count(&self) -> usize;

    /// Get data for a row (None if out of bounds)
    fn row_data(&self, row: usize) -> Option<Self::Data>;

    /// Set data for a row (optional, default prints warning)
    fn set_row_data(&self, row: usize, data: Self::Data) { ... }

    /// Return the tracker for change notifications
    fn model_tracker(&self) -> &dyn ModelTracker;

    /// For downcasting (typically return `self`)
    fn as_any(&self) -> &dyn core::any::Any { &() }
}
```

### ModelTracker

The interface for dependency tracking:

```rust
pub trait ModelTracker {
    /// Attach a peer to receive change notifications
    fn attach_peer(&self, peer: ModelPeer);

    /// Register dependency on row count changes
    fn track_row_count_changes(&self);

    /// Register dependency on a specific row's data
    fn track_row_data_changes(&self, row: usize);
}
```

### ModelNotify

The standard implementation of change notifications:

```rust
pub struct ModelNotify {
    inner: OnceCell<Pin<Box<ModelNotifyInner>>>,
}

impl ModelNotify {
    /// Notify that a row's data changed
    pub fn row_changed(&self, row: usize);

    /// Notify that rows were inserted
    pub fn row_added(&self, index: usize, count: usize);

    /// Notify that rows were removed
    pub fn row_removed(&self, index: usize, count: usize);

    /// Notify that the entire model was reset
    pub fn reset(&self);
}
```

### ModelRc

The standard wrapper for models in Slint's public API:

```rust
pub struct ModelRc<T>(Option<Rc<dyn Model<Data = T>>>);

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

Interface implemented by peers (like Repeater):

```rust
pub trait ModelChangeListener {
    fn row_changed(self: Pin<&Self>, row: usize);
    fn row_added(self: Pin<&Self>, index: usize, count: usize);
    fn row_removed(self: Pin<&Self>, index: usize, count: usize);
    fn reset(self: Pin<&Self>);
}
```

## Built-in Model Implementations

### VecModel

The most common mutable model:

```rust
pub struct VecModel<T> {
    array: RefCell<Vec<T>>,
    notify: ModelNotify,
}

impl<T> VecModel<T> {
    pub fn push(&self, value: T);
    pub fn insert(&self, index: usize, value: T);
    pub fn remove(&self, index: usize) -> T;
    pub fn set_vec(&self, new: impl Into<Vec<T>>);
    pub fn extend<I: IntoIterator<Item = T>>(&self, iter: I);
    pub fn clear(&self);
    pub fn swap(&self, a: usize, b: usize);
}
```

### SharedVectorModel

For shared/cloneable vectors:

```rust
pub struct SharedVectorModel<T> {
    array: RefCell<SharedVector<T>>,
    notify: ModelNotify,
}
```

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

The `Repeater<C>` manages instantiation of item trees based on model data.

### Structure

```rust
pub struct Repeater<C: RepeatedItemTree>(
    ModelChangeListenerContainer<RepeaterTracker<C>>
);

struct RepeaterTracker<T: RepeatedItemTree> {
    inner: RefCell<RepeaterInner<T>>,
    model: Property<ModelRc<T::Data>>,
    is_dirty: Property<bool>,
    listview_geometry_tracker: PropertyTracker,
}

struct RepeaterInner<C: RepeatedItemTree> {
    instances: Vec<(RepeatedInstanceState, Option<ItemTreeRc<C>>)>,
    offset: usize,              // For ListView virtualization
    cached_item_height: LogicalLength,
    // ...
}
```

### RepeatedItemTree Trait

Item trees that can be repeated implement:

```rust
pub trait RepeatedItemTree: ItemTree + HasStaticVTable<ItemTreeVTable> + 'static {
    type Data: 'static;

    /// Called when model data changes
    fn update(&self, index: usize, data: Self::Data);

    /// Called after first instantiation
    fn init(&self) {}

    /// For ListView layout
    fn listview_layout(self: Pin<&Self>, offset_y: &mut LogicalLength) -> LogicalLength;
}
```

### Update Flow

1. **Model changes** → `ModelChangeListener` callbacks called on `RepeaterTracker`
2. **RepeaterTracker** marks `is_dirty` and updates instance states
3. **During rendering** → `ensure_updated()` called
4. **Repeater** creates/updates/removes instances as needed

```rust
impl<C: RepeatedItemTree> Repeater<C> {
    /// Ensure all instances are up-to-date
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ItemTreeRc<C>);

    /// For ListView with virtualization
    pub fn ensure_updated_listview(
        self: Pin<&Self>,
        init: impl Fn() -> ItemTreeRc<C>,
        viewport_width: Pin<&Property<LogicalLength>>,
        viewport_height: Pin<&Property<LogicalLength>>,
        viewport_y: Pin<&Property<LogicalLength>>,
        listview_width: LogicalLength,
        listview_height: Pin<&Property<LogicalLength>>,
    );
}
```

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

For `if` expressions in Slint (0 or 1 instances):

```rust
pub struct Conditional<C: RepeatedItemTree> {
    model: Property<bool>,
    instance: RefCell<Option<ItemTreeRc<C>>>,
}
```

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
