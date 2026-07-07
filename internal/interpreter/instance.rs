// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runtime component tree: a hierarchy of [`SubComponentInstance`]s rooted
//! in an [`Instance`].

use crate::erased::{ErasedItemRc, SubComponentCallback, SubComponentProperty};
use crate::globals::GlobalStorage;
use crate::item_registry::ItemRegistry;
use i_slint_compiler::llr::{
    self, CompilationUnit, ItemInstanceIdx, RepeatedElementIdx, SubComponentIdx,
    SubComponentInstanceIdx,
};
use i_slint_core::item_tree::{ItemTreeNode, ItemTreeVTable};
use i_slint_core::model::{Conditional, Repeater};
use i_slint_core::properties::ChangeTracker;
use i_slint_core::window::WindowAdapterRc;
use i_slint_core::{Callback, Property};
use std::cell::{OnceCell, RefCell};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use typed_index_collections::TiVec;
use vtable::{VRc, VWeak};

/// Either a `Repeater<Instance>` (`for` loops) or a `Conditional<Instance>`
/// (`if expr` elements).
/// The conditional variant reuses the existing instance while the condition
/// stays true, avoiding spurious re-init.
pub enum RepeaterOrConditional {
    Repeater(Pin<Box<Repeater<Instance>>>),
    Conditional(Pin<Box<Conditional<Instance>>>),
}

impl RepeaterOrConditional {
    pub fn visit(
        &self,
        order: i_slint_core::item_tree::TraversalOrder,
        visitor: i_slint_core::item_tree::ItemVisitorRefMut<'_>,
    ) -> i_slint_core::item_tree::VisitChildrenResult {
        match self {
            Self::Repeater(r) => Pin::as_ref(r).visit(order, visitor),
            Self::Conditional(c) => Pin::as_ref(c).visit(order, visitor),
        }
    }

    pub fn range(&self) -> core::ops::Range<usize> {
        match self {
            Self::Repeater(r) => r.range(),
            Self::Conditional(c) => c.range(),
        }
    }

    pub fn instance_at(&self, subindex: usize) -> Option<VRc<ItemTreeVTable, Instance>> {
        match self {
            Self::Repeater(r) => r.instance_at(subindex),
            Self::Conditional(c) => c.instance_at(subindex),
        }
    }

    pub fn instances_vec(&self) -> Vec<VRc<ItemTreeVTable, Instance>> {
        match self {
            Self::Repeater(r) => r.instances_vec(),
            Self::Conditional(c) => c.instances_vec(),
        }
    }

    /// Register the instance generation as a dependency of the current
    /// tracking scope. Layout expressions use this instead of instantiating,
    /// so they re-evaluate after the `ensure_instantiated` pass materializes
    /// instance changes.
    pub fn track_instance_changes(&self) {
        match self {
            Self::Repeater(r) => Pin::as_ref(r).track_instance_changes(),
            Self::Conditional(c) => Pin::as_ref(c).track_instance_changes(),
        }
    }

    /// Ensure the repeater/conditional has been updated. Must be called
    /// before accessing instances.
    /// Returns `true` if instances were created or removed.
    pub fn ensure_updated(
        &self,
        init: impl Fn() -> VRc<ItemTreeVTable, Instance> + 'static,
    ) -> bool {
        match self {
            Self::Repeater(r) => Pin::as_ref(r).ensure_updated(init),
            Self::Conditional(c) => Pin::as_ref(c).ensure_updated(init),
        }
    }

    /// Like `ensure_updated` but for listview repeaters that need
    /// virtualized row layout. The interpreter's viewport properties may
    /// live on a native item (e.g. `Flickable::viewport-y`), which
    /// doesn't expose a `Pin<&Property<Value>>` — so we go through the
    /// closure-based [`i_slint_core::model::ListViewProperties`] variant
    /// and let `load_property`/`store_property` route to rtti as needed.
    pub fn ensure_updated_listview_callback(
        &self,
        init: impl Fn() -> VRc<ItemTreeVTable, Instance> + 'static,
        props: &dyn i_slint_core::model::ListViewProperties,
        listview_width: i_slint_core::lengths::LogicalLength,
        listview_height: i_slint_core::lengths::LogicalLength,
    ) -> bool {
        match self {
            Self::Repeater(r) => Pin::as_ref(r).ensure_updated_listview_callback(
                init,
                props,
                listview_width,
                listview_height,
            ),
            Self::Conditional(_) => unreachable!("listview on a conditional element"),
        }
    }

    /// Set the model binding for `for` repeaters.
    pub fn set_model_binding(
        &self,
        binding: impl Fn() -> i_slint_core::model::ModelRc<crate::Value> + 'static,
    ) {
        match self {
            Self::Repeater(r) => Pin::as_ref(r).set_model_binding(binding),
            Self::Conditional(_) => unreachable!("set_model_binding on conditional"),
        }
    }

    /// Set the condition binding for conditional elements.
    pub fn set_condition_binding(&self, binding: impl Fn() -> bool + 'static) {
        match self {
            Self::Conditional(c) => c.set_model_binding(binding),
            Self::Repeater(_) => unreachable!("set_condition_binding on repeater"),
        }
    }

    /// Write model data back to a for-loop model row.
    pub fn model_set_row_data(&self, row: usize, data: crate::Value) {
        match self {
            Self::Repeater(r) => Pin::as_ref(r).model_set_row_data(row, data),
            Self::Conditional(_) => {} // conditionals have no model data
        }
    }

    pub fn is_conditional(&self) -> bool {
        matches!(self, Self::Conditional(_))
    }
}

