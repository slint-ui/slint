# Semantic Analysis Performance Optimization Analysis

This document analyzes allocation hotspots in the Slint compiler's semantic analysis phase and proposes optimization strategies.

## Executive Summary

The semantic analysis phase shows Vec-related allocation overhead in three main areas:
1. **Element duplication during inlining** - O(n) clones per element, recursive
2. **Exports sorting** - O(n²) insertion sort pattern
3. **Incremental Vec growth** - No pre-allocation for children, states, transitions

---

## 1. Critical Hotspots

### 1.1 Element Duplication in Inlining Pass

**Location:** `internal/compiler/passes/inlining.rs:349-405`

This is the most allocation-heavy operation. Each element duplication clones ~15 fields:

```rust
fn duplicate_element_with_mapping(element: &ElementRc, ...) -> ElementRc {
    let elem = element.borrow();
    let new = Rc::new(RefCell::new(Element {
        base_type: elem.base_type.clone(),           // Clone
        id: elem.id.clone(),                          // Clone (SmolStr - cheap)
        property_declarations: elem.property_declarations.clone(),  // BTreeMap clone!
        bindings: elem.bindings.iter()
            .map(|b| duplicate_binding(...))
            .collect(),                               // New BTreeMap + N clones
        change_callbacks: elem.change_callbacks.clone(),  // BTreeMap<K, RefCell<Vec>> clone!
        property_analysis: elem.property_analysis.clone(),
        children: elem.children.iter()
            .map(|x| duplicate_element_with_mapping(...))  // RECURSIVE
            .collect(),                               // New Vec
        states: elem.states.clone(),                  // Vec clone
        transitions: elem.transitions.iter()
            .map(|t| duplicate_transition(...))
            .collect(),                               // New Vec + N clones
        debug: elem.debug.clone(),                    // Vec clone
        // ... more fields
    }));
}
```

**Impact:** For a component with 100 elements, each with 5 properties and 2 children on average:
- ~1,500 BTreeMap/Vec clones
- ~500 binding duplications
- All recursive, so memory pressure compounds

**Optimization Options:**

| Option | Description | Effort | Impact |
|--------|-------------|--------|--------|
| **Arena allocation** | Use `bumpalo` or typed arena for elements during compilation | High | High |
| **Copy-on-write** | Use `Cow<>` or `Arc` for fields that rarely change | Medium | Medium |
| **Lazy cloning** | Clone only when mutated (track "dirty" flag) | Medium | High |
| **Pre-sized Vecs** | Use `Vec::with_capacity()` based on source element size | Low | Low |

### 1.2 O(n²) Export Sorting

**Location:** `internal/compiler/object_tree.rs:2950-2959`

```rust
let mut sorted_exports_with_duplicates: Vec<(ExportedName, _)> = Vec::new();

let mut extend_exports = |it: &mut dyn Iterator<...>| {
    for (name, compo_or_type) in it {
        let pos = sorted_exports_with_duplicates
            .partition_point(|(existing_name, _)| existing_name.name <= name.name);  // O(log n)
        sorted_exports_with_duplicates.insert(pos, (name, compo_or_type));  // O(n)!
    }
};

extend_exports(&mut ...);  // Called 3 times
```

**Impact:** For 50 exports, this is ~2,500 element shifts (50 × 50 / 2).

**Optimization Options:**

| Option | Description | Effort | Impact |
|--------|-------------|--------|--------|
| **Collect then sort** | Collect all into Vec, then call `.sort_by()` once | Low | High |
| **BTreeMap intermediate** | Use BTreeMap for sorted insertion, convert to Vec | Low | Medium |

**Suggested fix:**
```rust
let mut exports: Vec<(ExportedName, _)> = doc.ExportsList()
    .filter(...)
    .flat_map(...)
    .chain(doc.ExportsList().flat_map(...))  // Combine all iterators
    .chain(doc.ExportsList().flat_map(...))
    .collect();

exports.sort_by(|(a, _), (b, _)| a.name.cmp(&b.name));
exports.dedup_by(|(a, _), (b, _)| a.name == b.name);
```

