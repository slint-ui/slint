// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

/*!
This module contains the builtin `ComponentContainer` and related items

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/
use super::{Item, ItemConsts, ItemRc, RenderingResult};
use crate::component::ComponentWeak;
use crate::component_factory::ComponentFactory;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::item_tree::ItemTreeNode;
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
use crate::properties::Property;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;
use once_cell::unsync::OnceCell;

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

    my_component: OnceCell<ComponentWeak>,
    embedding_item_tree_index: OnceCell<usize>,
}

impl Item for ComponentContainer {
    fn init(self: Pin<&Self>, self_rc: &ItemRc) {
        let rc = self_rc.component();

        self.my_component.set(vtable::VRc::downgrade(rc)).ok().unwrap();

        // Find my embedding item_tree_index:
        let pin_rc = vtable::VRc::borrow_pin(rc);
        let item_tree = pin_rc.as_ref().get_item_tree();
        let ItemTreeNode::Item { children_index: child_item_tree_index, .. } =
            item_tree[self_rc.index()]
        else {
            panic!("Internal compiler error: ComponentContainer had no child.");
        };

        self.embedding_item_tree_index.set(child_item_tree_index as usize).ok().unwrap();
    }

    fn geometry(self: Pin<&Self>) -> LogicalRect {
        LogicalRect::new(
            LogicalPoint::from_lengths(self.x(), self.y()),
            LogicalSize::from_lengths(self.width(), self.height()),
        )
    }

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { ..Default::default() }
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
        _backend: &mut super::ItemRendererRef,
        _item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for ComponentContainer {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        ComponentContainer,
        CachedRenderingData,
    > = ComponentContainer::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