/// Runtime instance of a single [`SubComponent`](llr::SubComponent).
///
/// Each field is indexed by its corresponding LLR index, so lookups are O(1).
pub struct SubComponentInstance {
    pub compilation_unit: Rc<CompilationUnit>,
    pub sub_component_idx: SubComponentIdx,
    pub properties: TiVec<llr::PropertyIdx, SubComponentProperty>,
    pub callbacks: TiVec<llr::CallbackIdx, SubComponentCallback>,
    /// For each callback with `needs_tracker`, a `Property<()>` that tracks
    /// handler changes: invoking the callback from a binding reads it to
    /// register a dependency; setting a new handler marks it dirty so
    /// dependent bindings re-evaluate.
    pub callback_trackers: TiVec<llr::CallbackIdx, Option<Pin<Rc<Property<()>>>>>,
    pub items: TiVec<ItemInstanceIdx, ErasedItemRc>,
    pub sub_components: TiVec<SubComponentInstanceIdx, Pin<Rc<SubComponentInstance>>>,
    /// One repeater per LLR `RepeatedElementIdx`.
    /// Conditional elements (`if expr`) use `Conditional<Instance>` which
    /// reuses the existing instance when the condition stays true; `for`
    /// loops use `Repeater<Instance>` which manages a `ModelRc<Value>`.
    pub repeaters: TiVec<RepeatedElementIdx, RepeaterOrConditional>,
    /// Resolves `MemberReference::Relative { parent_level: > 0 }`.
    pub parent: Weak<SubComponentInstance>,
    /// Back-reference to the owning root, populated right after construction.
    pub root: OnceCell<VWeak<ItemTreeVTable, Instance>>,
    /// Change trackers for this sub-component's `change_callbacks`.
    /// Stored here so they live as long as the instance.
    pub change_trackers: RefCell<Vec<ChangeTracker>>,
    /// Per-sub-component runtime `Timer`s, one per `SubComponent::timers`
    /// entry. Owned here so they stay alive with the instance; their
    /// lifecycle (start / stop / interval) is driven by a change tracker
    /// that re-evaluates the LLR `running` / `interval` expressions.
    pub timers: RefCell<Vec<i_slint_core::timers::Timer>>,
    /// One entry per `SubComponent::popup_windows`. Stores the currently
    /// open popup's id (handed out by `WindowInner::show_popup`) so a
    /// later `popup.close()` in the same sub-component can resolve which
    /// popup to tear down.
    pub popup_ids: RefCell<Vec<Option<std::num::NonZeroU32>>>,
    /// Set on the root sub-component of a repeated `Instance`. Points back to
    /// the parent sub-component holding the `Repeater` this instance belongs to.
    /// Used by `ModelDataAssignment` to write back into the model.
    pub repeated_in: OnceCell<(Weak<SubComponentInstance>, RepeatedElementIdx)>,
    /// Keeps the `MenuFromItemTree` alive so the weak reference stored by
    /// `setup_menubar_shortcuts` in the window remains valid.
    pub menubar: RefCell<Option<vtable::VRc<i_slint_core::menus::MenuVTable>>>,
}

/// Top-level item tree handed to i-slint-core via `VRc<ItemTreeVTable, _>`.
pub struct Instance {
    pub root_sub_component: Pin<Rc<SubComponentInstance>>,
    /// Flat `ItemTreeNode` slice returned by the `get_item_tree` vtable entry.
    pub tree_nodes: Box<[ItemTreeNode]>,
    /// Parallel table mapping each `DynamicTree` flat index to the
    /// `(sub_component_path, RepeatedElementIdx)` that owns the repeater.
    /// `None` entries correspond to non-dynamic nodes.
    pub dynamic_table: Box<[Option<(Box<[SubComponentInstanceIdx]>, RepeatedElementIdx)>]>,
    /// Parallel table mapping each static-item flat index to the
    /// `(sub_component_path, ItemInstanceIdx)` that owns it. `None`
    /// entries correspond to dynamic-tree nodes.
    pub item_table: Box<[Option<(Box<[SubComponentInstanceIdx]>, ItemInstanceIdx)>]>,
    pub globals: Rc<GlobalStorage>,
    pub self_weak: OnceCell<VWeak<ItemTreeVTable, Instance>>,
    /// When this `Instance` is a repeated entry, points back to the parent
    /// item tree so `parent_node` can return a meaningful weak.
    pub parent_instance: Weak<SubComponentInstance>,
    /// Index into `compilation_unit.public_components` for the public
    /// component this instance was built from. `None` for repeated /
    /// nested instances that don't correspond to a public component.
    pub public_component_index: Option<usize>,
    /// Lazily-created window adapter, used by `ImplicitLayoutInfo` and the
    /// public window/run helpers.
    pub window_adapter: OnceCell<WindowAdapterRc>,
    /// Message of the first failed window adapter creation. Later accesses
    /// return it instead of asking the platform again, so the first error is
    /// what `create()` reports — like generated code, where the error
    /// propagates out of `new()` at the first access.
    window_adapter_error: OnceCell<String>,
    /// Set once [`Instance::attach_to_window`] has linked the window adapter
    /// back to this item tree via `WindowInner::set_component`. Keeps the
    /// attach idempotent and lets binding-evaluated code paths distinguish
    /// "adapter exists" from "window is fully wired for display".
    pub window_attached: OnceCell<()>,
    /// Set once `bindings::install_bindings_only` has wired up property
    /// bindings, two-way links and timers. Idempotent on repeated calls.
    pub bindings_installed: OnceCell<()>,
    /// Set once the user-facing `init_code` has run on this instance. Kept
    /// separate from `bindings_installed` so the listview-virtualization
    /// factory can install bindings eagerly (so the first measurement
    /// returns the right row height) while still deferring `init_code`
    /// until the core's `init_instances` step.
    pub init_code_run: OnceCell<()>,
    /// When this instance has been embedded into another item tree via
    /// `embed_component`, stores the weak handle to the outer item tree and
    /// the flat index of the `ComponentContainer` it substitutes into.
    /// `parent_node` uses this to let coordinate-mapping helpers walk up
    /// into the outer tree.
    pub embedded_in: OnceCell<(VWeak<ItemTreeVTable>, u32)>,
    /// `TypeLoader` snapshots (post-pass + pre-pass) kept around for the
    /// highlight module and the LSP live preview's `DocumentCache`
    /// reconstruction. Both sides are `None` on sub-tree / popup / repeated
    /// instances — only the top-level definition sets them.
    pub type_loaders: crate::component::TypeLoaders,
}

impl Drop for Instance {
    fn drop(&mut self) {
        // Like the generated code's `unregister_item_tree` call: free the
        // per-component renderer caches (text shaping, bounding rects, …)
        // and notify any `WindowAdapterInternal` that the item tree is
        // going away. Skipping this leaks cache entries across destroyed
        // conditional/repeated sub-trees; once the allocator hands out a
        // fresh item at a previously-cached pointer, the renderer serves
        // the old widget's text / font / color.
        //
        // `self_weak` can't be upgraded here — the strong count is already
        // zero — so build a borrowed `VRef<ItemTreeVTable>` from `&*self`.
        let Some(adapter) = self.window_adapter.get().cloned().or_else(|| {
            let mut parent = self.parent_instance.upgrade();
            while let Some(sub) = parent {
                let root = sub.root.get().and_then(|w| w.upgrade())?;
                if let Some(a) = root.window_adapter.get() {
                    return Some(a.clone());
                }
                parent = root.parent_instance.upgrade();
            }
            None
        }) else {
            return;
        };
        vtable::new_vref!(let item_tree_ref : VRef<i_slint_core::item_tree::ItemTreeVTable> for i_slint_core::item_tree::ItemTree = self);
        let items = collect_item_refs(&self.root_sub_component);
        // Same order as `i_slint_core::item_tree::unregister_item_tree`:
        // deinit each item (a focused TextInput resets
        // `text-input-focused`), free the renderer caches, notify the
        // adapter, then close popups whose parent item just went away.
        for item in &items {
            item.as_ref().deinit(&adapter);
        }
        let _ =
            adapter.renderer().free_graphics_resources(item_tree_ref, &mut items.iter().copied());
        if let Some(internal) = adapter.internal(i_slint_core::InternalToken) {
            internal.unregister_item_tree(item_tree_ref, &mut items.iter().copied());
        }
        let window_inner = i_slint_core::window::WindowInner::from_pub(adapter.window());
        let to_close_popups = window_inner
            .active_popups()
            .iter()
            .filter_map(|p| p.parent_item.upgrade().is_none().then_some(p.popup_id))
            .collect::<Vec<_>>();
        for popup_id in to_close_popups {
            window_inner.close_popup(popup_id);
        }
    }
}

