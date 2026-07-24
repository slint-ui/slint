# Encapsulating `Element::bindings`

## Context

PR #12196 added *debug hooks* to the compiler.
A debug hook is an `Expression::DebugHook { expression, id, synthetic }` wrapper.
The `inject_debug_hooks` pass wraps every real binding expression in a hook, and *synthesizes* a hook (`synthetic: true`) for unbound properties so the interpreter can override them at runtime without recompiling.
The synthesis is narrower than it first appears — see [Which properties get a synthetic hook](#which-properties-get-a-synthetic-hook).

The side effect: `Element::bindings` (a `BTreeMap<SmolStr, RefCell<BindingExpression>>`, aliased `BindingsMap`) now holds entries for properties that are semantically *unbound*.
Any property that receives a synthetic hook has a map entry even though the user never wrote it.

`bindings` is currently a `pub` field.
For such a property, a pass reading the raw map with `contains_key(p)` / `get(p)` / `iter()` can no longer tell "the user bound this" from "a synthetic hook sits here".
This document proposes the accessor API that closes the field off and makes the synthetic/real distinction explicit at every call site.

## Which properties get a synthetic hook

`inject_debug_hooks::property_defaults` synthesizes a hook for a property only when all hold:
- it appears in `elem.base_type.property_list()` or the element's own `property_declarations`;
- its visibility is `Public`, `InOut`, `Input`, or `Private` (not `Constexpr`/`Output`/`Protected`/`Fake`); and
- `Expression::default_value_for_type` for its type is not `Invalid` (callbacks and functions are excluded).

On top of that, `add_hooks_for_non_existent_bindings` adds two hardcoded sets: the geometry properties `x`/`y`/`width`/`height` (excluded on the component root and future popup roots; `z` is excluded everywhere), and the transform properties `transform-rotation`/`transform-scale-x`/`transform-scale-y`.

Nothing else is synthesized. In particular the `RESERVED_*` groups — `RESERVED_OTHER_PROPERTIES` (`clip`/`visible`/`opacity`/`cache-rendering-hint`), the layout properties (`col`/`row`/`colspan`/`rowspan`/`flex-*`/`padding-*`), and the accessibility properties (`accessible-*`) — live only in `typeregister::reserved_properties()`, consulted by name resolution in `lookup.rs`, never by `property_defaults`. This narrowness is why most raw-`bindings` reads flagged below are not actually fooled today; see the appendix.

## Goal

Make `bindings` private to the `object_tree` module and route every other module through accessors.
Because all passes live in the same crate, `pub(crate)` would not force them through the API — only module privacy does.
Same-module code (the `Element::from_node` constructor, the `Debug` impl, `deep_clone`) keeps direct field access; every pass, generator, and LLR module goes through the methods below.

## Naming convention

An accessor that hides synthetic hooks needs no marker — that is the default, hook-aware behavior a caller should reach for.
An accessor that *exposes* synthetic hooks must say so in its name, with the word `synthetic`.
There is no implicit "this one returns everything" accessor: `real_bindings()` and `bindings_including_synthetic()` sit side by side, and the difference is legible without reading the implementation.

This convention applies to every accessor that returns or counts synthetic entries — iterators, single-entry lookups, presence checks, and bulk take/extend.

## Access categories

Every call site falls into one of these shapes.
A1–A5 are already served by the existing API; migrating to them is hardening — an audit (see the appendix) found the current sites are defended, not actively broken, but routing them through the API guards against future regressions.
B6–B8 are the gaps this proposal fills.
C is left to a narrow escape hatch.

| # | Category | Shape | Served by |
|---|----------|-------|-----------|
| A1 | Is this property really bound? | `contains_key` / `get().is_some()` as a bool | `is_binding_set` / `binding(n).is_some()` |
| A2 | Read a real binding's expression or metadata | `get().borrow()`, then read or match the expression | `binding(n)` (+ `value_expression()` before matching) |
| A3 | Mutate a real binding in place | `get().borrow_mut()` | `binding_mut(n)` (+ `ignore_debug_hooks_mut()`) |
| A4 | Iterate the properties actually set | `iter` / `values` / `keys` | `real_bindings()` |
| A5 | Set or insert a binding | `insert` on a fresh or existing property | `set_binding` / `set_binding_if_not_set` |
| B6 | Take or remove a binding by ownership | `remove()` | `take_binding` (new) |
| B7 | Codegen / lowering that must see synthetic hooks too | iterate / get without filtering | `bindings_including_synthetic` / `binding_cell_including_synthetic` (new) |
| B8 | Replace a property's value while keeping its priority, animation and two-way binding; reentrant borrow | value swap + synthetic-hook upgrade; `try_borrow` | `binding_cell_including_synthetic` + `BindingExpression::set_value_expression` |
| C | Bulk / whole-map transfer and construction | `mem::take`, `extend`, struct literal, `clone`, cross-element `Entry` merge | escape hatch + same-module access |

## Existing API (recap)

These stay, and cover A1–A5.
All treat a synthetic hook as "no binding".

- `is_binding_set(name, need_explicit) -> bool`
- `is_property_set(name) -> bool`
- `binding(name) -> Option<Ref<BindingExpression>>`
- `binding_mut(name) -> Option<RefMut<BindingExpression>>`
- `real_bindings() -> impl Iterator<Item = (&SmolStr, &RefCell<BindingExpression>)>`
- `set_binding(name, BindingExpression) -> Option<BindingExpression>`
- `set_binding_if_not_set(name, impl FnOnce() -> Expression) -> bool`

## New API

### Hook-aware

```rust
/// Remove and return the binding for `name`.
///
/// Removes the map entry whether it held a real binding or only a synthetic hook,
/// but returns `Some` only for a real binding — a synthetic-only slot reads as
/// `None`, matching "nothing was ever bound here".
///
/// Use when a property is being consumed, renamed, or deleted and the caller
/// wants to branch on whether it was really set (B6).
pub fn take_binding(&mut self, name: &str) -> Option<BindingExpression>;
```

### Synthetic-inclusive

Codegen and lowering must emit or lower synthetic hooks so unbound properties stay live-editable.
These accessors deliberately expose them, and say so.

```rust
/// Iterate every binding entry, including synthetic hooks.
///
/// The counterpart to `real_bindings()`. Use only where synthetic hooks must be
/// lowered or emitted (codegen, LLR); prefer `real_bindings()` everywhere else.
pub fn bindings_including_synthetic(
    &self,
) -> impl Iterator<Item = (&SmolStr, &RefCell<BindingExpression>)>;

/// The raw binding cell for `name`, including a synthetic hook.
///
/// Returns the `&RefCell` rather than a borrow guard, so callers that need to
/// borrow, drop, and re-borrow within one scope (reentrant binding analysis) or
/// use `try_borrow` can do so. Does not filter synthetic hooks.
pub fn binding_cell_including_synthetic(
    &self,
    name: &str,
) -> Option<&RefCell<BindingExpression>>;
```

### Bulk escape hatch

The C sites move or rebuild the whole map across elements (`move_declarations`, `inlining`, `repeater_component`, the `typeloader` snapshot).
They keep raw access through explicit, named methods rather than a public field.

```rust
pub(crate) fn take_bindings_including_synthetic(&mut self) -> BindingsMap;
pub(crate) fn extend_bindings_including_synthetic(
    &mut self,
    it: impl IntoIterator<Item = (SmolStr, RefCell<BindingExpression>)>,
);
```

### Reading and writing past a hook wrapper

Reading: the A2 pattern matches `binding.expression` against an expression variant, which silently stops matching once the hook wrapper appears (see `layout.rs:704` in the appendix). `value_expression` makes the correct pattern the easy one.

Writing: `lower_states` and `materialize_fake_properties` set a value onto a slot that may already hold a two-way binding or a synthetic hook. They reach the binding through `binding_cell_including_synthetic` and call `set_value_expression`, falling back to `set_binding` when the slot is empty.

```rust
impl BindingExpression {
    /// The bound expression with any debug-hook wrapper removed.
    /// Use before matching on the expression variant.
    pub fn value_expression(&self) -> &Expression {
        self.expression.ignore_debug_hooks()
    }

    /// Replace the bound value, leaving priority, animation and two-way bindings
    /// untouched. A synthetic hook is upgraded in place — its wrapper and id are
    /// kept and it becomes real — so the property stays live-editable.
    pub fn set_value_expression(&mut self, expr: Expression);
}
```

## Decisions made

- **One `take_binding`, not two.**
  It removes the entry unconditionally but returns `Some` only for a real binding.
  Removing a leftover synthetic hook is always safe: the properties taken this way (`z`, `commands`, `init`, shadow and popup properties) are being lowered away, and dropping the hook keeps `validate_no_orphan_synthetic_hooks` satisfied.
  Rename sites become `take_binding` followed by `set_binding` under the new name.

- **Synthetic-inclusive accessors are named, never implicit.**
  `real_bindings()` / `bindings_including_synthetic()` and `binding()` / `binding_cell_including_synthetic()` sit side by side.
  The bulk helpers carry the same suffix: `take_bindings_including_synthetic`, `extend_bindings_including_synthetic`.

- **`bindings` becomes module-private, not `pub(crate)`.**
  Only module privacy forces the passes through the API.
  The constructor, `Debug`, and `deep_clone` stay on direct field access as same-module code.

- **Separate `real_bindings` from `bindings_including_synthetic` rather than one parameterized accessor.**
  Two named methods keep the intent visible at the call site and guard against future A4-style bugs.

- **Value replacement lives on `BindingExpression`, not on `Element`.**
  The one thing the existing setters cannot do — swap a property's value while keeping its priority, animation and two-way binding, upgrading a synthetic hook in place — is `BindingExpression::set_value_expression`.
  The two call sites reach the binding through `binding_cell_including_synthetic` and fall back to `set_binding` for an empty slot, so no dedicated upsert method is added to `Element`.

- **`inject_debug_hooks` reuses the general API.**
  It checks for and iterates entries with `bindings_including_synthetic`, and inserts its synthetic hooks with `set_binding` after filtering out names that already have an entry — so no hook-machinery-only accessors are needed.

## Open questions

These are not yet settled and need a decision before or during implementation.

- **How far to lock down category C.**
  The two bulk helpers cover whole-map take and extend.
  `inlining` merges one element's map into another's with per-entry `Entry` logic and priority arithmetic; it is unclear whether that expresses cleanly on top of `take`/`extend`, or needs its own helper.
  Open: the final set of escape-hatch methods.

- **Access from the external `tests/` crate.**
  `tests/consistent_styles.rs` and `tests/lower_shadows.rs` read `bindings` directly and must move to the accessors to compile once the field is private.
  Open: whether the reads all map onto `binding()` / `real_bindings()`, or whether a `#[cfg(test)]`-gated helper is warranted.

- **Priority semantics at `remove_aliases.rs:184`.**
  Current code reads the priority off whatever occupies the slot, synthetic or not, and adds one.
  A hook-aware rewrite changes the result when only a synthetic hook is present.
  Open: preserve the current behavior with a priority-peek accessor, or accept the semantic change.

## Appendix: audit of the raw-`bindings` sites

The sites below read the raw `bindings` map in a hook-unaware way. An earlier draft of this document called them latent bugs. They were then audited empirically: each was driven with a real `.slint` case through the interpreter test driver, run with and without `--features inject-debug-hooks`.
**None reproduced a hooks-only failure — all sixteen are currently non-triggerable.**
Migrating them through the API is therefore hardening (guarding against future regressions and pass reordering), not fixing shipped bugs. They are safe today for five distinct reasons.

1. **Target is a reserved property, so no synthetic hook exists** (the largest group). `clip.rs:32` (`clip`), `visible.rs:39/70` (`visible`), `lower_accessibility.rs:23/54` (`accessible-*`), `lower_popups.rs:51-52` (`close-policy`/`close-on-click`, also `Constexpr`), `lower_tabwidget.rs:130/144/158` (`visible`/`accessible-*`), `lower_layout.rs:2541` (`col`/`row`/`flex-*`/`dialog-button-role`), `object_tree.rs:3753` (`flex-*`). These properties are never in `property_list()`, so `property_defaults` never synthesizes a hook for them and the raw check sees exactly what it saw before hooks.

2. **The pass runs before `inject_debug_hooks`.** `check_builtin_shadowing.rs:66` runs in `run_import_passes`, before hooks are injected, so the map cannot yet hold a synthetic entry. `lower_layout.rs:1338` overwrites `layoutinfo-h/v`, which `default_geometry` synthesizes *after* hooks — so that binding is never hook-wrapped.

3. **A downstream `has_binding()` re-derive masks it.** `layout.rs:570` (`init_fake_property`) is fooled for `spacing-horizontal`/`-vertical` (which do get hooks), but `LayoutGeometry::new` immediately re-derives the value via `binding_reference()`/`has_binding()`, which excludes synthetic hooks, so the solver still uses the correct fallback.

4. **The effect is optimization-only, with a correct runtime fallback.** `layout.rs:704` (`compile_time_direction`) genuinely forgets `ignore_debug_hooks()`, so a real `direction: row/column` binding loses its compile-time-constant classification under hooks (axis → `Unknown`) — but every consumer falls back to the runtime path already used for a dynamic `direction`, so the result stays correct. `namedreference.rs:95` (`is_constant`) and `resolve_native_classes.rs:34` similarly only feed optimizations that fall back safely (native-class selection can pick a larger tier, but the extra properties have no-op defaults, and the one landmine — `ClippedImage`'s source-clip — is deliberately kept consistent in `default_geometry.rs`).

5. **Already defended by the PR author.** `lower_states.rs:180`'s `.unwrap()` is protected by the synthetic-hook upgrade-in-place at the `entry()` match earlier in the same function. `materialize_fake_properties.rs:42` is a no-op for synthetic keys (`z` is explicitly excluded from geometry hooks with a comment naming this pass). `windows.rs:93`'s `extend` is provably a no-op (its keys come from `property_list()` or `property_declarations`, both already handled).

**One flaw is worth fixing on its own merits:** `layout.rs:704` — the missing `ignore_debug_hooks()` in `compile_time_direction` (reason 4). It is not user-observable, but it is a real correctness-adjacent defect, and the `value_expression()` helper is the clean fix.

**Not covered by this audit** (structurally cannot fail the interpreter test driver, so not re-tested): `embed_glyphs.rs:181/675` (glyph-embedding path), `key_bindings.rs:33` (a *missing* warning), `generator/slint_sc.rs:32` (a different codegen backend), `inlining.rs:725/771` (perf only), and the hook-`id`-continuity sites `flickable.rs:69` / `lower_radiogroup.rs:152-223` / `lower_absolute_coordinates.rs:40` (live-edit only). These may still warrant the API under their own backends/features and should be checked separately.