### 1.3 PropertyPath Cloning in Binding Analysis

**Location:** `internal/compiler/passes/binding_analysis.rs:105`

```rust
fn relative(&self, second: &PropertyPath) -> Self {
    // ...
    let mut elements = self.elements.clone();  // Full Vec clone
    loop {
        if let Some(last) = elements.pop() {
            // May only pop a few elements
        }
    }
    // ...
}
```

**Impact:** Called frequently during binding dependency analysis. Clones full path even when only truncating.

**Optimization Options:**

| Option | Description | Effort | Impact |
|--------|-------------|--------|--------|
| **Slice view** | Return slice indices instead of cloned Vec | Medium | High |
| **Truncate in place** | Pass `&mut self` and truncate, or use `Cow` | Low | Medium |
| **SmallVec** | Use `SmallVec<[_; 4]>` for typical short paths | Low | Medium |

---

## 2. Data Structure Analysis

### 2.1 Element Struct

**Location:** `internal/compiler/object_tree.rs:813-882`

```rust
pub struct Element {
    pub id: SmolStr,                                    // 24 bytes, inline small strings
    pub base_type: ElementType,                         // enum, variable size
    pub bindings: BTreeMap<SmolStr, RefCell<BindingExpression>>,  // Heap
    pub change_callbacks: BTreeMap<SmolStr, RefCell<Vec<Expression>>>,  // Double heap!
    pub property_analysis: RefCell<HashMap<SmolStr, PropertyAnalysis>>,
    pub children: Vec<ElementRc>,                       // Heap, grows incrementally
    pub property_declarations: BTreeMap<SmolStr, PropertyDeclaration>,
    pub states: Vec<State>,                             // Heap
    pub transitions: Vec<Transition>,                   // Heap
    pub debug: Vec<ElementDebugInfo>,                   // Heap
    // ... 15 more fields
}
```

**Issues:**
1. `change_callbacks: BTreeMap<K, RefCell<Vec<V>>>` - Triple indirection (map → RefCell → Vec)
2. `children` grows via `.push()` without capacity hints
3. Many small Vecs allocated separately

**Optimization Options:**

| Field | Current | Proposed | Rationale |
|-------|---------|----------|-----------|
| `children` | `Vec<ElementRc>` | `SmallVec<[ElementRc; 4]>` | Most elements have ≤4 children |
| `states` | `Vec<State>` | `SmallVec<[State; 2]>` | Most elements have 0-2 states |
| `transitions` | `Vec<Transition>` | `SmallVec<[Transition; 2]>` | Most have 0-2 transitions |
| `debug` | `Vec<ElementDebugInfo>` | `SmallVec<[ElementDebugInfo; 1]>` | Usually exactly 1 |
| `change_callbacks` | `BTreeMap<K, RefCell<Vec<V>>>` | `BTreeMap<K, SmallVec<[V; 1]>>` | Usually 1 callback per property |

### 2.2 Expression Enum

**Location:** `internal/compiler/expression_tree.rs:600-760`

Heavy use of `Box<Expression>` for recursive variants:

```rust
pub enum Expression {
    BinaryExpression { lhs: Box<Expression>, rhs: Box<Expression>, op: char },
    UnaryOp { sub: Box<Expression>, op: char },
    Condition { condition: Box<Expression>, true_expr: Box<Expression>, false_expr: Box<Expression> },
    StructFieldAccess { base: Box<Expression>, name: SmolStr },
    // ... 13 more Box-using variants
}
```

**Impact:** Deep expression trees like `a + b + c + d + e` create 8+ heap allocations.

**Optimization Options:**