/// Collect every native item in `sub` and its nested sub-components as
/// pinned vtable refs for `free_graphics_resources` / `unregister_item_tree`.
fn collect_item_refs<'a>(
    sub: &'a Pin<Rc<SubComponentInstance>>,
) -> Vec<Pin<vtable::VRef<'a, i_slint_core::items::ItemVTable>>> {
    let mut out = Vec::new();
    fn walk<'a>(
        sub: &'a Pin<Rc<SubComponentInstance>>,
        out: &mut Vec<Pin<vtable::VRef<'a, i_slint_core::items::ItemVTable>>>,
    ) {
        for item in &sub.items {
            out.push(Pin::as_ref(item).as_item_ref());
        }
        for nested in &sub.sub_components {
            walk(nested, out);
        }
    }
    walk(sub, &mut out);
    out
}

impl Instance {
    /// Like [`Self::try_window_adapter`], but collapse the error case to
    /// `None` for the many callers that only need best-effort access.
    pub fn window_adapter_or_default(&self) -> Option<WindowAdapterRc> {
        self.try_window_adapter().ok()
    }

    /// Return a window adapter, creating one through the platform selector
    /// if needed. Failure to create one surfaces as the platform's error so
    /// callers with an error channel (e.g. `create()`) can report it.
    ///
    /// Does **not** call `WindowInner::set_component`: this method is called
    /// from inside binding evaluation (e.g. `ImplicitLayoutInfo`), and
    /// `set_component` eagerly reads and writes window-item properties,
    /// which would recurse into the in-flight binding. Call
    /// [`Self::attach_to_window`] separately from lifecycle entry points
    /// (show/run) to link the window back to this item tree.
    ///
    /// Sub-instances (popups, repeated/conditional sub-trees) inherit the
    /// adapter of the root instance instead of creating a fresh one — that
    /// would otherwise leave dispatched events going to a different window
    /// than the one the test driver captured.
    pub fn try_window_adapter(&self) -> Result<WindowAdapterRc, i_slint_core::api::PlatformError> {
        if let Some(a) = self.window_adapter.get() {
            return Ok(a.clone());
        }
        // An embedded instance reuses the outer tree's adapter. We must
        // _not_ create a fresh one: any resize event on it would fire
        // `set_window_item_geometry`, which walks the TwoWayBinding chain
        // down into `common_1.set(..)` and erases the ComponentContainer
        // width/height bindings the embedded root is supposed to track.
        if let Some((outer_weak, _)) = self.embedded_in.get()
            && let Some(outer) = outer_weak.upgrade()
        {
            let mut result = None;
            vtable::VRc::borrow_pin(&outer).as_ref().window_adapter(true, &mut result);
            if let Some(a) = result {
                let _ = self.window_adapter.set(a.clone());
                return Ok(a);
            }
        }
        // Walk up the parent chain to find an existing adapter on the root
        // instance, so popup-in-popup etc. share the same window.
        let mut outermost_root = None;
        let mut parent_sub = self.parent_instance.upgrade();
        while let Some(sub) = parent_sub {
            let Some(root_vrc) = sub.root.get().and_then(|w| w.upgrade()) else { break };
            if let Some(a) = root_vrc.window_adapter.get() {
                let cloned = a.clone();
                // Cache on this instance so future lookups don't have to walk
                // again, but don't store a *new* adapter on a non-root.
                let _ = self.window_adapter.set(cloned.clone());
                return Ok(cloned);
            }
            parent_sub = root_vrc.parent_instance.upgrade();
            outermost_root = Some(root_vrc);
        }
        if let Some(e) = self
            .window_adapter_error
            .get()
            .or_else(|| outermost_root.as_ref().and_then(|root| root.window_adapter_error.get()))
        {
            return Err(i_slint_core::api::PlatformError::Other(e.clone()));
        }
        let adapter = i_slint_backend_selector::with_platform(|p| p.create_window_adapter())
            .inspect_err(|e| {
                let msg = e.to_string();
                if let Some(root) = &outermost_root {
                    let _ = root.window_adapter_error.set(msg.clone());
                }
                let _ = self.window_adapter_error.set(msg);
            })?;
        // Point the renderer at its adapter right away: font registration in
        // `pre_init_code` and image decoding need the renderer's Slint context
        // before `attach_to_window` runs `set_component` on show.
        adapter.renderer().set_window_adapter(&adapter);
        // A freshly created adapter belongs to the outermost root instance;
        // caching it only on a sub-tree would leave the root creating a
        // second one later, splitting the tree across two windows.
        if let Some(root) = outermost_root {
            let _ = root.window_adapter.set(adapter.clone());
        }
        let _ = self.window_adapter.set(adapter.clone());
        Ok(adapter)
    }

    /// Link this instance's root item tree into its window adapter via
    /// `WindowInner::set_component`, if not already attached.
    ///
    /// Must be called from a context that is **not** currently evaluating a
    /// property binding — `set_component` touches geometry and scale-factor
    /// trackers and would otherwise trip `Recursion detected`. The public
    /// `show()` / `run()` entry points call this before handing off to the
    /// backend event loop. Idempotent via the `window_attached` flag.
    pub fn attach_to_window(&self) {
        if self.window_attached.get().is_some() {
            return;
        }
        let Some(adapter) = self.window_adapter_or_default() else { return };
        let Some(self_rc) = self.self_weak.get().and_then(|w| w.upgrade()) else { return };
        let _ = self.window_attached.set(());
        i_slint_core::window::WindowInner::from_pub(adapter.window())
            .set_component(&vtable::VRc::into_dyn(self_rc));
    }
}

/// When the LLR `RepeatedElement` at `rep_idx` is actually a
/// `ComponentContainer` placeholder (created by `lower_component_container`),
/// return a pinned reference to the `ComponentContainer` item that hosts
/// the embedded tree. Returns `None` for regular repeaters and conditional
/// elements.
pub(crate) fn component_container_item(
    sub: &Pin<Rc<SubComponentInstance>>,
    rep_idx: RepeatedElementIdx,
) -> Option<Pin<&i_slint_core::items::ComponentContainer>> {
    let sc = &sub.compilation_unit.sub_components[sub.sub_component_idx];
    let cc_item_idx = sc.repeated.get(rep_idx)?.container_item_index?;
    let item = sub.items.get(cc_item_idx)?;
    i_slint_core::items::ItemRef::downcast_pin::<i_slint_core::items::ComponentContainer>(
        Pin::as_ref(item).as_item_ref(),
    )
}

