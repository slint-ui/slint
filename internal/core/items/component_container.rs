// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains the builtin `ComponentContainer` and related items

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/
use super::{Item, ItemConsts, ItemRc, RenderingResult};
use crate::component_factory::{ComponentFactory, FactoryContext};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::{CachedRenderingData, RenderRectangle};
use crate::item_tree::{IndexRange, ItemTreeRc, ItemTreeWeak, ItemWeak};
use crate::item_tree::{ItemTreeNode, ItemVisitorVTable, TraversalOrder, VisitChildrenResult};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalRect, LogicalSize};
use crate::properties::{Property, PropertyTracker};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use alloc::boxed::Box;
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::RefCell;
use core::pin::Pin;
use i_slint_core_macros::*;
use once_cell::unsync::OnceCell;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `ComponentContainer` element
pub struct ComponentContainer {
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub component_factory: Property<ComponentFactory>,
    pub has_component: Property<bool>,

    pub cached_rendering_data: CachedRenderingData,

    component_tracker: OnceCell<Pin<Box<PropertyTracker>>>,
    item_tree: RefCell<Option<ItemTreeRc>>,

    my_component: OnceCell<ItemTreeWeak>,
    embedding_item_tree_index: OnceCell<u32>,
    self_weak: OnceCell<ItemWeak>,
}

impl ComponentContainer {
    pub fn ensure_updated(self: Pin<&Self>) {
        let factory = self
            .component_tracker
            .get()
            .unwrap()
            .as_ref()
            .evaluate_if_dirty(|| self.component_factory());

        let Some(factory) = factory else {
            return;
        };

        let mut window = None;
        if let Some(parent) = self.my_component.get().and_then(|x| x.upgrade()) {
            vtable::VRc::borrow_pin(&parent).as_ref().window_adapter(false, &mut window);
        }
        let prevent_focus_change =
            window.as_ref().is_some_and(|w| w.window().0.prevent_focus_change.replace(true));

        let factory_context = FactoryContext {
            parent_item_tree: self.my_component.get().unwrap().clone(),
            parent_item_tree_index: *self.embedding_item_tree_index.get().unwrap(),
        };

        let product = factory.build(factory_context);

        if let Some(w) = window {
            w.window().0.prevent_focus_change.set(prevent_focus_change);
        }

        if let Some(item_tree) = product.clone() {
            let item_tree = vtable::VRc::borrow_pin(&item_tree);
            let root_item = item_tree.as_ref().get_item_ref(0);
            if let Some(window_item) =
                crate::items::ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
            {
                // Do _not_ use a two-way binding: That causes evaluations of width and height to
                // assert on recursive property evaluation.
                let weak = self.self_weak.get().unwrap().clone();
                window_item.width.set_binding(Box::new(move || {
                    if let Some(self_rc) = weak.upgrade() {
                        let self_pin = self_rc.borrow();
                        if let Some(self_cc) = crate::items::ItemRef::downcast_pin::<Self>(self_pin)
                        {
                            return self_cc.width();
                        }
                    }
                    Default::default()
                }));
                let weak = self.self_weak.get().unwrap().clone();
                window_item.height.set_binding(Box::new(move || {
                    if let Some(self_rc) = weak.upgrade() {
                        let self_pin = self_rc.borrow();
                        if let Some(self_cc) = crate::items::ItemRef::downcast_pin::<Self>(self_pin)
                        {
                            return self_cc.height();
                        }
                    }
                    Default::default()
                }));
            }
        }

        self.has_component.set(product.is_some());

        self.item_tree.replace(product);
    }

    pub fn subtree_range(self: Pin<&Self>) -> IndexRange {
        IndexRange { start: 0, end: if self.item_tree.borrow().is_some() { 1 } else { 0 } }
    }

    pub fn subtree_component(self: Pin<&Self>) -> ItemTreeWeak {
        self.item_tree.borrow().as_ref().map_or(ItemTreeWeak::default(), vtable::VRc::downgrade)
    }

    pub fn visit_children_item(
        self: Pin<&Self>,
        _index: isize,
        order: TraversalOrder,
        visitor: vtable::VRefMut<ItemVisitorVTable>,
    ) -> VisitChildrenResult {
        let rc = self.item_tree.borrow().clone();
        if let Some(rc) = &rc {
            vtable::VRc::borrow_pin(rc).as_ref().visit_children_item(-1, order, visitor)
        } else {
            VisitChildrenResult::CONTINUE
        }
    }
}

impl Item for ComponentContainer {
    fn init(self: Pin<&Self>, self_rc: &ItemRc) {
        let rc = self_rc.item_tree();

        self.my_component.set(vtable::VRc::downgrade(rc)).ok().unwrap();

        // Find my embedding item_tree_index:
        let pin_rc = vtable::VRc::borrow_pin(rc);
        let item_tree = pin_rc.as_ref().get_item_tree();
        let ItemTreeNode::Item { children_index, children_count, .. } =
            item_tree[self_rc.index() as usize]
        else {
            panic!("ComponentContainer not found in item tree");
        };

        assert_eq!(children_count, 1);
        assert!(matches!(item_tree[children_index as usize], ItemTreeNode::DynamicTree { .. }));

        self.embedding_item_tree_index.set(children_index).ok().unwrap();

        self.component_tracker.set(Box::pin(PropertyTracker::default())).ok().unwrap();
        self.self_weak.set(self_rc.downgrade()).ok().unwrap();
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        self.ensure_updated();
        if let Some(rc) = self.item_tree.borrow().clone() {
            vtable::VRc::borrow_pin(&rc).as_ref().layout_info(orientation)
        } else {
            Default::default()
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut super::ItemRendererRef,
        item_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        backend.draw_rectangle(self, item_rc, size, &self.cached_rendering_data);
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl RenderRectangle for ComponentContainer {
    fn background(self: Pin<&Self>) -> crate::Brush {
        self.item_tree
            .borrow()
            .clone()
            .and_then(|item_tree| {
                let item_tree = vtable::VRc::borrow_pin(&item_tree);
                let root_item = item_tree.as_ref().get_item_ref(0);
                crate::items::ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
                    .map(|window_item| window_item.background())
            })
            .unwrap_or_default()
    }
}

impl ItemConsts for ComponentContainer {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        ComponentContainer,
        CachedRenderingData,
    > = ComponentContainer::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