| Option | Description | Effort | Impact |
|--------|-------------|--------|--------|
| **Arena allocation** | Allocate expressions from arena, store indices | High | High |
| **Flattened representation** | Store as bytecode-like Vec, index-based | High | High |
| **Inline small expressions** | Use `enum { Inline(SmallExpr), Boxed(Box<Expression>) }` | Medium | Medium |

### 2.3 Component Struct

**Location:** `internal/compiler/object_tree.rs:452-493`

```rust
pub struct Component {
    pub optimized_elements: RefCell<Vec<ElementRc>>,
    pub popup_windows: RefCell<Vec<PopupWindow>>,
    pub timers: RefCell<Vec<Timer>>,
    pub menu_item_tree: RefCell<Vec<Rc<Component>>>,
    pub exported_global_names: RefCell<Vec<ExportedName>>,
    pub private_properties: RefCell<Vec<(SmolStr, Type)>>,
    pub init_code: RefCell<InitCode>,  // Contains 3 more Vecs
}
```

**Issue:** 7+ `RefCell<Vec<T>>` fields, each requiring:
1. RefCell borrow checking overhead
2. Separate heap allocation per Vec
3. No capacity hints

**Optimization Options:**

| Option | Description | Effort | Impact |
|--------|-------------|--------|--------|
| **Consolidate Vecs** | Use single Vec with tagged enum for different types | Medium | Medium |
| **SmallVec** | Use `SmallVec<[T; N]>` for typically-small collections | Low | Medium |
| **Remove RefCell** | Use indices + arena for mutation without RefCell | High | High |

---

## 3. Incremental Vec Growth Patterns

### 3.1 Children Population

**Location:** `internal/compiler/object_tree.rs:1693-1734`

```rust
// Inside a loop iterating node.children()
r.borrow_mut().children.push(Element::from_sub_element_node(...));
// ...
r.borrow_mut().children.push(rep);  // For repeated elements
// ...
r.borrow_mut().children.push(rep);  // For conditional elements
```

**Issue:** Vec grows 1 element at a time. Rust's Vec doubles capacity, but still causes multiple reallocations.

**Optimization:**
```rust
// Before the loop:
let child_count = node.children().filter(|n| /* is element */).count();
r.borrow_mut().children.reserve(child_count);
```

### 3.2 States and Transitions

**Location:** `internal/compiler/object_tree.rs:1762-1797`

```rust
// Inside loops
r.borrow_mut().transitions.push(t);
r.borrow_mut().states.push(s);
```

**Optimization:**
```rust
let state_count = node.State().count();
let transition_count = node.Transition().count() +
    node.State().flat_map(|s| s.Transition()).count();

let mut elem = r.borrow_mut();
elem.states.reserve(state_count);
elem.transitions.reserve(transition_count);
```

---

## 4. Recommended Optimization Priority

### High Priority (Low effort, High impact)

1. **Fix O(n²) export sorting** - Replace insertion sort with collect-then-sort
   - File: `object_tree.rs:2950-2959`
   - Estimated complexity: ~20 lines changed

2. **Pre-allocate children Vec** - Count children before populating
   - File: `object_tree.rs:1693`
   - Estimated complexity: ~5 lines added

3. **SmallVec for Element fields** - Replace `Vec` with `SmallVec` for:
   - `children: SmallVec<[ElementRc; 4]>`
   - `states: SmallVec<[State; 2]>`
   - `transitions: SmallVec<[Transition; 2]>`
   - `debug: SmallVec<[ElementDebugInfo; 1]>`
   - Estimated complexity: Type changes + Cargo.toml dependency

### Medium Priority (Medium effort, High impact)

4. **PropertyPath slice optimization** - Avoid cloning in `relative()`
   - File: `binding_analysis.rs:105`
   - Estimated complexity: ~30 lines refactored

5. **SmallVec for PropertyPath::elements**
   - Typical paths are short (1-4 elements)

6. **change_callbacks simplification** - Remove RefCell from values
   - `BTreeMap<SmolStr, SmallVec<[Expression; 1]>>`