impl Instance {
    /// Resolve a flat `tree_nodes` index into the owning sub-component and
    /// its local repeater index by walking the cached
    /// `dynamic_table` entry's `sub_component_path`.
    pub fn dynamic_at(
        &self,
        tree_index: u32,
    ) -> Option<(Pin<Rc<SubComponentInstance>>, RepeatedElementIdx)> {
        let entry = self.dynamic_table.get(tree_index as usize)?.as_ref()?;
        let mut current = self.root_sub_component.clone();
        for &idx in entry.0.iter() {
            let next = current.sub_components[idx].clone();
            current = next;
        }
        Some((current, entry.1))
    }

    /// Ensure the repeater at `tree_index` is populated from its model.
    /// Called by `get_subtree_range`, `get_subtree` and
    /// `visit_dynamic_children` before reading the repeater's instances.
    ///
    /// When the LLR `RepeatedElement` is actually a `ComponentContainer`
    /// placeholder (`container_item_index = Some`), defer to the
    /// `ComponentContainer` item's own `ensure_updated`, which drives
    /// the `ComponentFactory` and stores the embedded item tree on the
    /// container item directly — the repeater slot stays a no-op
    /// `Conditional` with `model: false`.
    pub fn ensure_updated(&self, tree_index: u32) -> bool {
        let Some((sub, rep_idx)) = self.dynamic_at(tree_index) else { return false };
        if let Some(cc) = component_container_item(&sub, rep_idx) {
            return cc.ensure_updated();
        }
        let cu = sub.compilation_unit.clone();
        let sc_idx = sub.sub_component_idx;
        let sub_weak = Rc::downgrade(&Pin::into_inner(sub.clone()));
        let globals = self.globals.clone();
        let repeated = &cu.sub_components[sc_idx].repeated[rep_idx];
        let listview_factory = repeated.listview.is_some();
        let listview_info = repeated.listview.clone();
        let factory = move || {
            let item_tree = &cu.sub_components[sc_idx].repeated[rep_idx].sub_tree;
            let vrc = Instance::new_repeated(
                cu.clone(),
                item_tree,
                sub_weak.clone(),
                rep_idx,
                globals.clone(),
            );
            if listview_factory {
                // The listview measurement reads row heights *before* the
                // core calls `RepeatedItemTree::init` on each row, so the
                // height/width/geometry bindings must be in place
                // immediately; `init_code` stays deferred to `init()`.
                install_bindings_for_repeated_row(&vrc);
            }
            vrc
        };
        let repeater = &sub.repeaters[rep_idx];
        if let Some(lv) = listview_info.as_ref() {
            let listview_width = read_logical_length(&sub, &lv.listview_width);
            let listview_height = read_logical_length(&sub, &lv.listview_height);
            // If layout hasn't propagated a real visible height yet (eager
            // hit-test before show()), bail out instead of running the
            // virtualization with `0`, which would create no rows or — with
            // the loop_count == 3 retry — instantiate the whole model.
            if listview_height.get() <= 0.0 {
                return false;
            }
            let props = ValueListViewProps {
                viewport_y: lv.viewport_y.clone(),
                viewport_width: lv.viewport_width.clone(),
                viewport_height: lv.viewport_height.clone(),
                ctx_sub: sub.clone(),
            };
            repeater.ensure_updated_listview_callback(
                factory,
                &props,
                listview_width,
                listview_height,
            )
        } else {
            repeater.ensure_updated(factory)
        }
    }

    /// Instantiate every repeater, conditional and `ComponentContainer` in
    /// this item tree. Runs as a dedicated update pass before rendering and
    /// event dispatch, so the visit pass only has to register dependencies.
    /// Returns `true` if any instance was created or removed.
    pub fn ensure_instantiated(&self) -> bool {
        let mut changed = false;
        for idx in 0..self.dynamic_table.len() {
            if self.dynamic_table[idx].is_some() {
                changed |= self.ensure_updated(idx as u32);
            }
        }
        changed
    }

    /// `visit_children_item` entry point for `DynamicTree` nodes.
    ///
    /// For `ComponentContainer` placeholders the visit delegates to the
    /// container item's own `visit_children_item`, which hops into the
    /// embedded item tree stored on the container. The repeater slot is
    /// a dummy `Conditional` (see `lower_component_container`) and must
    /// not be visited directly, or the embedded content never renders.
    pub fn visit_dynamic_children(
        self: Pin<&Self>,
        dyn_index: u32,
        order: i_slint_core::item_tree::TraversalOrder,
        visitor: vtable::VRefMut<'_, i_slint_core::item_tree::ItemVisitorVTable>,
    ) -> i_slint_core::item_tree::VisitChildrenResult {
        let Some((sub, rep_idx)) = self.get_ref().dynamic_at(dyn_index) else {
            return i_slint_core::item_tree::VisitChildrenResult::CONTINUE;
        };
        if let Some(cc) = component_container_item(&sub, rep_idx) {
            return cc.visit_children_item(-1, order, visitor);
        }
        // Instantiation happens in the `ensure_instantiated` pass; the visit
        // only registers dependencies so the redraw tracker is notified when
        // the model or the ListView viewport geometry changes.
        let repeater = &sub.repeaters[rep_idx];
        let sc = &sub.compilation_unit.sub_components[sub.sub_component_idx];
        if let (Some(lv), RepeaterOrConditional::Repeater(r)) =
            (sc.repeated[rep_idx].listview.as_ref(), repeater)
        {
            let props = ValueListViewProps {
                viewport_y: lv.viewport_y.clone(),
                viewport_width: lv.viewport_width.clone(),
                viewport_height: lv.viewport_height.clone(),
                ctx_sub: sub.clone(),
            };
            let listview_width = read_logical_length(&sub, &lv.listview_width);
            let _ = read_logical_length(&sub, &lv.listview_height);
            Pin::as_ref(r).track_changes_listview_callback(&props, listview_width);
        }
        repeater.visit(order, visitor)
    }

    /// Build an instance for a public component.
    ///
    /// Properties are default-valued, then `bindings::install_bindings` wires
    /// up `property_init`, `two_way_bindings` and `init_code`.
    pub fn new(
        compilation_unit: Rc<CompilationUnit>,
        public_component_index: usize,
    ) -> VRc<ItemTreeVTable, Instance> {
        Self::new_with_window(compilation_unit, public_component_index, None, Default::default())
    }

    /// Build an instance for a public component and optionally reuse an
    /// existing [`WindowAdapterRc`]. Live preview passes in the window from
    /// the old instance so reloaded components keep the same window frame.
    pub fn new_with_window(
        compilation_unit: Rc<CompilationUnit>,
        public_component_index: usize,
        window_adapter: Option<i_slint_core::window::WindowAdapterRc>,
        type_loaders: crate::component::TypeLoaders,
    ) -> VRc<ItemTreeVTable, Instance> {
        Self::new_with_options(
            compilation_unit,
            public_component_index,
            window_adapter,
            type_loaders,
            None,
        )
    }

