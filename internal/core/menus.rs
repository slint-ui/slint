// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// for MenuVTable_static
#![allow(unsafe_code)]

use crate::item_rendering::CachedRenderingData;
use crate::item_tree::{ItemTreeRc, ItemWeak, VisitChildrenResult};
use crate::items::{ItemRc, ItemRef, MenuEntry, VoidArg};
use crate::properties::PropertyTracker;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::string::ToSharedString;
use crate::window::WindowAdapter;
use crate::{Callback, Property, SharedString, SharedVector};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use i_slint_core_macros::SlintElement;
use vtable::{VRef, VRefMut};

/// Interface for native menu and menubar
#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
#[vtable::vtable]
#[repr(C)]
pub struct MenuVTable {
    /// destructor
    drop: extern "C" fn(VRefMut<MenuVTable>),
    /// Return the list of items for the sub menu (or the main menu of parent is None)
    sub_menu: extern "C" fn(VRef<MenuVTable>, Option<&MenuEntry>, &mut SharedVector<MenuEntry>),
    /// Handler when the menu entry is activated
    activate: extern "C" fn(VRef<MenuVTable>, &MenuEntry),
}

struct ShadowTreeNode {
    item: ItemWeak,
    children: SharedVector<MenuEntry>,
}

pub struct MenuFromItemTree {
    item_tree: ItemTreeRc,
    item_cache: RefCell<BTreeMap<SharedString, ShadowTreeNode>>,
    root: RefCell<SharedVector<MenuEntry>>,
    next_id: Cell<usize>,
    tracker: Pin<Box<PropertyTracker>>,
}

impl MenuFromItemTree {
    pub fn new(item_tree: ItemTreeRc) -> Self {
        Self {
            item_tree,
            item_cache: Default::default(),
            root: Default::default(),
            tracker: Box::pin(PropertyTracker::default()),
            next_id: 0.into(),
        }
    }

    fn update_shadow_tree(&self) {
        self.tracker.as_ref().evaluate_if_dirty(|| {
            self.item_cache.replace(Default::default());
            self.root
                .replace(self.update_shadow_tree_recursive(&ItemRc::new(self.item_tree.clone(), 0)))
        });
    }

    fn update_shadow_tree_recursive(&self, parent: &ItemRc) -> SharedVector<MenuEntry> {
        let mut result = SharedVector::default();
        let mut last_is_separator = false;

        let mut actual_visitor =
            |item_tree: &ItemTreeRc, index: u32, item_pin: Pin<ItemRef>| -> VisitChildrenResult {
                if let Some(menu_item) = ItemRef::downcast_pin::<MenuItem>(item_pin) {
                    let title = menu_item.title();
                    let is_separator =
                        title.as_str() == i_slint_common::MENU_SEPARATOR_PLACEHOLDER_TITLE;
                    if is_separator && (last_is_separator || result.is_empty()) {
                        return VisitChildrenResult::CONTINUE;
                    }
                    last_is_separator = is_separator;
                    let next_id = self.next_id.get();
                    self.next_id.set(next_id + 1);
                    let id = next_id.to_shared_string();
                    let item = ItemRc::new(item_tree.clone(), index);
                    let children = self.update_shadow_tree_recursive(&item);
                    let has_sub_menu = !children.is_empty();
                    let enabled = menu_item.enabled();
                    self.item_cache.borrow_mut().insert(
                        id.clone(),
                        ShadowTreeNode { item: ItemRc::downgrade(&item), children },
                    );
                    result.push(MenuEntry { title, id, has_sub_menu, is_separator, enabled });
                }
                VisitChildrenResult::CONTINUE
            };
        vtable::new_vref!(let mut actual_visitor : VRefMut<crate::item_tree::ItemVisitorVTable> for crate::item_tree::ItemVisitor = &mut actual_visitor);

        vtable::VRc::borrow_pin(parent.item_tree()).as_ref().visit_children_item(
            parent.index() as isize,
            crate::item_tree::TraversalOrder::BackToFront,
            actual_visitor,
        );
        if last_is_separator {
            result.pop();
        }
        result
    }
}

impl Menu for MenuFromItemTree {
    fn sub_menu(&self, parent: Option<&MenuEntry>, result: &mut SharedVector<MenuEntry>) {
        self.update_shadow_tree();
        match parent {
            Some(parent) => {
                if let Some(r) = self.item_cache.borrow().get(parent.id.as_str()) {
                    *result = r.children.clone();
                }
            }
            None => {
                *result = self.root.borrow().clone();
            }
        }
    }

    fn activate(&self, entry: &MenuEntry) {
        if let Some(menu_item) =
            self.item_cache.borrow().get(entry.id.as_str()).and_then(|e| e.item.upgrade())
        {
            if let Some(menu_item) = menu_item.downcast::<MenuItem>() {
                menu_item.activated.call(&());
            }
        }
    }
}

MenuVTable_static!(static MENU_FROM_ITEM_TREE_VT for MenuFromItemTree);

#[repr(C)]
#[derive(const_field_offset::FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of an MenuItem items that does nothing
pub struct MenuItem {
    pub cached_rendering_data: CachedRenderingData,
    pub title: Property<SharedString>,
    pub activated: Callback<VoidArg>,
    pub enabled: Property<bool>,
}

impl crate::items::Item for MenuItem {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: crate::items::Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::layout::LayoutInfo {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: crate::input::MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::input::InputEventFilterResult {
        Default::default()
    }

    fn input_event(
        self: Pin<&Self>,
        _: crate::input::MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::input::InputEventResult {
        Default::default()
    }

    fn key_event(
        self: Pin<&Self>,
        _: &crate::input::KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::input::KeyEventResult {
        Default::default()
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &crate::input::FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::input::FocusEventResult {
        Default::default()
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut &mut dyn crate::item_rendering::ItemRenderer,
        _self_rc: &ItemRc,
        _size: crate::lengths::LogicalSize,
    ) -> crate::items::RenderingResult {
        Default::default()
    }

    fn bounding_rect(
        self: Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: crate::lengths::LogicalRect,
    ) -> crate::lengths::LogicalRect {
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl crate::items::ItemConsts for MenuItem {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        MenuItem,
        CachedRenderingData,
    > = MenuItem::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

#[cfg(feature = "ffi")]
pub mod ffi {
    use super::*;

    /// Create a `VBox::<MenuVTable>`` that wraps the [`ItemTreeRc`]
    ///
    /// Put the created VBox into the result pointer with std::ptr::write
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_menus_create_wrapper(
        menu_tree: &ItemTreeRc,
        result: *mut vtable::VBox<MenuVTable>,
    ) {
        let b = vtable::VBox::<MenuVTable>::new(MenuFromItemTree::new(menu_tree.clone()));
        core::ptr::write(result, b);
    }
}