### Lower Priority (High effort, High impact)

7. **Arena allocation for Elements** - Use `typed-arena` or `bumpalo`
   - Eliminates per-element Rc overhead
   - Requires significant refactoring

8. **Lazy element cloning in inlining** - Copy-on-write semantics
   - Only clone when mutating

9. **Expression arena** - Allocate all expressions from arena
   - Eliminates Box overhead for expression trees

---

## 5. Benchmarks

Benchmarks for the semantic analysis phase are located in `internal/compiler/benches/semantic_analysis.rs`.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p i-slint-compiler

# Run specific category
cargo bench -p i-slint-compiler -- full_compilation

# Run specific benchmark
cargo bench -p i-slint-compiler -- "full_compilation::nested_components"

# Quick test (verify benchmarks work without timing)
cargo bench -p i-slint-compiler -- --test
```

### Benchmark Categories

| Category | Benchmarks | What it Measures |
|----------|------------|------------------|
| `lexing` | simple, many_children, many_properties | Token allocation in lexer |
| `parsing` | simple, many_children, many_properties | Syntax tree creation |
| `full_compilation` | 8 scenarios (see below) | End-to-end compilation |
| `expression_complexity` | binary chains, struct field access | Expression tree allocations |

### Stress Test Scenarios

Each scenario is designed to stress a specific allocation pattern:

| Benchmark | Parameters | Hotspot Targeted |
|-----------|------------|------------------|
| `many_children` | 10, 50, 100, 200 | `children: Vec<ElementRc>` growth |
| `many_properties` | 10, 50, 100 | `property_declarations` BTreeMap |
| `many_exports` | 5, 20, 60 | O(n²) export sorting (realistic: app, std-widgets, material lib) |
| `many_states` | 5, 10, 20 | `states`/`transitions` Vec allocation |
| `nested_components` | 5, 10, 15 | Inlining pass element duplication |
| `deep_expressions` | 5, 10, 20 | `Box<Expression>` chain allocation |
| `binding_chain` | 10, 50, 100 | Binding analysis dependency tracking |
| `struct_field_access_chain` | 3, 5, 8 | Nested `StructFieldAccess` expressions |

### Baseline Results

Representative results on a typical development machine (times will vary):

```
full_compilation::simple_component      ~10ms   (baseline)
full_compilation::nested_components/5   ~10ms
full_compilation::nested_components/10  ~11ms
full_compilation::nested_components/15  ~12ms   (+20% vs baseline)
full_compilation::many_children/100     ~10ms
full_compilation::many_exports/60       ~10ms   (largest real-world case)
```

### Measuring Allocations

For detailed allocation profiling, use `dhat` or a custom allocator:

```rust
// Add to benchmark or test
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();
```

Or use external tools:
```bash
# macOS: Instruments with Allocations template
# Linux: heaptrack or valgrind --tool=massif
# Cross-platform: cargo-instruments, samply
```

---

## 6. SmallVec Sizing Recommendations

Based on typical Slint component structure:

| Collection | Typical Size | Recommended SmallVec |
|------------|--------------|---------------------|
| Element.children | 2-4 | `SmallVec<[_; 4]>` |
| Element.states | 0-2 | `SmallVec<[_; 2]>` |
| Element.transitions | 0-2 | `SmallVec<[_; 2]>` |
| Element.debug | 1 | `SmallVec<[_; 1]>` |
| PropertyPath.elements | 1-3 | `SmallVec<[_; 4]>` |
| InitCode.constructor_code | 0-5 | `SmallVec<[_; 4]>` |
| Component.popup_windows | 0-1 | `SmallVec<[_; 1]>` |
| Component.timers | 0-2 | `SmallVec<[_; 2]>` |

Note: SmallVec inline storage should be sized to fit common cases without exceeding reasonable stack usage (~64-128 bytes per SmallVec).