    /// Build an instance embedded inside an existing item tree via a
    /// `ComponentFactory`. Records the outer item tree handle and the
    /// `ComponentContainer` slot index it substitutes into so that
    /// `parent_node` can walk back into the host tree.
    pub fn new_embedded(
        compilation_unit: Rc<CompilationUnit>,
        public_component_index: usize,
        type_loaders: crate::component::TypeLoaders,
        parent: vtable::VWeak<ItemTreeVTable>,
        parent_item_tree_index: u32,
    ) -> VRc<ItemTreeVTable, Instance> {
        Self::new_with_options(
            compilation_unit,
            public_component_index,
            None,
            type_loaders,
            Some((parent, parent_item_tree_index)),
        )
    }

    fn new_with_options(
        compilation_unit: Rc<CompilationUnit>,
        public_component_index: usize,
        window_adapter: Option<i_slint_core::window::WindowAdapterRc>,
        type_loaders: crate::component::TypeLoaders,
        embedded_in: Option<(vtable::VWeak<ItemTreeVTable>, u32)>,
    ) -> VRc<ItemTreeVTable, Instance> {
        let public = &compilation_unit.public_components[public_component_index];
        let globals = Rc::new(GlobalStorage::new(&compilation_unit));
        let item_tree = &public.item_tree;
        let vrc = build_instance(
            &compilation_unit,
            item_tree,
            Weak::new(),
            globals,
            Some(public_component_index),
            type_loaders,
        );
        if let Some(adapter) = window_adapter {
            let _ = vrc.window_adapter.set(adapter);
        }
        // Set the outer-tree handle before finalizing so bindings that
        // read absolute coordinates during `install_bindings` /
        // `init_code` can resolve `parent_node` through the host.
        if let Some((parent, idx)) = embedded_in {
            let _ = vrc.embedded_in.set((parent, idx));
        }
        finalize_instance(&vrc);
        vrc
    }

    /// Build an instance for a repeated sub-tree, sharing `globals` with its
    /// owning root instance.
    /// `repeater_idx` lets `ModelDataAssignment` find the owning repeater
    /// when an event in the repeated sub-tree wants to write back.
    pub fn new_repeated(
        compilation_unit: Rc<CompilationUnit>,
        item_tree: &llr::ItemTree,
        parent: Weak<SubComponentInstance>,
        repeater_idx: RepeatedElementIdx,
        globals: Rc<GlobalStorage>,
    ) -> VRc<ItemTreeVTable, Instance> {
        let vrc = build_instance(
            &compilation_unit,
            item_tree,
            parent.clone(),
            globals,
            None,
            Default::default(),
        );
        let _ = vrc.root_sub_component.repeated_in.set((parent, repeater_idx));
        vrc
    }

    /// Build an instance for a popup sub-tree. The resulting `Instance` is
    /// parented on the sub-component that owns the popup so that parent-
    /// relative property references resolve through `parent.upgrade()`.
    pub fn new_popup(
        compilation_unit: Rc<CompilationUnit>,
        item_tree: &llr::ItemTree,
        parent: Weak<SubComponentInstance>,
        globals: Rc<GlobalStorage>,
    ) -> VRc<ItemTreeVTable, Instance> {
        build_instance(&compilation_unit, item_tree, parent, globals, None, Default::default())
    }
}

/// Allocate the `Instance` skeleton (sub-component tree, items, repeaters,
/// tree nodes, globals) but do **not** install bindings yet.
///
/// Bindings install happens via [`finalize_instance`], which the caller
/// invokes once the parent repeater (if any) has dropped its `RefCell`
/// borrow. This avoids re-entrant repeater access when an `init` callback
/// reads a layout property that walks back through the same repeater.
fn build_instance(
    compilation_unit: &Rc<CompilationUnit>,
    item_tree: &llr::ItemTree,
    parent: Weak<SubComponentInstance>,
    globals: Rc<GlobalStorage>,
    public_component_index: Option<usize>,
    type_loaders: crate::component::TypeLoaders,
) -> VRc<ItemTreeVTable, Instance> {
    let parent_for_root = parent.clone();
    let root_sub_component =
        build_sub_component_instance(compilation_unit, item_tree.root, parent_for_root);
    let (tree_nodes, dynamic_table, item_table) = build_tree_nodes(&item_tree.tree);

    let vrc = VRc::new(Instance {
        root_sub_component,
        tree_nodes: tree_nodes.into_boxed_slice(),
        dynamic_table: dynamic_table.into_boxed_slice(),
        item_table: item_table.into_boxed_slice(),
        globals,
        self_weak: OnceCell::new(),
        parent_instance: parent,
        public_component_index,
        window_adapter: OnceCell::new(),
        window_adapter_error: OnceCell::new(),
        window_attached: OnceCell::new(),
        bindings_installed: OnceCell::new(),
        init_code_run: OnceCell::new(),
        embedded_in: OnceCell::new(),
        type_loaders,
    });
    let weak = VRc::downgrade(&vrc);
    let _ = vrc.self_weak.set(weak.clone());
    // Repeated sub-trees and popups share their owner's storage; keep its root.
    let _ = vrc.globals.root.set(weak.clone());
    propagate_root(&vrc.root_sub_component, &weak);
    vrc
}

/// Install global, sub-component and init bindings on a freshly built
/// instance, then run `init_code`.
///
/// Idempotent: separate `OnceCell` flags guard the bindings install and
/// the `init_code` step so each side can be called independently. The
/// listview virtualization path uses
/// [`install_bindings_for_repeated_row`] to install bindings before the
/// first measurement and defers `init_code` to the core's
/// `init_instances` callback (`<Instance as RepeatedItemTree>::init`).
pub(crate) fn finalize_instance(vrc: &VRc<ItemTreeVTable, Instance>) {
    install_bindings_for_repeated_row(vrc);
    if vrc.init_code_run.get().is_some() {
        return;
    }
    let _ = vrc.init_code_run.set(());
    // For top-level instances, attach the window to the item tree *before*
    // running init_code so `set_component` doesn't clear focus set by
    // `forward-focus`. Embedded instances piggy-back on the host tree's
    // adapter (see `window_adapter_or_default`) and skip this: the host
    // has already run `set_component`, and running it again on the
    // embedded root would reroute the host's window events into the sub-
    // tree and clobber the ComponentContainer-driven size bindings.
    if vrc.public_component_index.is_some() && vrc.embedded_in.get().is_none() {
        vrc.attach_to_window();
    }
    // Call Item::init() on every native item and register the item tree
    // with the window adapter. Registration matters: the rendering backend
    // keeps per-component caches (text shaping, bounding rects) released
    // only by the matching `unregister_item_tree` on Drop, and skipping
    // the pair leaks entries until the renderer serves stale data for
    // reused item addresses.
    {
        let dyn_rc = vtable::VRc::into_dyn(vrc.self_weak.get().unwrap().upgrade().unwrap());
        let adapter = vrc.window_adapter_or_default();
        i_slint_core::item_tree::register_item_tree(&dyn_rc, adapter);
    }
    crate::bindings::run_init_code_for_instance(vrc);
}

