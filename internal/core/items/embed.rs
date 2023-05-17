// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains the builtin Embed related items

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/
use super::{Item, ItemConsts, ItemRc, RenderingResult};
use crate::component::{ComponentRc, ComponentWeak};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::{CachedRenderingData, ItemRenderer};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
use crate::properties::PropertyTracker;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::Property;
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::RefCell;
use core::pin::Pin;
use i_slint_core_macros::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `Image` element
pub struct Embed {
    pub component_factory: Property<Option<ComponentWeak>>,

    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,

    pub cached_rendering_data: CachedRenderingData,

    component_tracker: RefCell<Option<Pin<Box<PropertyTracker>>>>,
    component_rc: RefCell<Option<ComponentRc>>,
}

impl Embed {
    pub fn ensure_updated(self: Pin<&Self>, component: &ComponentWeak, index: usize) {
        let is_dirty =
            self.component_tracker.borrow().as_ref().map(|t| t.is_dirty()).unwrap_or(true);
        if is_dirty {
            self.update(component, index);
        }
    }

    fn update(self: Pin<&Self>, component: &ComponentWeak, index: usize) {
        eprintln!("Embed::update!");
        let t =
            self.component_tracker.take().unwrap_or_else(|| Box::pin(PropertyTracker::default()));
        t.as_ref().evaluate_as_dependency_root(|| self.component_factory());
        self.component_tracker.replace(Some(t));

        if let Some(weak) = self.component_factory() {
            let rc = weak.upgrade();
            if let Some(rc) = &rc {
                vtable::VRc::borrow_pin(rc).as_ref().set_parent_node(Some(component), index);
            }
            self.component_rc.replace(rc);
        } else {
            self.component_rc.replace(None);
        }
    }

    pub fn component_rc(self: Pin<&Self>) -> Option<ComponentRc> {
        self.component_rc.borrow().clone()
    }
}

impl Item for Embed {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

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
        _backend: &mut &mut dyn ItemRenderer,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for Embed {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        Embed,
        CachedRenderingData,
    > = Embed::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
