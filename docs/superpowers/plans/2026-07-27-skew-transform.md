# SkewTransform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `transform-skew-x` and `transform-skew-y` properties to Slint, enabling CSS-like skew/oblique transforms on any element.

**Architecture:** Skew is composed into the existing Transform element alongside rotation and scale. The transform pipeline is: compiler registers properties → lowers to Transform item → Transform::render calls backend → backend applies skew to canvas/matrix. Each renderer (Skia, FemtoVG, Software, anyrender) implements the skew natively.

**Tech Stack:** Rust, euclid (2D affine transforms), kurbo (for anyrender), Skia canvas API, FemtoVG canvas API

## Global Constraints

- Follow existing transform property naming: `transform-skew-x`, `transform-skew-y`
- Skew angles use `Type::Angle` (degrees), consistent with `transform-rotation`
- Default values: `0` (no skew)
- Transform composition order: translate(origin) → scale → skew → rotate → translate(-origin)
- All renderers must implement the new trait method (even if stubbed like software's rotate/scale)

---

## Task 1: Add skew properties to the Transform item

**Files:**
- Modify: `internal/core/items.rs:1094-1100` (Transform struct)
- Modify: `internal/core/items.rs:1164-1176` (Transform::render)

**Interfaces:**
- Consumes: `backend.skew(skew_x, skew_y)` from ItemRenderer trait (Task 2)
- Produces: `transform_skew_x`, `transform_skew_y` properties on Transform item

- [ ] **Step 1: Add fields to Transform struct**

In `internal/core/items.rs`, add two properties to the `Transform` struct:

```rust
pub struct Transform {
    pub transform_rotation: Property<f32>,
    pub transform_scale_x: Property<f32>,
    pub transform_scale_y: Property<f32>,
    pub transform_skew_x: Property<f32>,   // NEW
    pub transform_skew_y: Property<f32>,   // NEW
    pub transform_origin: Property<LogicalPosition>,
    pub cached_rendering_data: CachedRenderingData,
}
```

- [ ] **Step 2: Call skew in Transform::render**

In `Transform::render`, add the skew call between scale and rotate:

```rust
fn render(
    self: Pin<&Self>,
    backend: &mut ItemRendererRef,
    _self_rc: &ItemRc,
    _size: LogicalSize,
) -> RenderingResult {
    let origin = self.transform_origin().to_euclid().to_vector();
    (*backend).translate(origin);
    (*backend).scale(self.transform_scale_x(), self.transform_scale_y());
    (*backend).skew(self.transform_skew_x(), self.transform_skew_y());  // NEW
    (*backend).rotate(self.transform_rotation());
    (*backend).translate(-origin);
    RenderingResult::ContinueRenderingChildren
}
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p i-slint-core`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add internal/core/items.rs
git commit -m "feat(core): add skew properties to Transform item"
```

---

## Task 2: Add skew to ItemRenderer trait

**Files:**
- Modify: `internal/core/item_rendering.rs:557-564` (trait methods)

**Interfaces:**
- Consumes: skew implementation from each renderer
- Produces: `fn skew(&mut self, skew_x: f32, skew_y: f32)` trait method

- [ ] **Step 1: Add trait method**

In `internal/core/item_rendering.rs`, add `skew` to the `ItemRenderer` trait after `scale`:

```rust
fn scale(&mut self, scale_x_factor: f32, scale_y_factor: f32);
/// Apply a skew transformation. skew_x and skew_y are in degrees.
fn skew(&mut self, skew_x: f32, skew_y: f32);
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-core`
Expected: SUCCESS (trait method added but not yet implemented by renderers — this will fail in renderer crates)

- [ ] **Step 3: Commit**

```bash
git add internal/core/item_rendering.rs
git commit -m "feat(core): add skew to ItemRenderer trait"
```

---

## Task 3: Implement skew in Skia renderer

**Files:**
- Modify: `internal/renderers/skia/itemrenderer.rs:919-922` (after scale method)

**Interfaces:**
- Consumes: `fn skew(f32, f32)` from trait
- Produces: Skia canvas skew + euclid transform tracking

- [ ] **Step 1: Implement skew method**

Add after the `scale` method in `SkiaItemRenderer`:

```rust
fn skew(&mut self, skew_x: f32, skew_y: f32) {
    let skew_x_rad = skew_x.to_radians();
    let skew_y_rad = skew_y.to_radians();
    self.current_state.transform = self.current_state.transform.pre_skew(
        euclid::Angle::radians(skew_x_rad),
        euclid::Angle::radians(skew_y_rad),
    );
    self.canvas.skew((skew_x_rad, skew_y_rad));
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-renderer-skia`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/renderers/skia/itemrenderer.rs
git commit -m "feat(skia): implement skew transform"
```

---

## Task 4: Implement skew in FemtoVG renderer

**Files:**
- Modify: `internal/renderers/femtovg/itemrenderer.rs:929-936` (after scale method)

**Interfaces:**
- Consumes: `fn skew(f32, f32)` from trait
- Produces: FemtoVG canvas skew + clip recomputation

- [ ] **Step 1: Implement skew method**

FemtoVG has `skew_x(angle)` and `skew_y(angle)` methods on Canvas. Add after `scale`:

```rust
fn skew(&mut self, skew_x: f32, skew_y: f32) {
    let skew_x_rad = skew_x.to_radians();
    let skew_y_rad = skew_y.to_radians();
    self.canvas.borrow_mut().skew_x(skew_x_rad);
    self.canvas.borrow_mut().skew_y(skew_y_rad);
    // Recompute clip bounding box after skew (similar to rotate)
    let clip = &mut self.state.last_mut().unwrap().scissor;
    let tan_x = skew_x_rad.tan();
    let tan_y = skew_y_rad.tan();
    let skew_point = |p: LogicalPoint| {
        (p.x + p.y * tan_x, p.y + p.x * tan_y)
    };
    let corners = [
        skew_point(clip.origin),
        skew_point(clip.origin + euclid::vec2(clip.width(), 0.)),
        skew_point(clip.origin + euclid::vec2(0., clip.height())),
        skew_point(clip.origin + clip.size),
    ];
    let origin: LogicalPoint = (
        corners.iter().fold(f32::MAX, |a, b| b.0.min(a)),
        corners.iter().fold(f32::MAX, |a, b| b.1.min(a)),
    ).into();
    let end: LogicalPoint = (
        corners.iter().fold(f32::MIN, |a, b| b.0.max(a)),
        corners.iter().fold(f32::MIN, |a, b| b.1.max(a)),
    ).into();
    *clip = LogicalRect::new(origin, (end - origin).into());
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-renderer-femtovg`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/renderers/femtovg/itemrenderer.rs
git commit -m "feat(femtovg): implement skew transform"
```

---

## Task 5: Implement skew in Software renderer

**Files:**
- Modify: `internal/renderers/software/lib.rs:3216-3218` (after scale stub)

**Interfaces:**
- Consumes: `fn skew(f32, f32)` from trait
- Produces: Stub implementation (consistent with existing rotate/scale TODOs)

- [ ] **Step 1: Implement skew method**

The software renderer currently stubs rotate and scale. Add skew alongside them:

```rust
fn skew(&mut self, _skew_x: f32, _skew_y: f32) {
    // TODO (#6068) — same tracking issue as rotate/scale
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-renderer-software`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/renderers/software/lib.rs
git commit -m "feat(software): add skew stub (tracked in #6068)"
```

---

## Task 6: Implement skew in partial renderer

**Files:**
- Modify: `internal/core\partial_renderer.rs:710-712` (after scale method)

**Interfaces:**
- Consumes: `fn skew(f32, f32)` from trait
- Produces: Forwards skew to actual renderer

- [ ] **Step 1: Add forwarding method**

In `PartialRenderer`, add after `scale`:

```rust
fn skew(&mut self, skew_x: f32, skew_y: f32) {
    self.actual_renderer.skew(skew_x, skew_y)
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-core`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/core/item_rendering.rs internal/core/partial_renderer.rs
git commit -m "feat(core): forward skew in partial renderer"
```

---

## Task 7: Register skew properties in compiler

**Files:**
- Modify: `internal/compiler/typeregister.rs:231-236` (RESERVED_TRANSFORM_PROPERTIES)

**Interfaces:**
- Consumes: property names for transform lowering
- Produces: `transform-skew-x`, `transform-skew-y` as reserved transform properties

- [ ] **Step 1: Add to RESERVED_TRANSFORM_PROPERTIES**

```rust
pub const RESERVED_TRANSFORM_PROPERTIES: &[(&str, Type)] = &[
    ("transform-rotation", Type::Angle),
    ("transform-scale-x", Type::Float32),
    ("transform-scale-y", Type::Float32),
    ("transform-scale", Type::Float32),
    ("transform-skew-x", Type::Angle),   // NEW
    ("transform-skew-y", Type::Angle),   // NEW
];
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-compiler`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/compiler/typeregister.rs
git commit -m "feat(compiler): register transform-skew-x/y properties"
```

---

## Task 8: Add skew properties to Transform builtin

**Files:**
- Modify: `internal/compiler/builtins.slint:504-511` (Transform component)

**Interfaces:**
- Consumes: property names from typeregister
- Produces: `transform-skew-x`, `transform-skew-y` properties on Transform element

- [ ] **Step 1: Add properties to Transform**

```slint
export component Transform inherits Empty {
    in property <angle> transform-rotation;
    in property <percent> transform-scale-x;
    in property <percent> transform-scale-y;
    in property <angle> transform-skew-x;    // NEW
    in property <angle> transform-skew-y;    // NEW
    in property <Point> transform-origin;
    //-default_size_binding:expands_to_parent_geometry
    //-is_internal
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-compiler`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/compiler/builtins.slint
git commit -m "feat(compiler): add skew properties to Transform builtin"
```

---

## Task 9: Add default values for skew in transform lowering

**Files:**
- Modify: `internal/compiler/passes/lower_property_to_element.rs:140-163` (transform_property_default_value)

**Interfaces:**
- Consumes: property names
- Produces: default value expressions (0) for skew properties

- [ ] **Step 1: Add match arms for skew properties**

In `transform_property_default_value`, add cases before the `_ => unreachable!()`:

```rust
"transform-skew-x" | "transform-skew-y" => {
    Some(Expression::NumberLiteral(0., Default::default()))
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-compiler`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/compiler/passes/lower_property_to_element.rs
git commit -m "feat(compiler): add skew default values in transform lowering"
```

---

## Task 10: Add skew to inject_debug_hooks

**Files:**
- Modify: `internal/compiler/passes/inject_debug_hooks.rs:216-217` (TRANSFORM_PROPS array)

**Interfaces:**
- Consumes: property names
- Produces: skew properties included in debug hook injection

- [ ] **Step 1: Add skew to TRANSFORM_PROPS**

```rust
const TRANSFORM_PROPS: [&str; 5] = [
    "transform-rotation",
    "transform-scale-x",
    "transform-scale-y",
    "transform-skew-x",   // NEW
    "transform-skew-y",   // NEW
];
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-compiler`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/compiler/passes/inject_debug_hooks.rs
git commit -m "feat(compiler): include skew in transform debug hooks"
```

---

## Task 11: Add skew to children_transform

**Files:**
- Modify: `internal/core/item_tree.rs:983-993` (children_transform method)

**Interfaces:**
- Consumes: skew properties from Transform item
- Produces: skew composed into the affine transform for coordinate mapping

- [ ] **Step 1: Add then_skew to transform composition**

```rust
pub fn children_transform(&self) -> Option<ItemTransform> {
    self.downcast::<crate::items::Transform>().map(|transform_item| {
        let item = transform_item.as_pin_ref();
        let origin = item.transform_origin().to_euclid().to_vector().cast::<f32>();
        ItemTransform::translation(-origin.x, -origin.y)
            .cast()
            .then_scale(item.transform_scale_x(), item.transform_scale_y())
            .then_skew(
                euclid::Angle::radians(item.transform_skew_x().to_radians()),
                euclid::Angle::radians(item.transform_skew_y().to_radians()),
            )
            .then_rotate(euclid::Angle { radians: item.transform_rotation().to_radians() })
            .then_translate(origin)
    })
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p i-slint-core`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add internal/core/item_tree.rs
git commit -m "feat(core): compose skew into children_transform"
```

---

## Task 12: Add screenshot test for skew

**Files:**
- Create: `tests/cases/transforms/skew.slint`
- Test: `cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots`

**Interfaces:**
- Consumes: new skew properties
- Produces: visual regression test

- [ ] **Step 1: Create test case**

Create `tests/cases/transforms/skew.slint`:

```slint
export component SkewTest inherits Window {
    width: 200px;
    height: 200px;

    Rectangle {
        x: 50px;
        y: 50px;
        width: 100px;
        height: 100px;
        background: blue;
        transform-skew-x: 20deg;
    }

    Rectangle {
        x: 50px;
        y: 50px;
        width: 100px;
        height: 100px;
        background: red;
        opacity: 0.5;
        transform-skew-y: 20deg;
    }
}
```

- [ ] **Step 2: Generate reference screenshot**

Run: `SLINT_CREATE_SCREENSHOTS=1 cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots -- skew`
Expected: Reference PNG generated

- [ ] **Step 3: Verify test passes**

Run: `cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots -- skew`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tests/cases/transforms/skew.slint tests/screenshots/references/
git commit -m "test: add skew transform screenshot test"
```

---

## Task 13: Full build and test

**Files:** none (verification only)

- [ ] **Step 1: Build entire workspace**

Run: `cargo build`
Expected: SUCCESS

- [ ] **Step 2: Run core tests**

Run: `cargo test -p i-slint-core`
Expected: PASS

- [ ] **Step 3: Run compiler tests**

Run: `cargo test -p i-slint-compiler`
Expected: PASS

- [ ] **Step 4: Run screenshot tests**

Run: `cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots`
Expected: PASS (no regressions)

- [ ] **Step 5: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "feat: complete skew transform implementation" || echo "Nothing to commit"
```