/// Install bindings, two-way links and timers on `vrc` without running
/// `init_code`. Used by the listview row factory; safe to call from any
/// other path that needs bindings in place but doesn't want to fire user
/// init handlers yet.
pub(crate) fn install_bindings_for_repeated_row(vrc: &VRc<ItemTreeVTable, Instance>) {
    if vrc.bindings_installed.get().is_some() {
        return;
    }
    let _ = vrc.bindings_installed.set(());
    let is_root = vrc.parent_instance.upgrade().is_none();
    if is_root {
        crate::globals::install_global_bindings(&vrc.globals);
    }
    crate::bindings::install_bindings_only(vrc);
}

/// Back-fill the root weak reference on every sub-component under `sub`.
fn propagate_root(sub: &Pin<Rc<SubComponentInstance>>, weak: &VWeak<ItemTreeVTable, Instance>) {
    let _ = sub.root.set(weak.clone());
    for nested in &sub.sub_components {
        propagate_root(nested, weak);
    }
}

/// Recursively allocate a [`SubComponentInstance`].
fn build_sub_component_instance(
    cu: &Rc<CompilationUnit>,
    sub_idx: SubComponentIdx,
    parent: Weak<SubComponentInstance>,
) -> Pin<Rc<SubComponentInstance>> {
    let sc = &cu.sub_components[sub_idx];
    let registry = ItemRegistry::global();

    let properties = sc
        .properties
        .iter()
        .map(|p| Rc::pin(Property::new(crate::eval::default_value_for_type(&p.ty))))
        .collect();
    let callbacks = sc.callbacks.iter().map(|_| Rc::pin(Callback::default())).collect();
    let callback_trackers =
        sc.callbacks.iter().map(|c| c.needs_tracker.then(|| Rc::pin(Property::new(())))).collect();
    let items =
        sc.items
            .iter()
            .map(|item| {
                registry.factory(&item.ty.class_name).unwrap_or_else(|| {
                    panic!("native item `{}` is not registered", item.ty.class_name)
                })()
            })
            .collect();
    let repeaters = sc
        .repeated
        .iter()
        .map(|rep| {
            if rep.data_prop.is_none() {
                RepeaterOrConditional::Conditional(Box::pin(Conditional::default()))
            } else {
                RepeaterOrConditional::Repeater(Box::pin(Repeater::default()))
            }
        })
        .collect();

    // `Rc::new_cyclic` gives nested sub-components a `Weak` to their parent.
    // `SubComponentInstance` is `Unpin` (every pinned field lives behind its own
    // `Pin<Rc<_>>`), so `Pin::new` on the resulting `Rc` needs no unsafe.
    let rc = Rc::new_cyclic(|weak_self: &Weak<SubComponentInstance>| {
        let sub_components = sc
            .sub_components
            .iter()
            .map(|nested| build_sub_component_instance(cu, nested.ty, weak_self.clone()))
            .collect();
        SubComponentInstance {
            compilation_unit: cu.clone(),
            sub_component_idx: sub_idx,
            properties,
            callbacks,
            callback_trackers,
            items,
            sub_components,
            repeaters,
            parent,
            root: OnceCell::new(),
            change_trackers: RefCell::new(Vec::new()),
            timers: RefCell::new(
                (0..sc.timers.len()).map(|_| i_slint_core::timers::Timer::default()).collect(),
            ),
            popup_ids: RefCell::new(vec![None; sc.popup_windows.len()]),
            repeated_in: OnceCell::new(),
            menubar: RefCell::new(None),
        }
    });
    Pin::new(rc)
}

/// Read a `MemberReference` (rooted in `sub`) and convert the result to a
/// `LogicalLength`. Used to seed the listview virtualization with the
/// listview-width / listview-height values stored as `Value::Number`.
fn read_logical_length(
    sub: &Pin<Rc<SubComponentInstance>>,
    mr: &llr::MemberReference,
) -> i_slint_core::lengths::LogicalLength {
    let mut ctx = crate::eval::EvalContext::new(sub.clone());
    let v = crate::eval::load_property(&ctx, mr);
    let _ = &mut ctx;
    let n: f64 = v.try_into().unwrap_or(0.0);
    i_slint_core::lengths::LogicalLength::new(n as f32)
}

/// Shim implementing [`i_slint_core::model::ListViewProperties`] over
/// the interpreter's `Value`-typed viewport storage. The viewport
/// references may be user-declared `Property<Value>` fields *or* native
/// item properties (e.g. `Flickable::viewport-y`); routing through
/// `load_property` / `store_property` handles both uniformly.
struct ValueListViewProps {
    viewport_y: llr::MemberReference,
    /// `None` when the user set `viewport-width` explicitly, in which case
    /// the ListView must not overwrite it (see #12264).
    viewport_width: Option<llr::MemberReference>,
    viewport_height: Option<llr::MemberReference>,
    ctx_sub: Pin<Rc<SubComponentInstance>>,
}

impl i_slint_core::model::ListViewProperties for ValueListViewProps {
    fn viewport_y_get(&self) -> i_slint_core::lengths::LogicalLength {
        read_logical_length(&self.ctx_sub, &self.viewport_y)
    }
    fn viewport_y_get_internal(&self) -> i_slint_core::lengths::LogicalLength {
        // The rtti route has no equivalent of `Property::get_internal`;
        // reading normally only differs while a physics animation drives
        // `viewport-y`, where it may re-evaluate the animated binding.
        read_logical_length(&self.ctx_sub, &self.viewport_y)
    }
    fn viewport_y_set(&self, value: i_slint_core::lengths::LogicalLength) {
        let ctx = crate::eval::EvalContext::new(self.ctx_sub.clone());
        crate::eval::store_property(
            &ctx,
            &self.viewport_y,
            crate::Value::Number(value.get() as f64),
        );
    }
    fn viewport_y_has_binding(&self) -> bool {
        // The interpreter doesn't track whether the underlying property
        // has an external binding; `false` matches the rust codegen
        // default and lets `update_visible_instances` clamp the value
        // when scrolling.
        false
    }
    fn has_viewport_width(&self) -> bool {
        self.viewport_width.is_some()
    }
    fn has_viewport_height(&self) -> bool {
        self.viewport_height.is_some()
    }
    fn viewport_width_set(&self, value: i_slint_core::lengths::LogicalLength) {
        let Some(viewport_width) = &self.viewport_width else { return };
        let ctx = crate::eval::EvalContext::new(self.ctx_sub.clone());
        crate::eval::store_property(&ctx, viewport_width, crate::Value::Number(value.get() as f64));
    }
    fn viewport_height_set(&self, value: i_slint_core::lengths::LogicalLength) {
        let Some(viewport_height) = &self.viewport_height else { return };
        let ctx = crate::eval::EvalContext::new(self.ctx_sub.clone());
        crate::eval::store_property(
            &ctx,
            viewport_height,
            crate::Value::Number(value.get() as f64),
        );
    }
    fn register_as_dependencies(&self) {
        // Reading through `load_property` registers the dependency with the
        // current tracking scope, which is all this hook needs.
        if let Some(viewport_width) = &self.viewport_width {
            let _ = read_logical_length(&self.ctx_sub, viewport_width);
        }
        if let Some(viewport_height) = &self.viewport_height {
            let _ = read_logical_length(&self.ctx_sub, viewport_height);
        }
        let _ = read_logical_length(&self.ctx_sub, &self.viewport_y);
    }
}

