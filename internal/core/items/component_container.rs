// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

/*!
This module contains the builtin `ComponentContainer` and related items

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/
use super::{Item, ItemConsts, ItemRc, Rectangle, RenderingResult};
use crate::component_factory::ComponentFactory;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::item_tree::{IndexRange, ItemTreeRc, ItemTreeWeak};
use crate::item_tree::{ItemTreeNode, ItemVisitorVTable, TraversalOrder, VisitChildrenResult};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalSize};
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

fn limit_to_constraints(input: LogicalLength, constraint: LayoutInfo) -> LogicalLength {
    let input = input.get();
    LogicalLength::new(if input < constraint.min {
        constraint.min
    } else if input > constraint.max {
        constraint.max
    } else if input >= constraint.min && input <= constraint.max {
        input
    } else {
        constraint.preferred
    })
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `ComponentContainer` element
pub struct ComponentContainer {
    pub component_factory: Property<ComponentFactory>,

    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,

    component_tracker: OnceCell<Pin<Box<PropertyTracker>>>,
    item_tree: RefCell<Option<ItemTreeRc>>,

    my_component: OnceCell<ItemTreeWeak>,
    embedding_item_tree_index: OnceCell<u32>,
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
            // nothing changed!
            return;
        };

        let product = factory.build().and_then(|rc| {
            vtable::VRc::borrow_pin(&rc)
                .as_ref()
                .embed_component(
                    self.my_component.get().unwrap(),
                    *self.embedding_item_tree_index.get().unwrap(),
                )
                .then_some(rc)
        });

        if let Some(rc) = &product {
            // The change resulted in a new component to set up:
            let component = vtable::VRc::borrow_pin(rc);
            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) =
                crate::items::ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
            {
                // Calculate new size for both myself and the embedded window:
                let new_width = limit_to_constraints(
                    window_item.width(),
                    component.as_ref().layout_info(Orientation::Horizontal),
                );
                let new_height = limit_to_constraints(
                    window_item.height(),
                    component.as_ref().layout_info(Orientation::Vertical),
                );

                Property::link_two_way(
                    ComponentContainer::FIELD_OFFSETS.width.apply_pin(self),
                    super::WindowItem::FIELD_OFFSETS.width.apply_pin(window_item),
                );
                Property::link_two_way(
                    ComponentContainer::FIELD_OFFSETS.height.apply_pin(self),
                    super::WindowItem::FIELD_OFFSETS.height.apply_pin(window_item),
                );

                ComponentContainer::FIELD_OFFSETS.width.apply_pin(self).set(new_width);
                ComponentContainer::FIELD_OFFSETS.height.apply_pin(self).set(new_height);
            }
        } else {
            // There change resulted in no component to embed:
            ComponentContainer::FIELD_OFFSETS.width.apply_pin(self).set(Default::default());
            ComponentContainer::FIELD_OFFSETS.height.apply_pin(self).set(Default::default());
        }

        self.item_tree.replace(product);
    }

    pub fn subtree_range(self: Pin<&Self>) -> IndexRange {
        IndexRange { start: 0, end: if self.item_tree.borrow().is_some() { 1 } else { 0 } }
    }

    pub fn subtree_component(self: Pin<&Self>) -> ItemTreeWeak {
        self.item_tree
            .borrow()
            .as_ref()
            .map_or(ItemTreeWeak::default(), |rc| vtable::VRc::downgrade(rc))
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
        let ItemTreeNode::Item { children_index: child_item_tree_index, .. } =
            item_tree[self_rc.index() as usize]
        else {
            panic!("Internal compiler error: ComponentContainer had no child.");
        };

        self.embedding_item_tree_index.set(child_item_tree_index).ok().unwrap();

        self.component_tracker.set(Box::pin(PropertyTracker::default())).ok().unwrap();
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
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
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
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
        if let Some(item_tree) = self.item_tree.borrow().clone() {
            let item_tree = vtable::VRc::borrow_pin(&item_tree);
            let root_item = item_tree.as_ref().get_item_ref(0);
            if let Some(window_item) =
                crate::items::ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
            {
                let mut rect = Rectangle::default();
                rect.background.set(window_item.background());
                backend.draw_rectangle(core::pin::pin!(rect).as_ref(), item_rc, size);
            }
        }

        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for ComponentContainer {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        ComponentContainer,
        CachedRenderingData,
    > = ComponentContainer::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