type DynamicEntry = Option<(Box<[SubComponentInstanceIdx]>, RepeatedElementIdx)>;
type ItemEntry = Option<(Box<[SubComponentInstanceIdx]>, ItemInstanceIdx)>;

/// Flatten an LLR [`llr::TreeNode`] into the `ItemTreeNode` slice expected by
/// the `get_item_tree` vtable entry, plus two parallel tables: one mapping
/// flat indices to the dynamic repeaters they represent, and one mapping
/// static flat indices to the sub-component path + items slot that owns them.
///
/// Walks in the same order as [`llr::TreeNode::visit_in_array`], so flat
/// indices match what the rest of the runtime expects.
fn build_tree_nodes(
    root: &llr::TreeNode,
) -> (Vec<ItemTreeNode>, Vec<DynamicEntry>, Vec<ItemEntry>) {
    use itertools::Either;

    let mut out = Vec::new();
    let mut dyn_table: Vec<DynamicEntry> = Vec::new();
    let mut item_table: Vec<ItemEntry> = Vec::new();
    root.visit_in_array(&mut |node, children_offset, parent_index| {
        let parent_index = parent_index as u32;
        let (entry, dyn_entry, item_entry) = match node.item_index {
            Either::Left(item_idx) => (
                ItemTreeNode::Item {
                    is_accessible: node.is_accessible,
                    children_count: node.children.len() as u32,
                    children_index: children_offset as u32,
                    parent_index,
                    // `item_array_index` is the flat tree index so
                    // `get_item_ref` can walk the item_table directly.
                    item_array_index: out.len() as u32,
                },
                None,
                Some((node.sub_component_path.clone().into_boxed_slice(), item_idx)),
            ),
            Either::Right(dynamic_index) => (
                // The `index` field on `DynamicTree` is opaque to the core:
                // whatever value we store here is echoed back to
                // `visit_dynamic_children` / `get_subtree_range` /
                // `get_subtree`. Use the flat tree index of this node so
                // those hooks can look up `dynamic_table` directly, rather
                // than the Rust-codegen convention of a global repeater
                // index that's unique across the sub-component tree.
                ItemTreeNode::DynamicTree { index: out.len() as u32, parent_index },
                Some((
                    node.sub_component_path.clone().into_boxed_slice(),
                    (dynamic_index as usize).into(),
                )),
                None,
            ),
        };
        out.push(entry);
        dyn_table.push(dyn_entry);
        item_table.push(item_entry);
    });
    (out, dyn_table, item_table)
}

/// Lets [`Instance`] be used inside a `Repeater<C>`.
///
/// `update(idx, data)` writes the repeater's `index_prop` and `data_prop` on
/// the repeated instance's root sub-component, matching what the Rust code
/// generator does in the `RepeatedItemTree` impl it synthesizes.
impl i_slint_core::model::RepeatedItemTree for Instance {
    type Data = crate::Value;

    fn update(&self, index: usize, data: Self::Data) {
        let sc_idx = self.root_sub_component.sub_component_idx;
        let cu = self.root_sub_component.compilation_unit.clone();
        let sc = &cu.sub_components[sc_idx];
        // `lower_sub_component` pushes `model_data` and `model_index` as the
        // first two properties of a repeated component's root sub-component.
        // Walk the full property list so user-declared `index` / `model-data`
        // shadows don't accidentally collide with slot 0/1.
        for (idx, prop) in sc.properties.iter_enumerated() {
            let target = &self.root_sub_component.properties[idx];
            match prop.name.as_str() {
                "model_data" => Pin::as_ref(target).set(data.clone()),
                "model_index" => Pin::as_ref(target).set(crate::Value::Number(index as f64)),
                _ => {}
            }
        }
    }

    fn init(&self) {
        // Bindings and init code are installed here rather than in
        // `Instance::new_repeated`: by the time `init` runs,
        // `Repeater::ensure_updated` has released its `RefCell` borrow, so
        // a binding evaluated here can walk back through the same repeater
        // (e.g. an `init` callback that reads a layout property).
        if let Some(weak) = self.self_weak.get()
            && let Some(vrc) = weak.upgrade()
        {
            finalize_instance(&vrc);
        }
    }

    fn listview_layout(
        self: Pin<&Self>,
        offset_y: &mut i_slint_core::lengths::LogicalLength,
    ) -> i_slint_core::lengths::LogicalLength {
        use i_slint_core::item_tree::ItemTree as _;
        use i_slint_core::lengths::LogicalLength;
        // Mirror the rust codegen's per-row layout: write `prop_y` on the
        // repeated row's root sub-component, advance `offset_y` by
        // `prop_height`, and return the row's preferred horizontal layout
        // info width as the new viewport width estimate.
        let this = self.get_ref();
        let Some((parent_weak, rep_idx)) = this.root_sub_component.repeated_in.get() else {
            return LogicalLength::default();
        };
        let Some(parent_sub) = parent_weak.upgrade() else { return LogicalLength::default() };
        let parent_sub = Pin::new(parent_sub);
        let parent_cu = parent_sub.compilation_unit.clone();
        let parent_sc = &parent_cu.sub_components[parent_sub.sub_component_idx];
        let Some(lv) = parent_sc.repeated[*rep_idx].listview.as_ref() else {
            return LogicalLength::default();
        };

        // `prop_y` and `prop_height` are member references in the repeated
        // sub-component's own context, so evaluate them against
        // `this.root_sub_component`.
        let row_sub = this.root_sub_component.clone();
        let ctx = crate::eval::EvalContext::new(row_sub.clone());
        crate::eval::store_property(&ctx, &lv.prop_y, crate::Value::Number(offset_y.get() as f64));
        let height_v = crate::eval::load_property(&ctx, &lv.prop_height);
        let height: f64 = height_v.try_into().unwrap_or(0.0);
        *offset_y += LogicalLength::new(height as f32);
        let info = self.layout_info(i_slint_core::items::Orientation::Horizontal);
        LogicalLength::new(info.min)
    }

    fn layout_item_info(
        self: Pin<&Self>,
        orientation: i_slint_core::items::Orientation,
        child_index: Option<usize>,
    ) -> i_slint_core::layout::LayoutItemInfo {
        // Evaluate the repeated component's `layout_info_h` / `layout_info_v`
        // and wrap the result in a LayoutItemInfo.
        //
        // When the sub-component is a repeated Row with `row_child_templates`,
        // each `child_index` points at one concrete child position. Walk the
        // templates in declaration order and return per-child layout info —
        // static children read `grid_layout_children[idx]`, repeated children
        // forward to the inner repeater instance's own `layout_info`.
        let this = self.get_ref();
        let cu = this.root_sub_component.compilation_unit.clone();
        let sc_idx = this.root_sub_component.sub_component_idx;
        let sc = &cu.sub_components[sc_idx];

        if let (Some(index), true, Some(templates)) =
            (child_index, sc.is_repeated_row, sc.row_child_templates.as_ref())
        {
            return row_child_layout_item_info(this, sc, templates, orientation, index);
        }

        let expr = match orientation {
            i_slint_core::items::Orientation::Horizontal => sc.layout_info_h.borrow(),
            i_slint_core::items::Orientation::Vertical => sc.layout_info_v.borrow(),
        };
        let mut ctx = crate::eval::EvalContext::new(this.root_sub_component.clone());
        let constraint =
            crate::eval::eval_expression(&mut ctx, &expr).try_into().unwrap_or_default();
        i_slint_core::layout::LayoutItemInfo { constraint }
    }

    fn flexbox_layout_item_info(
        self: Pin<&Self>,
        orientation: i_slint_core::items::Orientation,
        child_index: Option<usize>,
    ) -> i_slint_core::layout::FlexboxLayoutItemInfo {
        // For flexbox, the SubComponent stores `flexbox_layout_item_info_for_repeated`
        // - an expression that evaluates to a `FlexboxLayoutItemInfo` struct.
        // Fall back to wrapping `layout_item_info` if it's not set.
        let cu = self.root_sub_component.compilation_unit.clone();
        let sc_idx = self.root_sub_component.sub_component_idx;
        let sc = &cu.sub_components[sc_idx];
        if let Some(expr) = &sc.flexbox_layout_item_info_for_repeated {
            let expr = expr.borrow();
            let mut ctx = crate::eval::EvalContext::new(self.root_sub_component.clone());
            let value = crate::eval::eval_expression(&mut ctx, &expr);
            let mut info = value_to_flexbox_layout_item_info(value, orientation, self);
            // Break the height-for-width recursion for a repeated instance in
            // a column FlexboxLayout: its vertical info must not read
            // self.width (set by the parent flex cache it is feeding). Use the
            // constrained vertical info (computed at the instance's own
            // preferred width) instead, like the generated code does.
            if matches!(orientation, i_slint_core::items::Orientation::Vertical)
                && child_index.is_none()
                && let Some(v_expr) = &sc.layout_info_v_constrained_for_repeated
            {
                let mut ctx = crate::eval::EvalContext::new(self.root_sub_component.clone());
                info.constraint = crate::eval::eval_expression(&mut ctx, &v_expr.borrow())
                    .try_into()
                    .unwrap_or_default();
                return info;
            }
            // The expression leaves the constraint unset; fill it with the
            // layout item's real constraint, like the generated code does.
            info.constraint = self.layout_item_info(orientation, child_index).constraint;
            return info;
        }
        let info = self.layout_item_info(orientation, None);
        info.into()
    }
}

impl Instance {
    /// Vertical flexbox info for a repeated instance measured at the container
    /// cross width instead of its own preferred width, so a height-for-width
    /// cell wraps to the same height as an equivalent static cell. Mirrors the
    /// generated `flexbox_layout_item_info_at_cross_width`.
    pub fn flexbox_layout_item_info_at_cross_width(
        self: Pin<&Self>,
        flex_cross_width: f32,
    ) -> i_slint_core::layout::FlexboxLayoutItemInfo {
        use i_slint_core::items::Orientation;
        use i_slint_core::model::RepeatedItemTree;
        let mut info =
            RepeatedItemTree::flexbox_layout_item_info(self, Orientation::Vertical, None);
        let cu = self.root_sub_component.compilation_unit.clone();
        let sc = &cu.sub_components[self.root_sub_component.sub_component_idx];
        if let Some(v_expr) = &sc.layout_info_v_at_cross_width_for_repeated {
            let mut ctx = crate::eval::EvalContext::new(self.root_sub_component.clone());
            ctx.locals.insert(
                i_slint_compiler::llr::lower_layout_expression::FLEX_CROSS_WIDTH_LOCAL.into(),
                crate::Value::Number(flex_cross_width as f64),
            );
            info.constraint = crate::eval::eval_expression(&mut ctx, &v_expr.borrow())
                .try_into()
                .unwrap_or_default();
        }
        info
    }
}

/// Walk the row_child_templates in declaration order, counting cells, until
/// the target `index` is reached. Static cells read from `grid_layout_children`;
/// a repeated cell forwards to the inner repeater instance's `layout_info`.
fn row_child_layout_item_info(
    this: &Instance,
    sc: &i_slint_compiler::llr::SubComponent,
    templates: &[i_slint_compiler::llr::RowChildTemplateInfo],
    orientation: i_slint_core::items::Orientation,
    mut index: usize,
) -> i_slint_core::layout::LayoutItemInfo {
    use i_slint_compiler::llr::RowChildTemplateInfo;
    use i_slint_core::model::RepeatedItemTree;
    for entry in templates {
        match entry {
            RowChildTemplateInfo::Static { child_index } => {
                if index == 0 {
                    let child = &sc.grid_layout_children[*child_index];
                    let expr = match orientation {
                        i_slint_core::items::Orientation::Horizontal => {
                            child.layout_info_h.borrow()
                        }
                        i_slint_core::items::Orientation::Vertical => child.layout_info_v.borrow(),
                    };
                    let mut ctx = crate::eval::EvalContext::new(this.root_sub_component.clone());
                    let constraint = crate::eval::eval_expression(&mut ctx, &expr)
                        .try_into()
                        .unwrap_or_default();
                    return i_slint_core::layout::LayoutItemInfo { constraint };
                }
                index -= 1;
            }
            RowChildTemplateInfo::Repeated { repeater_index } => {
                let repeater = &this.root_sub_component.repeaters[*repeater_index];
                repeater.track_instance_changes();
                let count = repeater.range().len();
                if index < count {
                    if let Some(inner) = repeater.instance_at(index) {
                        return RepeatedItemTree::layout_item_info(
                            inner.as_pin_ref(),
                            orientation,
                            None,
                        );
                    }
                    return i_slint_core::layout::LayoutItemInfo::default();
                }
                index -= count;
            }
        }
    }
    i_slint_core::layout::LayoutItemInfo::default()
}

fn value_to_flexbox_layout_item_info(
    v: crate::Value,
    orientation: i_slint_core::items::Orientation,
    instance: Pin<&Instance>,
) -> i_slint_core::layout::FlexboxLayoutItemInfo {
    use i_slint_core::model::RepeatedItemTree;
    let crate::Value::Struct(s) = v else {
        let info = RepeatedItemTree::layout_item_info(instance, orientation, None);
        return info.into();
    };
    crate::eval_layout::flexbox_item_info_from_struct(&s)
}
