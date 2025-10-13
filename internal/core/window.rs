// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore backtab

#![warn(missing_docs)]
//! Exposed Window API

use crate::api::{
    CloseRequestResponse, LogicalPosition, PhysicalPosition, PhysicalSize, PlatformError, Window,
    WindowPosition, WindowSize,
};
use crate::input::{
    key_codes, ClickState, FocusEvent, FocusReason, InternalKeyboardModifierState, KeyEvent,
    KeyEventType, MouseEvent, MouseInputState, PointerEventButton, TextCursorBlinker,
};
use crate::item_tree::{
    ItemRc, ItemTreeRc, ItemTreeRef, ItemTreeVTable, ItemTreeWeak, ItemWeak,
    ParentItemTraversalMode,
};
use crate::items::{ColorScheme, InputType, ItemRef, MouseCursor, PopupClosePolicy};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, SizeLengths};
use crate::menus::MenuVTable;
use crate::properties::{Property, PropertyTracker};
use crate::renderer::Renderer;
use crate::{Callback, SharedString};
use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::num::NonZeroU32;
use core::pin::Pin;
use euclid::num::Zero;
use vtable::VRcMapped;

pub mod popup;

fn next_focus_item(item: ItemRc) -> ItemRc {
    item.next_focus_item()
}

fn previous_focus_item(item: ItemRc) -> ItemRc {
    item.previous_focus_item()
}

/// This trait represents the adaptation layer between the [`Window`] API and then
/// windowing specific window representation, such as a Win32 `HWND` handle or a `wayland_surface_t`.
///
/// Implement this trait to establish the link between the two, and pass messages in both
/// directions:
///
/// - When receiving messages from the windowing system about state changes, such as the window being resized,
///   the user requested the window to be closed, input being received, etc. you need to create a
///   [`WindowEvent`](crate::platform::WindowEvent) and send it to Slint via [`Window::try_dispatch_event()`].
///
/// - Slint sends requests to change visibility, position, size, etc. via functions such as [`Self::set_visible`],
///   [`Self::set_size`], [`Self::set_position`], or [`Self::update_window_properties()`]. Re-implement these functions
///   and delegate the requests to the windowing system.
///
/// If the implementation of this bi-directional message passing protocol is incomplete, the user may
/// experience unexpected behavior, or the intention of the developer calling functions on the [`Window`]
/// API may not be fulfilled.
///
/// Your implementation must hold a renderer, such as [`SoftwareRenderer`](crate::software_renderer::SoftwareRenderer).
/// In the [`Self::renderer()`] function, you must return a reference to it.
///
/// It is also required to hold a [`Window`] and return a reference to it in your
/// implementation of [`Self::window()`].
///
/// See also [`MinimalSoftwareWindow`](crate::software_renderer::MinimalSoftwareWindow)
/// for a minimal implementation of this trait using the software renderer
pub trait WindowAdapter {
    /// Returns the window API.
    fn window(&self) -> &Window;

    /// Show the window if the argument is true, hide otherwise.
    fn set_visible(&self, _visible: bool) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    ///
    /// The default implementation returns `None`
    ///
    /// Called from [`Window::position()`]
    fn position(&self) -> Option<PhysicalPosition> {
        None
    }
    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    ///
    /// The default implementation does nothing
    ///
    /// Called from [`Window::set_position()`]
    fn set_position(&self, _position: WindowPosition) {}

    /// Request a new size for the window to the specified size on the screen, in physical or logical pixels
    /// and excluding a window frame (if present).
    ///
    /// This is called from [`Window::set_size()`]
    ///
    /// The default implementation does nothing
    ///
    /// This function should sent the size to the Windowing system. If the window size actually changes, you
    /// should dispatch a [`WindowEvent::Resized`](crate::platform::WindowEvent::Resized) using
    /// [`Window::dispatch_event()`] to propagate the new size to the slint view
    fn set_size(&self, _size: WindowSize) {}

    /// Return the size of the Window on the screen
    fn size(&self) -> PhysicalSize;

    /// Issues a request to the windowing system to re-render the contents of the window.
    ///
    /// This request is typically asynchronous.
    /// It is called when a property that was used during window rendering is marked as dirty.
    ///
    /// An implementation should repaint the window in a subsequent iteration of the event loop,
    /// throttled to the screen refresh rate if possible.
    /// It is important not to query any Slint properties to avoid introducing a dependency loop in the properties,
    /// including the use of the render function, which itself queries properties.
    ///
    /// See also [`Window::request_redraw()`]
    fn request_redraw(&self) {}

    /// Return the renderer.
    ///
    /// The `Renderer` trait is an internal trait that you are not expected to implement.
    /// In your implementation you should return a reference to an instance of one of the renderers provided by Slint.
    ///
    /// Currently, the only public struct that implement renderer is [`SoftwareRenderer`](crate::software_renderer::SoftwareRenderer).
    fn renderer(&self) -> &dyn Renderer;

    /// Re-implement this function to update the properties such as window title or layout constraints.
    ///
    /// This function is called before `set_visible(true)`, and will be called again when the properties
    /// that were queried on the last call are changed. If you do not query any properties, it may not
    /// be called again.
    fn update_window_properties(&self, _properties: WindowProperties<'_>) {}

    #[doc(hidden)]
    fn internal(&self, _: crate::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        None
    }

    /// Re-implement this to support exposing raw window handles (version 0.6).
    #[cfg(feature = "raw-window-handle-06")]
    fn window_handle_06(
        &self,
    ) -> Result<raw_window_handle_06::WindowHandle<'_>, raw_window_handle_06::HandleError> {
        Err(raw_window_handle_06::HandleError::NotSupported)
    }

    /// Re-implement this to support exposing raw display handles (version 0.6).
    #[cfg(feature = "raw-window-handle-06")]
    fn display_handle_06(
        &self,
    ) -> Result<raw_window_handle_06::DisplayHandle<'_>, raw_window_handle_06::HandleError> {
        Err(raw_window_handle_06::HandleError::NotSupported)
    }
}

/// Implementation details behind [`WindowAdapter`], but since this
/// trait is not exported in the public API, it is not possible for the
/// users to call or re-implement these functions.
// TODO: add events for window receiving and loosing focus
#[doc(hidden)]
pub trait WindowAdapterInternal {
    /// This function is called by the generated code when a component and therefore its tree of items are created.
    fn register_item_tree(&self) {}

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources.
    fn unregister_item_tree(
        &self,
        _component: ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>,
    ) {
    }

    /// Create a window for a popup.
    ///
    /// `geometry` is the location of the popup in the window coordinate
    ///
    /// If this function return None (the default implementation), then the
    /// popup will be rendered within the window itself.
    fn create_popup(&self, _geometry: LogicalRect) -> Option<Rc<dyn WindowAdapter>> {
        None
    }

    /// Set the mouse cursor
    // TODO: Make the enum public and make public
    fn set_mouse_cursor(&self, _cursor: MouseCursor) {}

    /// This method allow editable input field to communicate with the platform about input methods
    fn input_method_request(&self, _: InputMethodRequest) {}

    /// Return self as any so the backend can upcast
    // TODO: consider using the as_any crate, or deriving the trait from Any to provide a better default
    fn as_any(&self) -> &dyn core::any::Any {
        &()
    }

    /// Handle focus change
    // used for accessibility
    fn handle_focus_change(&self, _old: Option<ItemRc>, _new: Option<ItemRc>) {}

    /// returns the color scheme used
    fn color_scheme(&self) -> ColorScheme {
        ColorScheme::Unknown
    }

    /// Returns whether we can have a native menu bar
    fn supports_native_menu_bar(&self) -> bool {
        false
    }

    fn setup_menubar(&self, _menubar: vtable::VRc<MenuVTable>) {}

    fn show_native_popup_menu(
        &self,
        _context_menu_item: vtable::VRc<MenuVTable>,
        _position: LogicalPosition,
    ) -> bool {
        false
    }

    /// Re-implement this to support exposing raw window handles (version 0.6).
    #[cfg(all(feature = "std", feature = "raw-window-handle-06"))]
    fn window_handle_06_rc(
        &self,
    ) -> Result<
        std::sync::Arc<dyn raw_window_handle_06::HasWindowHandle>,
        raw_window_handle_06::HandleError,
    > {
        Err(raw_window_handle_06::HandleError::NotSupported)
    }

    /// Re-implement this to support exposing raw display handles (version 0.6).
    #[cfg(all(feature = "std", feature = "raw-window-handle-06"))]
    fn display_handle_06_rc(
        &self,
    ) -> Result<
        std::sync::Arc<dyn raw_window_handle_06::HasDisplayHandle>,
        raw_window_handle_06::HandleError,
    > {
        Err(raw_window_handle_06::HandleError::NotSupported)
    }

    /// Brings the window to the front and focuses it.
    fn bring_to_front(&self) -> Result<(), PlatformError> {
        Ok(())
    }
}

/// This is the parameter from [`WindowAdapterInternal::input_method_request()`] which lets the editable text input field
/// communicate with the platform about input methods.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum InputMethodRequest {
    /// Enables the input method with the specified properties.
    Enable(InputMethodProperties),
    /// Updates the input method with new properties.
    Update(InputMethodProperties),
    /// Disables the input method.
    Disable,
}

/// This struct holds properties related to an input method.
#[non_exhaustive]
#[derive(Clone, Default, Debug)]
pub struct InputMethodProperties {
    /// The text surrounding the cursor.
    ///
    /// This field does not include pre-edit text or composition.
    pub text: SharedString,
    /// The position of the cursor in bytes within the `text`.
    pub cursor_position: usize,
    /// When there is a selection, this is the position of the second anchor
    /// for the beginning (or the end) of the selection.
    pub anchor_position: Option<usize>,
    /// The current value of the pre-edit text as known by the input method.
    /// This is the text currently being edited but not yet committed.
    /// When empty, there is no pre-edit text.
    pub preedit_text: SharedString,
    /// When the `preedit_text` is not empty, this is the offset of the pre-edit within the `text`.
    pub preedit_offset: usize,
    /// The top-left corner of the cursor rectangle in window coordinates.
    pub cursor_rect_origin: LogicalPosition,
    /// The size of the cursor rectangle.
    pub cursor_rect_size: crate::api::LogicalSize,
    /// The position of the anchor (bottom). Only meaningful if anchor_position is Some
    pub anchor_point: LogicalPosition,
    /// The type of input for the text edit.
    pub input_type: InputType,
    /// The clip rect in window coordinates
    pub clip_rect: Option<LogicalRect>,
}

/// This struct describes layout constraints of a resizable element, such as a window.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct LayoutConstraints {
    /// The minimum size.
    pub min: Option<crate::api::LogicalSize>,
    /// The maximum size.
    pub max: Option<crate::api::LogicalSize>,
    /// The preferred size.
    pub preferred: crate::api::LogicalSize,
}

/// This struct contains getters that provide access to properties of the `Window`
/// element, and is used with [`WindowAdapter::update_window_properties`].
pub struct WindowProperties<'a>(&'a WindowInner);

impl WindowProperties<'_> {
    /// Returns the Window's title
    pub fn title(&self) -> SharedString {
        self.0.window_item().map(|w| w.as_pin_ref().title()).unwrap_or_default()
    }

    /// The background color or brush of the Window
    pub fn background(&self) -> crate::Brush {
        self.0
            .window_item()
            .map(|w: VRcMapped<ItemTreeVTable, crate::items::WindowItem>| {
                w.as_pin_ref().background()
            })
            .unwrap_or_default()
    }

    /// Returns the layout constraints of the window
    pub fn layout_constraints(&self) -> LayoutConstraints {
        let component = self.0.component();
        let component = ItemTreeRc::borrow_pin(&component);
        let h = component.as_ref().layout_info(crate::layout::Orientation::Horizontal);
        let v = component.as_ref().layout_info(crate::layout::Orientation::Vertical);
        let (min, max) = crate::layout::min_max_size_for_layout_constraints(h, v);
        LayoutConstraints {
            min,
            max,
            preferred: crate::api::LogicalSize::new(
                h.preferred_bounded() as f32,
                v.preferred_bounded() as f32,
            ),
        }
    }

    /// Returns true if the window should be shown fullscreen; false otherwise.
    #[deprecated(note = "Please use `is_fullscreen` instead")]
    pub fn fullscreen(&self) -> bool {
        self.is_fullscreen()
    }

    /// Returns true if the window should be shown fullscreen; false otherwise.
    pub fn is_fullscreen(&self) -> bool {
        self.0.is_fullscreen()
    }

    /// true if the window is in a maximized state, otherwise false
    pub fn is_maximized(&self) -> bool {
        self.0.maximized.get()
    }

    /// true if the window is in a minimized state, otherwise false
    pub fn is_minimized(&self) -> bool {
        self.0.minimized.get()
    }
}

struct WindowPropertiesTracker {
    window_adapter_weak: Weak<dyn WindowAdapter>,
}

impl crate::properties::PropertyDirtyHandler for WindowPropertiesTracker {
    fn notify(self: Pin<&Self>) {
        let win = self.window_adapter_weak.clone();
        crate::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                WindowInner::from_pub(window_adapter.window()).update_window_properties();
            };
        })
    }
}

struct WindowRedrawTracker {
    window_adapter_weak: Weak<dyn WindowAdapter>,
}

impl crate::properties::PropertyDirtyHandler for WindowRedrawTracker {
    fn notify(self: Pin<&Self>) {
        if let Some(window_adapter) = self.window_adapter_weak.upgrade() {
            window_adapter.request_redraw();
        };
    }
}

/// This enum describes the different ways a popup can be rendered by the back-end.
#[derive(Clone)]
pub enum PopupWindowLocation {
    /// The popup is rendered in its own top-level window that is know to the windowing system.
    TopLevel(Rc<dyn WindowAdapter>),
    /// The popup is rendered as an embedded child window at the given position.
    ChildWindow(LogicalPoint),
}

/// This structure defines a graphical element that is designed to pop up from the surrounding
/// UI content, for example to show a context menu.
#[derive(Clone)]
pub struct PopupWindow {
    /// The ID of the associated popup.
    pub popup_id: NonZeroU32,
    /// The location defines where the pop up is rendered.
    pub location: PopupWindowLocation,
    /// The component that is responsible for providing the popup content.
    pub component: ItemTreeRc,
    /// Defines the close behaviour of the popup.
    pub close_policy: PopupClosePolicy,
    /// the item that had the focus in the parent window when the popup was opened
    focus_item_in_parent: ItemWeak,
    /// The item from where the Popup was invoked from
    pub parent_item: ItemWeak,
    /// Whether the popup is a popup menu.
    /// Popup menu allow the mouse event to be propagated on their parent menu/menubar
    is_menu: bool,
}

#[pin_project::pin_project]
struct WindowPinnedFields {
    #[pin]
    redraw_tracker: PropertyTracker<WindowRedrawTracker>,
    /// Gets dirty when the layout restrictions, or some other property of the windows change
    #[pin]
    window_properties_tracker: PropertyTracker<WindowPropertiesTracker>,
    #[pin]
    scale_factor: Property<f32>,
    #[pin]
    active: Property<bool>,
    #[pin]
    text_input_focused: Property<bool>,
}

/// Inner datastructure for the [`crate::api::Window`]
pub struct WindowInner {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    component: RefCell<ItemTreeWeak>,
    /// When the window is visible, keep a strong reference
    strong_component_ref: RefCell<Option<ItemTreeRc>>,
    mouse_input_state: Cell<MouseInputState>,
    pub(crate) modifiers: Cell<InternalKeyboardModifierState>,

    /// ItemRC that currently have the focus (possibly an instance of TextInput)
    pub focus_item: RefCell<crate::item_tree::ItemWeak>,
    /// The last text that was sent to the input method
    pub(crate) last_ime_text: RefCell<SharedString>,
    /// Don't let ComponentContainers's instantiation change the focus.
    /// This is a workaround for a recursion when instantiating ComponentContainer because the
    /// init code for the component might have code that sets the focus, but we don't want that
    /// for the ComponentContainer
    pub(crate) prevent_focus_change: Cell<bool>,
    cursor_blinker: RefCell<pin_weak::rc::PinWeak<crate::input::TextCursorBlinker>>,

    pinned_fields: Pin<Box<WindowPinnedFields>>,
    maximized: Cell<bool>,
    minimized: Cell<bool>,

    /// Stack of currently active popups
    active_popups: RefCell<Vec<PopupWindow>>,
    next_popup_id: Cell<NonZeroU32>,
    had_popup_on_press: Cell<bool>,
    close_requested: Callback<(), CloseRequestResponse>,
    click_state: ClickState,
    pub(crate) ctx: once_cell::unsync::Lazy<crate::SlintContext>,
}

impl Drop for WindowInner {
    fn drop(&mut self) {
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }
    }
}

impl WindowInner {
    /// Create a new instance of the window, given the window_adapter factory fn
    pub fn new(window_adapter_weak: Weak<dyn WindowAdapter>) -> Self {
        #![allow(unused_mut)]

        let mut window_properties_tracker =
            PropertyTracker::new_with_dirty_handler(WindowPropertiesTracker {
                window_adapter_weak: window_adapter_weak.clone(),
            });

        let mut redraw_tracker = PropertyTracker::new_with_dirty_handler(WindowRedrawTracker {
            window_adapter_weak: window_adapter_weak.clone(),
        });

        #[cfg(slint_debug_property)]
        {
            window_properties_tracker
                .set_debug_name("i_slint_core::Window::window_properties_tracker".into());
            redraw_tracker.set_debug_name("i_slint_core::Window::redraw_tracker".into());
        }

        Self {
            window_adapter_weak,
            component: Default::default(),
            strong_component_ref: Default::default(),
            mouse_input_state: Default::default(),
            modifiers: Default::default(),
            pinned_fields: Box::pin(WindowPinnedFields {
                redraw_tracker,
                window_properties_tracker,
                scale_factor: Property::new_named(1., "i_slint_core::Window::scale_factor"),
                active: Property::new_named(false, "i_slint_core::Window::active"),
                text_input_focused: Property::new_named(
                    false,
                    "i_slint_core::Window::text_input_focused",
                ),
            }),
            maximized: Cell::new(false),
            minimized: Cell::new(false),
            focus_item: Default::default(),
            last_ime_text: Default::default(),
            cursor_blinker: Default::default(),
            active_popups: Default::default(),
            next_popup_id: Cell::new(NonZeroU32::MIN),
            had_popup_on_press: Default::default(),
            close_requested: Default::default(),
            click_state: ClickState::default(),
            prevent_focus_change: Default::default(),
            // The ctx is lazy so that a Window can be initialized before the backend.
            // (for example in test_empty_window)
            ctx: once_cell::unsync::Lazy::new(|| {
                crate::context::GLOBAL_CONTEXT.with(|ctx| ctx.get().unwrap().clone())
            }),
        }
    }

    /// Associates this window with the specified component. Further event handling and rendering, etc. will be
    /// done with that component.
    pub fn set_component(&self, component: &ItemTreeRc) {
        self.close_all_popups();
        self.focus_item.replace(Default::default());
        self.mouse_input_state.replace(Default::default());
        self.modifiers.replace(Default::default());
        self.component.replace(ItemTreeRc::downgrade(component));
        self.pinned_fields.window_properties_tracker.set_dirty(); // component changed, layout constraints for sure must be re-calculated
        let window_adapter = self.window_adapter();
        window_adapter.renderer().set_window_adapter(&window_adapter);
        self.set_window_item_geometry(
            window_adapter.size().to_logical(self.scale_factor()).to_euclid(),
        );
        window_adapter.request_redraw();
        let weak = Rc::downgrade(&window_adapter);
        crate::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = weak.upgrade() {
                WindowInner::from_pub(window_adapter.window()).update_window_properties();
            }
        })
    }

    /// return the component.
    /// Panics if it wasn't set.
    pub fn component(&self) -> ItemTreeRc {
        self.component.borrow().upgrade().unwrap()
    }

    /// returns the component or None if it isn't set.
    pub fn try_component(&self) -> Option<ItemTreeRc> {
        self.component.borrow().upgrade()
    }

    /// Returns a slice of the active popups.
    pub fn active_popups(&self) -> core::cell::Ref<'_, [PopupWindow]> {
        core::cell::Ref::map(self.active_popups.borrow(), |v| v.as_slice())
    }

    /// Receive a mouse event and pass it to the items of the component to
    /// change their state.
    pub fn process_mouse_input(&self, mut event: MouseEvent) {
        crate::animations::update_animations();

        // handle multiple press release
        event = self.click_state.check_repeat(event, self.ctx.platform().click_interval());

        let window_adapter = self.window_adapter();
        let mut mouse_input_state = self.mouse_input_state.take();
        if let Some(mut drop_event) = mouse_input_state.drag_data.clone() {
            match &event {
                MouseEvent::Released { position, button: PointerEventButton::Left, .. } => {
                    if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
                        window_adapter.set_mouse_cursor(MouseCursor::Default);
                    }
                    drop_event.position = crate::lengths::logical_position_to_api(*position);
                    event = MouseEvent::Drop(drop_event);
                    mouse_input_state.drag_data = None;
                }
                MouseEvent::Moved { position } => {
                    if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
                        window_adapter.set_mouse_cursor(MouseCursor::NoDrop);
                    }
                    drop_event.position = crate::lengths::logical_position_to_api(*position);
                    event = MouseEvent::DragMove(drop_event);
                }
                MouseEvent::Exit => {
                    mouse_input_state.drag_data = None;
                }
                _ => {}
            }
        }

        let pressed_event = matches!(event, MouseEvent::Pressed { .. });
        let released_event = matches!(event, MouseEvent::Released { .. });

        let last_top_item = mouse_input_state.top_item_including_delayed();
        if released_event {
            mouse_input_state =
                crate::input::process_delayed_event(&window_adapter, mouse_input_state);
        }

        let Some(item_tree) = self.try_component() else { return };

        // Try to get the root window in case `self` is the popup itself (to get the active_popups list)
        let mut root_adapter = None;
        ItemTreeRc::borrow_pin(&item_tree).as_ref().window_adapter(false, &mut root_adapter);
        let root_adapter = root_adapter.unwrap_or_else(|| window_adapter.clone());
        let active_popups = &WindowInner::from_pub(root_adapter.window()).active_popups;
        let native_popup_index = active_popups.borrow().iter().position(|p| {
            if let PopupWindowLocation::TopLevel(wa) = &p.location {
                Rc::ptr_eq(wa, &window_adapter)
            } else {
                false
            }
        });

        if pressed_event {
            self.had_popup_on_press.set(!active_popups.borrow().is_empty());
        }

        let mut popup_to_close = active_popups.borrow().last().and_then(|popup| {
            let mouse_inside_popup = || {
                if let PopupWindowLocation::ChildWindow(coordinates) = &popup.location {
                    event.position().is_none_or(|pos| {
                        ItemTreeRc::borrow_pin(&popup.component)
                            .as_ref()
                            .item_geometry(0)
                            .contains(pos - coordinates.to_vector())
                    })
                } else {
                    native_popup_index.is_some_and(|idx| idx == active_popups.borrow().len() - 1)
                        && event.position().is_none_or(|pos| {
                            ItemTreeRc::borrow_pin(&item_tree)
                                .as_ref()
                                .item_geometry(0)
                                .contains(pos)
                        })
                }
            };
            match popup.close_policy {
                PopupClosePolicy::CloseOnClick => {
                    let mouse_inside_popup = mouse_inside_popup();
                    (mouse_inside_popup && released_event && self.had_popup_on_press.get())
                        || (!mouse_inside_popup && pressed_event)
                }
                PopupClosePolicy::CloseOnClickOutside => !mouse_inside_popup() && pressed_event,
                PopupClosePolicy::NoAutoClose => false,
            }
            .then_some(popup.popup_id)
        });

        mouse_input_state = if let Some(mut event) =
            crate::input::handle_mouse_grab(&event, &window_adapter, &mut mouse_input_state)
        {
            let mut item_tree = self.component.borrow().upgrade();
            let mut offset = LogicalPoint::default();
            let mut menubar_item = None;
            for (idx, popup) in active_popups.borrow().iter().enumerate().rev() {
                item_tree = None;
                menubar_item = None;
                if let PopupWindowLocation::ChildWindow(coordinates) = &popup.location {
                    let geom = ItemTreeRc::borrow_pin(&popup.component).as_ref().item_geometry(0);
                    let mouse_inside_popup = event
                        .position()
                        .is_none_or(|pos| geom.contains(pos - coordinates.to_vector()));
                    if mouse_inside_popup {
                        item_tree = Some(popup.component.clone());
                        offset = *coordinates;
                        break;
                    }
                } else if native_popup_index.is_some_and(|i| i == idx) {
                    item_tree = self.component.borrow().upgrade();
                    break;
                }

                if !popup.is_menu {
                    break;
                } else if popup_to_close.is_some() {
                    // clicking outside of a popup menu should close all the menus
                    popup_to_close = Some(popup.popup_id);
                }

                menubar_item = popup.parent_item.upgrade();
            }

            let root = match menubar_item {
                None => item_tree.map(|item_tree| ItemRc::new(item_tree.clone(), 0)),
                Some(menubar_item) => {
                    event.translate(
                        menubar_item
                            .map_to_item_tree(Default::default(), &self.component())
                            .to_vector(),
                    );
                    menubar_item.parent_item(ParentItemTraversalMode::StopAtPopups)
                }
            };

            if let Some(root) = root {
                event.translate(-offset.to_vector());
                let mut new_input_state = crate::input::process_mouse_input(
                    root,
                    &event,
                    &window_adapter,
                    mouse_input_state,
                );
                new_input_state.offset = offset;
                new_input_state
            } else {
                // When outside, send exit event
                let mut new_input_state = MouseInputState::default();
                crate::input::send_exit_events(
                    &mouse_input_state,
                    &mut new_input_state,
                    event.position(),
                    &window_adapter,
                );
                new_input_state
            }
        } else {
            mouse_input_state
        };

        if last_top_item != mouse_input_state.top_item_including_delayed() {
            self.click_state.reset();
            self.click_state.check_repeat(event, self.ctx.platform().click_interval());
        }

        self.mouse_input_state.set(mouse_input_state);

        if let Some(popup_id) = popup_to_close {
            WindowInner::from_pub(root_adapter.window()).close_popup(popup_id);
        }

        crate::properties::ChangeTracker::run_change_handlers();
    }

    /// Called by the input code's internal timer to send an event that was delayed
    pub(crate) fn process_delayed_event(&self) {
        self.mouse_input_state.set(crate::input::process_delayed_event(
            &self.window_adapter(),
            self.mouse_input_state.take(),
        ));
    }

    /// Receive a key event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `event`: The key event received by the windowing system.
    pub fn process_key_input(&self, mut event: KeyEvent) -> crate::input::KeyEventResult {
        if let Some(updated_modifier) = self
            .modifiers
            .get()
            .state_update(event.event_type == KeyEventType::KeyPressed, &event.text)
        {
            // Updates the key modifiers depending on the key code and pressed state.
            self.modifiers.set(updated_modifier);
        }

        event.modifiers = self.modifiers.get().into();

        let mut item = self.focus_item.borrow().clone().upgrade();

        if item.as_ref().is_some_and(|i| !i.is_visible()) {
            // Reset the focus... not great, but better than keeping it.
            self.take_focus_item(&FocusEvent::FocusOut(FocusReason::TabNavigation));
            item = None;
        }

        let item_list = {
            let mut tmp = Vec::new();
            let mut item = item.clone();

            while let Some(i) = item {
                tmp.push(i.clone());
                item = i.parent_item(ParentItemTraversalMode::StopAtPopups);
            }

            tmp
        };

        // Check capture_key_event (going from window to focused item):
        for i in item_list.iter().rev() {
            if i.borrow().as_ref().capture_key_event(&event, &self.window_adapter(), i)
                == crate::input::KeyEventResult::EventAccepted
            {
                crate::properties::ChangeTracker::run_change_handlers();
                return crate::input::KeyEventResult::EventAccepted;
            }
        }

        drop(item_list);

        // Deliver key_event (to focused item, going up towards the window):
        while let Some(focus_item) = item {
            if focus_item.borrow().as_ref().key_event(&event, &self.window_adapter(), &focus_item)
                == crate::input::KeyEventResult::EventAccepted
            {
                crate::properties::ChangeTracker::run_change_handlers();
                return crate::input::KeyEventResult::EventAccepted;
            }
            item = focus_item.parent_item(ParentItemTraversalMode::StopAtPopups);
        }

        // Make Tab/Backtab handle keyboard focus
        let extra_mod = event.modifiers.control || event.modifiers.meta || event.modifiers.alt;
        if event.text.starts_with(key_codes::Tab)
            && !event.modifiers.shift
            && !extra_mod
            && event.event_type == KeyEventType::KeyPressed
        {
            self.focus_next_item();
            crate::properties::ChangeTracker::run_change_handlers();
            return crate::input::KeyEventResult::EventAccepted;
        } else if (event.text.starts_with(key_codes::Backtab)
            || (event.text.starts_with(key_codes::Tab) && event.modifiers.shift))
            && event.event_type == KeyEventType::KeyPressed
            && !extra_mod
        {
            self.focus_previous_item();
            crate::properties::ChangeTracker::run_change_handlers();
            return crate::input::KeyEventResult::EventAccepted;
        } else if event.event_type == KeyEventType::KeyPressed
            && event.text.starts_with(key_codes::Escape)
        {
            // Closes top most popup on ESC key pressed when policy is not no-auto-close

            // Try to get the parent window in case `self` is the popup itself
            let mut adapter = self.window_adapter();
            let item_tree = self.component();
            let mut a = None;
            ItemTreeRc::borrow_pin(&item_tree).as_ref().window_adapter(false, &mut a);
            if let Some(a) = a {
                adapter = a;
            }
            let window = WindowInner::from_pub(adapter.window());

            let close_on_escape = if let Some(popup) = window.active_popups.borrow().last() {
                popup.close_policy == PopupClosePolicy::CloseOnClick
                    || popup.close_policy == PopupClosePolicy::CloseOnClickOutside
            } else {
                false
            };

            if close_on_escape {
                window.close_top_popup();
            }
            crate::properties::ChangeTracker::run_change_handlers();
            return crate::input::KeyEventResult::EventAccepted;
        }
        crate::properties::ChangeTracker::run_change_handlers();
        crate::input::KeyEventResult::EventIgnored
    }

    /// Installs a binding on the specified property that's toggled whenever the text cursor is supposed to be visible or not.
    pub fn set_cursor_blink_binding(&self, prop: &crate::Property<bool>) {
        let existing_blinker = self.cursor_blinker.borrow().clone();

        let blinker = existing_blinker.upgrade().unwrap_or_else(|| {
            let new_blinker = TextCursorBlinker::new();
            *self.cursor_blinker.borrow_mut() =
                pin_weak::rc::PinWeak::downgrade(new_blinker.clone());
            new_blinker
        });

        TextCursorBlinker::set_binding(blinker, prop, self.ctx.platform().cursor_flash_cycle());
    }

    /// Sets the focus to the item pointed to by item_ptr. This will remove the focus from any
    /// currently focused item. If set_focus is false, the focus is cleared.
    pub fn set_focus_item(&self, new_focus_item: &ItemRc, set_focus: bool, reason: FocusReason) {
        if self.prevent_focus_change.get() {
            return;
        }

        let popup_wa = self.active_popups.borrow().last().and_then(|p| match &p.location {
            PopupWindowLocation::TopLevel(wa) => Some(wa.clone()),
            PopupWindowLocation::ChildWindow(..) => None,
        });
        if let Some(popup_wa) = popup_wa {
            // Set the focus item on the popup's Window instead
            popup_wa.window().0.set_focus_item(new_focus_item, set_focus, reason);
            return;
        }

        let current_focus_item = self.focus_item.borrow().clone();
        if let Some(current_focus_item_rc) = current_focus_item.upgrade() {
            if set_focus {
                if current_focus_item_rc == *new_focus_item {
                    // don't send focus out and in even to the same item if focus doesn't change
                    return;
                }
            } else if current_focus_item_rc != *new_focus_item {
                // can't clear focus unless called with currently focused item.
                return;
            }
        }

        let old = self.take_focus_item(&FocusEvent::FocusOut(reason));
        let new = if set_focus {
            self.move_focus(new_focus_item.clone(), next_focus_item, reason)
        } else {
            None
        };
        let window_adapter = self.window_adapter();
        if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
            window_adapter.handle_focus_change(old, new);
        }
    }

    /// Take the focus_item out of this Window
    ///
    /// This sends the event which must be either FocusOut or WindowLostFocus for popups
    fn take_focus_item(&self, event: &FocusEvent) -> Option<ItemRc> {
        let focus_item = self.focus_item.take();
        assert!(matches!(event, FocusEvent::FocusOut(_)));

        if let Some(focus_item_rc) = focus_item.upgrade() {
            focus_item_rc.borrow().as_ref().focus_event(
                event,
                &self.window_adapter(),
                &focus_item_rc,
            );
            Some(focus_item_rc)
        } else {
            None
        }
    }

    /// Publish the new focus_item to this Window and return the FocusEventResult
    ///
    /// This sends a FocusIn event!
    fn publish_focus_item(
        &self,
        item: &Option<ItemRc>,
        reason: FocusReason,
    ) -> crate::input::FocusEventResult {
        match item {
            Some(item) => {
                *self.focus_item.borrow_mut() = item.downgrade();
                item.borrow().as_ref().focus_event(
                    &FocusEvent::FocusIn(reason),
                    &self.window_adapter(),
                    item,
                )
            }
            None => {
                *self.focus_item.borrow_mut() = Default::default();
                crate::input::FocusEventResult::FocusAccepted // We were removing the focus, treat that as OK
            }
        }
    }

    fn move_focus(
        &self,
        start_item: ItemRc,
        forward: impl Fn(ItemRc) -> ItemRc,
        reason: FocusReason,
    ) -> Option<ItemRc> {
        let mut current_item = start_item;
        let mut visited = Vec::new();

        loop {
            if (current_item.is_visible() || reason == FocusReason::Programmatic)
                && self.publish_focus_item(&Some(current_item.clone()), reason)
                    == crate::input::FocusEventResult::FocusAccepted
            {
                return Some(current_item); // Item was just published.
            }
            visited.push(current_item.clone());
            current_item = forward(current_item);

            if visited.contains(&current_item) {
                return None; // Nothing to do: We took the focus_item already
            }
        }
    }

    /// Move keyboard focus to the next item
    pub fn focus_next_item(&self) {
        let start_item = self
            .take_focus_item(&FocusEvent::FocusOut(FocusReason::TabNavigation))
            .map(next_focus_item)
            .unwrap_or_else(|| {
                ItemRc::new(
                    self.active_popups
                        .borrow()
                        .last()
                        .map_or_else(|| self.component(), |p| p.component.clone()),
                    0,
                )
            });
        let end_item =
            self.move_focus(start_item.clone(), next_focus_item, FocusReason::TabNavigation);
        let window_adapter = self.window_adapter();
        if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
            window_adapter.handle_focus_change(Some(start_item), end_item);
        }
    }

    /// Move keyboard focus to the previous item.
    pub fn focus_previous_item(&self) {
        let start_item = previous_focus_item(
            self.take_focus_item(&FocusEvent::FocusOut(FocusReason::TabNavigation)).unwrap_or_else(
                || {
                    ItemRc::new(
                        self.active_popups
                            .borrow()
                            .last()
                            .map_or_else(|| self.component(), |p| p.component.clone()),
                        0,
                    )
                },
            ),
        );
        let end_item =
            self.move_focus(start_item.clone(), previous_focus_item, FocusReason::TabNavigation);
        let window_adapter = self.window_adapter();
        if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
            window_adapter.handle_focus_change(Some(start_item), end_item);
        }
    }

    /// Marks the window to be the active window. This typically coincides with the keyboard
    /// focus. One exception though is when a popup is shown, in which case the window may
    /// remain active but temporarily loose focus to the popup.
    ///
    /// This results in WindowFocusReceived and WindowFocusLost events.
    pub fn set_active(&self, have_focus: bool) {
        self.pinned_fields.as_ref().project_ref().active.set(have_focus);

        let event = if have_focus {
            FocusEvent::FocusIn(FocusReason::WindowActivation)
        } else {
            FocusEvent::FocusOut(FocusReason::WindowActivation)
        };

        if let Some(focus_item) = self.focus_item.borrow().upgrade() {
            focus_item.borrow().as_ref().focus_event(&event, &self.window_adapter(), &focus_item);
        }

        // If we lost focus due to for example a global shortcut, then when we regain focus
        // should not assume that the modifiers are in the same state.
        if !have_focus {
            self.modifiers.take();
        }
    }

    /// Returns true of the window is the active window. That typically implies having the
    /// keyboard focus, except when a popup is shown and temporarily takes the focus.
    pub fn active(&self) -> bool {
        self.pinned_fields.as_ref().project_ref().active.get()
    }

    /// If the component's root item is a Window element, then this function synchronizes its properties, such as the title
    /// for example, with the properties known to the windowing system.
    pub fn update_window_properties(&self) {
        let window_adapter = self.window_adapter();

        // No `if !dirty { return; }` check here because the backend window may be newly mapped and not up-to-date, so force
        // an evaluation.
        self.pinned_fields
            .as_ref()
            .project_ref()
            .window_properties_tracker
            .evaluate_as_dependency_root(|| {
                window_adapter.update_window_properties(WindowProperties(self));
            });
    }

    /// Calls the render_components to render the main component and any sub-window components, tracked by a
    /// property dependency tracker.
    /// Returns None if no component is set yet.
    pub fn draw_contents<T>(
        &self,
        render_components: impl FnOnce(&[(ItemTreeWeak, LogicalPoint)]) -> T,
    ) -> Option<T> {
        let component_weak = ItemTreeRc::downgrade(&self.try_component()?);
        Some(self.pinned_fields.as_ref().project_ref().redraw_tracker.evaluate_as_dependency_root(
            || {
                if !self
                    .active_popups
                    .borrow()
                    .iter()
                    .any(|p| matches!(p.location, PopupWindowLocation::ChildWindow(..)))
                {
                    render_components(&[(component_weak, LogicalPoint::default())])
                } else {
                    let borrow = self.active_popups.borrow();
                    let mut item_trees = Vec::with_capacity(borrow.len() + 1);
                    item_trees.push((component_weak, LogicalPoint::default()));
                    for popup in borrow.iter() {
                        if let PopupWindowLocation::ChildWindow(location) = &popup.location {
                            item_trees.push((ItemTreeRc::downgrade(&popup.component), *location));
                        }
                    }
                    drop(borrow);
                    render_components(&item_trees)
                }
            },
        ))
    }

    /// Registers the window with the windowing system, in order to render the component's items and react
    /// to input events once the event loop spins.
    pub fn show(&self) -> Result<(), PlatformError> {
        if let Some(component) = self.try_component() {
            let was_visible = self.strong_component_ref.replace(Some(component)).is_some();
            if !was_visible {
                *(self.ctx.0.window_count.borrow_mut()) += 1;
            }
        }

        self.update_window_properties();
        self.window_adapter().set_visible(true)?;
        // Make sure that the window's inner size is in sync with the root window item's
        // width/height.
        let size = self.window_adapter().size();
        self.set_window_item_geometry(size.to_logical(self.scale_factor()).to_euclid());
        self.window_adapter().renderer().resize(size).unwrap();
        if let Some(hook) = self.ctx.0.window_shown_hook.borrow_mut().as_mut() {
            hook(&self.window_adapter());
        }
        Ok(())
    }

    /// De-registers the window with the windowing system.
    pub fn hide(&self) -> Result<(), PlatformError> {
        let result = self.window_adapter().set_visible(false);
        let was_visible = self.strong_component_ref.borrow_mut().take().is_some();
        if was_visible {
            let mut count = self.ctx.0.window_count.borrow_mut();
            *count -= 1;
            if *count <= 0 {
                drop(count);
                let _ = self.ctx.event_loop_proxy().and_then(|p| p.quit_event_loop().ok());
            }
        }
        result
    }

    /// returns the color theme used
    pub fn color_scheme(&self) -> ColorScheme {
        self.window_adapter()
            .internal(crate::InternalToken)
            .map_or(ColorScheme::Unknown, |x| x.color_scheme())
    }

    /// Return whether the platform supports native menu bars
    pub fn supports_native_menu_bar(&self) -> bool {
        self.window_adapter()
            .internal(crate::InternalToken)
            .is_some_and(|x| x.supports_native_menu_bar())
    }

    /// Setup the native menu bar
    pub fn setup_menubar(&self, menubar: vtable::VRc<MenuVTable>) {
        if let Some(x) = self.window_adapter().internal(crate::InternalToken) {
            x.setup_menubar(menubar);
        }
    }

    /// Show a popup at the given position relative to the `parent_item` and returns its ID.
    /// The returned ID will always be non-zero.
    /// `is_menu` specifies whether the popup is a popup menu.
    pub fn show_popup(
        &self,
        popup_componentrc: &ItemTreeRc,
        position: LogicalPosition,
        close_policy: PopupClosePolicy,
        parent_item: &ItemRc,
        is_menu: bool,
    ) -> NonZeroU32 {
        let position = parent_item
            .map_to_window(parent_item.geometry().origin + position.to_euclid().to_vector());
        let root_of = |mut item_tree: ItemTreeRc| loop {
            if ItemRc::new(item_tree.clone(), 0).downcast::<crate::items::WindowItem>().is_some() {
                return item_tree;
            }
            let mut r = crate::item_tree::ItemWeak::default();
            ItemTreeRc::borrow_pin(&item_tree).as_ref().parent_node(&mut r);
            match r.upgrade() {
                None => return item_tree,
                Some(x) => item_tree = x.item_tree().clone(),
            }
        };

        let parent_root_item_tree = root_of(parent_item.item_tree().clone());
        let (parent_window_adapter, position) = if let Some(parent_popup) = self
            .active_popups
            .borrow()
            .iter()
            .find(|p| ItemTreeRc::ptr_eq(&p.component, &parent_root_item_tree))
        {
            match &parent_popup.location {
                PopupWindowLocation::TopLevel(wa) => (wa.clone(), position),
                PopupWindowLocation::ChildWindow(offset) => {
                    (self.window_adapter(), position + offset.to_vector())
                }
            }
        } else {
            (self.window_adapter(), position)
        };

        let popup_component = ItemTreeRc::borrow_pin(popup_componentrc);
        let popup_root = popup_component.as_ref().get_item_ref(0);

        let (mut w, mut h) = if let Some(window_item) =
            ItemRef::downcast_pin::<crate::items::WindowItem>(popup_root)
        {
            (window_item.width(), window_item.height())
        } else {
            (LogicalLength::zero(), LogicalLength::zero())
        };

        let layout_info_h =
            popup_component.as_ref().layout_info(crate::layout::Orientation::Horizontal);
        let layout_info_v =
            popup_component.as_ref().layout_info(crate::layout::Orientation::Vertical);

        if w <= LogicalLength::zero() {
            w = LogicalLength::new(layout_info_h.preferred);
        }
        if h <= LogicalLength::zero() {
            h = LogicalLength::new(layout_info_v.preferred);
        }
        w = w.max(LogicalLength::new(layout_info_h.min)).min(LogicalLength::new(layout_info_h.max));
        h = h.max(LogicalLength::new(layout_info_v.min)).min(LogicalLength::new(layout_info_v.max));

        let size = crate::lengths::LogicalSize::from_lengths(w, h);

        if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
            let width_property =
                crate::items::WindowItem::FIELD_OFFSETS.width.apply_pin(window_item);
            let height_property =
                crate::items::WindowItem::FIELD_OFFSETS.height.apply_pin(window_item);
            width_property.set(size.width_length());
            height_property.set(size.height_length());
        };

        let popup_id = self.next_popup_id.get();
        self.next_popup_id.set(self.next_popup_id.get().checked_add(1).unwrap());

        // Close active popups before creating a new one.
        let siblings: Vec<_> = self
            .active_popups
            .borrow()
            .iter()
            .filter(|p| p.parent_item == parent_item.downgrade())
            .map(|p| p.popup_id)
            .collect();

        for sibling in siblings {
            self.close_popup(sibling);
        }

        let location = match parent_window_adapter
            .internal(crate::InternalToken)
            .and_then(|x| x.create_popup(LogicalRect::new(position, size)))
        {
            None => {
                let clip = LogicalRect::new(
                    LogicalPoint::new(0.0 as crate::Coord, 0.0 as crate::Coord),
                    self.window_adapter().size().to_logical(self.scale_factor()).to_euclid(),
                );
                let rect = popup::place_popup(
                    popup::Placement::Fixed(LogicalRect::new(position, size)),
                    &Some(clip),
                );
                self.window_adapter().request_redraw();
                PopupWindowLocation::ChildWindow(rect.origin)
            }
            Some(window_adapter) => {
                WindowInner::from_pub(window_adapter.window()).set_component(popup_componentrc);
                PopupWindowLocation::TopLevel(window_adapter)
            }
        };

        let focus_item = self
            .take_focus_item(&FocusEvent::FocusOut(FocusReason::PopupActivation))
            .map(|item| item.downgrade())
            .unwrap_or_default();

        self.active_popups.borrow_mut().push(PopupWindow {
            popup_id,
            location,
            component: popup_componentrc.clone(),
            close_policy,
            focus_item_in_parent: focus_item,
            parent_item: parent_item.downgrade(),
            is_menu,
        });

        popup_id
    }

    /// Attempt to show a native popup menu
    ///
    /// context_menu_item is an instance of a ContextMenu
    ///
    /// Returns false if the native platform doesn't support it
    pub fn show_native_popup_menu(
        &self,
        context_menu_item: vtable::VRc<MenuVTable>,
        position: LogicalPosition,
        parent_item: &ItemRc,
    ) -> bool {
        if let Some(x) = self.window_adapter().internal(crate::InternalToken) {
            let position = parent_item
                .map_to_window(parent_item.geometry().origin + position.to_euclid().to_vector());
            let position = crate::lengths::logical_position_to_api(position);
            x.show_native_popup_menu(context_menu_item, position)
        } else {
            false
        }
    }

    // Close the popup associated with the given popup window.
    fn close_popup_impl(&self, current_popup: &PopupWindow) {
        match &current_popup.location {
            PopupWindowLocation::ChildWindow(offset) => {
                // Refresh the area that was previously covered by the popup.
                let popup_region = crate::properties::evaluate_no_tracking(|| {
                    let popup_component = ItemTreeRc::borrow_pin(&current_popup.component);
                    popup_component.as_ref().item_geometry(0)
                })
                .translate(offset.to_vector());

                if !popup_region.is_empty() {
                    let window_adapter = self.window_adapter();
                    window_adapter.renderer().mark_dirty_region(popup_region.into());
                    window_adapter.request_redraw();
                }
            }
            PopupWindowLocation::TopLevel(adapter) => {
                let _ = adapter.set_visible(false);
            }
        }
        if let Some(focus) = current_popup.focus_item_in_parent.upgrade() {
            self.set_focus_item(&focus, true, FocusReason::PopupActivation);
        }
    }

    /// Removes the popup matching the given ID.
    pub fn close_popup(&self, popup_id: NonZeroU32) {
        let mut active_popups = self.active_popups.borrow_mut();
        let maybe_index = active_popups.iter().position(|popup| popup.popup_id == popup_id);

        if let Some(popup_index) = maybe_index {
            let p = active_popups.remove(popup_index);
            drop(active_popups);
            self.close_popup_impl(&p);
            if p.is_menu {
                // close all sub-menus
                while self.active_popups.borrow().get(popup_index).is_some_and(|p| p.is_menu) {
                    let p = self.active_popups.borrow_mut().remove(popup_index);
                    self.close_popup_impl(&p);
                }
            }
        }
    }

    /// Close all active popups.
    pub fn close_all_popups(&self) {
        for popup in self.active_popups.take() {
            self.close_popup_impl(&popup);
        }
    }

    /// Close the top-most popup.
    pub fn close_top_popup(&self) {
        let popup = self.active_popups.borrow_mut().pop();
        if let Some(popup) = popup {
            self.close_popup_impl(&popup);
        }
    }

    /// Returns the scale factor set on the window, as provided by the windowing system.
    pub fn scale_factor(&self) -> f32 {
        self.pinned_fields.as_ref().project_ref().scale_factor.get()
    }

    /// Sets the scale factor for the window. This is set by the backend or for testing.
    pub(crate) fn set_scale_factor(&self, factor: f32) {
        self.pinned_fields.scale_factor.set(factor)
    }

    /// Reads the global property `TextInputInterface.text-input-focused`
    pub fn text_input_focused(&self) -> bool {
        self.pinned_fields.as_ref().project_ref().text_input_focused.get()
    }

    /// Sets the global property `TextInputInterface.text-input-focused`
    pub fn set_text_input_focused(&self, value: bool) {
        self.pinned_fields.text_input_focused.set(value)
    }

    /// Returns true if the window is visible
    pub fn is_visible(&self) -> bool {
        self.strong_component_ref.borrow().is_some()
    }

    /// Returns the window item that is the first item in the component. When Some()
    /// is returned, it's guaranteed to be safe to downcast to `WindowItem`.
    pub fn window_item_rc(&self) -> Option<ItemRc> {
        self.try_component().and_then(|component_rc| {
            let item_rc = ItemRc::new(component_rc, 0);
            if item_rc.downcast::<crate::items::WindowItem>().is_some() {
                Some(item_rc)
            } else {
                None
            }
        })
    }

    /// Returns the window item that is the first item in the component.
    pub fn window_item(&self) -> Option<VRcMapped<ItemTreeVTable, crate::items::WindowItem>> {
        self.try_component().and_then(|component_rc| {
            ItemRc::new(component_rc, 0).downcast::<crate::items::WindowItem>()
        })
    }

    /// Sets the size of the window item. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    pub(crate) fn set_window_item_geometry(&self, size: crate::lengths::LogicalSize) {
        if let Some(component_rc) = self.try_component() {
            let component = ItemTreeRc::borrow_pin(&component_rc);
            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) = ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
            {
                window_item.width.set(size.width_length());
                window_item.height.set(size.height_length());
            }
        }
    }

    /// Sets the close_requested callback. The callback will be run when the user tries to close a window.
    pub fn on_close_requested(&self, mut callback: impl FnMut() -> CloseRequestResponse + 'static) {
        self.close_requested.set_handler(move |()| callback());
    }

    /// Runs the close_requested callback.
    /// If the callback returns KeepWindowShown, this function returns false. That should prevent the Window from closing.
    /// Otherwise it returns true, which allows the Window to hide.
    pub fn request_close(&self) -> bool {
        match self.close_requested.call(&()) {
            CloseRequestResponse::HideWindow => true,
            CloseRequestResponse::KeepWindowShown => false,
        }
    }

    /// Returns if the window is currently maximized
    pub fn is_fullscreen(&self) -> bool {
        if let Some(window_item) = self.window_item() {
            window_item.as_pin_ref().full_screen()
        } else {
            false
        }
    }

    /// Set or unset the window to display fullscreen.
    pub fn set_fullscreen(&self, enabled: bool) {
        if let Some(window_item) = self.window_item() {
            window_item.as_pin_ref().full_screen.set(enabled);
            self.update_window_properties()
        }
    }

    /// Returns if the window is currently maximized
    pub fn is_maximized(&self) -> bool {
        self.maximized.get()
    }

    /// Set the window as maximized or unmaximized
    pub fn set_maximized(&self, maximized: bool) {
        self.maximized.set(maximized);
        self.update_window_properties()
    }

    /// Returns if the window is currently minimized
    pub fn is_minimized(&self) -> bool {
        self.minimized.get()
    }

    /// Set the window as minimized or unminimized
    pub fn set_minimized(&self, minimized: bool) {
        self.minimized.set(minimized);
        self.update_window_properties()
    }

    /// Returns the (context global) xdg app id for use with wayland and x11.
    pub fn xdg_app_id(&self) -> Option<SharedString> {
        self.ctx.xdg_app_id()
    }

    /// Returns the upgraded window adapter
    pub fn window_adapter(&self) -> Rc<dyn WindowAdapter> {
        self.window_adapter_weak.upgrade().unwrap()
    }

    /// Private access to the WindowInner for a given window.
    pub fn from_pub(window: &crate::api::Window) -> &Self {
        &window.0
    }

    /// Provides access to the Windows' Slint context.
    pub fn context(&self) -> &crate::SlintContext {
        &self.ctx
    }
}

/// Internal alias for `Rc<dyn WindowAdapter>`.
pub type WindowAdapterRc = Rc<dyn WindowAdapter>;

/// This module contains the functions needed to interface with the event loop and window traits
/// from outside the Rust language.
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]
    #![allow(clippy::missing_safety_doc)]
    #![allow(missing_docs)]

    use super::*;
    use crate::api::{RenderingNotifier, RenderingState, SetRenderingNotifierError};
    use crate::graphics::Size;
    use crate::graphics::{IntSize, Rgba8Pixel};
    use crate::items::WindowItem;
    use crate::SharedVector;

    /// This enum describes a low-level access to specific graphics APIs used
    /// by the renderer.
    #[repr(u8)]
    pub enum GraphicsAPI {
        /// The rendering is done using OpenGL.
        NativeOpenGL,
        /// The rendering is done using APIs inaccessible from C++, such as WGPU.
        Inaccessible,
    }

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Same layout as WindowAdapterRc
    #[repr(C)]
    pub struct WindowAdapterRcOpaque(*const c_void, *const c_void);

    /// Releases the reference to the windowrc held by handle.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_drop(handle: *mut WindowAdapterRcOpaque) {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        assert_eq!(
            core::mem::size_of::<Option<Rc<dyn WindowAdapter>>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        drop(core::ptr::read(handle as *mut Option<Rc<dyn WindowAdapter>>));
    }

    /// Releases the reference to the component window held by handle.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_clone(
        source: *const WindowAdapterRcOpaque,
        target: *mut WindowAdapterRcOpaque,
    ) {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        let window = &*(source as *const Rc<dyn WindowAdapter>);
        core::ptr::write(target as *mut Rc<dyn WindowAdapter>, window.clone());
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_show(handle: *const WindowAdapterRcOpaque) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);

        window_adapter.window().show().unwrap();
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_hide(handle: *const WindowAdapterRcOpaque) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().hide().unwrap();
    }

    /// Returns the visibility state of the window. This function can return false even if you previously called show()
    /// on it, for example if the user minimized the window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_visible(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        let window = &*(handle as *const Rc<dyn WindowAdapter>);
        window.window().is_visible()
    }

    /// Returns the window scale factor.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_get_scale_factor(
        handle: *const WindowAdapterRcOpaque,
    ) -> f32 {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).scale_factor()
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_scale_factor(
        handle: *const WindowAdapterRcOpaque,
        value: f32,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_scale_factor(value)
    }

    /// Returns the text-input-focused property value.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_get_text_input_focused(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).text_input_focused()
    }

    /// Set the text-input-focused property.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_text_input_focused(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_text_input_focused(value)
    }

    /// Sets the focus item.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_focus_item(
        handle: *const WindowAdapterRcOpaque,
        focus_item: &ItemRc,
        set_focus: bool,
        reason: FocusReason,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_focus_item(focus_item, set_focus, reason)
    }

    /// Associates the window with the given component.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_component(
        handle: *const WindowAdapterRcOpaque,
        component: &ItemTreeRc,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_component(component)
    }

    /// Show a popup and return its ID. The returned ID will always be non-zero.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_show_popup(
        handle: *const WindowAdapterRcOpaque,
        popup: &ItemTreeRc,
        position: LogicalPosition,
        close_policy: PopupClosePolicy,
        parent_item: &ItemRc,
        is_menu: bool,
    ) -> NonZeroU32 {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).show_popup(
            popup,
            position,
            close_policy,
            parent_item,
            is_menu,
        )
    }

    /// Close the popup by the given ID.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_close_popup(
        handle: *const WindowAdapterRcOpaque,
        popup_id: NonZeroU32,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).close_popup(popup_id);
    }

    /// C binding to the set_rendering_notifier() API of Window
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_rendering_notifier(
        handle: *const WindowAdapterRcOpaque,
        callback: extern "C" fn(
            rendering_state: RenderingState,
            graphics_api: GraphicsAPI,
            user_data: *mut c_void,
        ),
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        user_data: *mut c_void,
        error: *mut SetRenderingNotifierError,
    ) -> bool {
        struct CNotifier {
            callback: extern "C" fn(
                rendering_state: RenderingState,
                graphics_api: GraphicsAPI,
                user_data: *mut c_void,
            ),
            drop_user_data: extern "C" fn(*mut c_void),
            user_data: *mut c_void,
        }

        impl Drop for CNotifier {
            fn drop(&mut self) {
                (self.drop_user_data)(self.user_data)
            }
        }

        impl RenderingNotifier for CNotifier {
            fn notify(&mut self, state: RenderingState, graphics_api: &crate::api::GraphicsAPI) {
                let cpp_graphics_api = match graphics_api {
                    crate::api::GraphicsAPI::NativeOpenGL { .. } => GraphicsAPI::NativeOpenGL,
                    crate::api::GraphicsAPI::WebGL { .. } => unreachable!(), // We don't support wasm with C++
                    #[cfg(feature = "unstable-wgpu-26")]
                    crate::api::GraphicsAPI::WGPU26 { .. } => GraphicsAPI::Inaccessible, // There is no C++ API for wgpu (maybe wgpu c in the future?)
                    #[cfg(feature = "unstable-wgpu-27")]
                    crate::api::GraphicsAPI::WGPU27 { .. } => GraphicsAPI::Inaccessible, // There is no C++ API for wgpu (maybe wgpu c in the future?)
                };
                (self.callback)(state, cpp_graphics_api, self.user_data)
            }
        }

        let window = &*(handle as *const Rc<dyn WindowAdapter>);
        match window.renderer().set_rendering_notifier(Box::new(CNotifier {
            callback,
            drop_user_data,
            user_data,
        })) {
            Ok(()) => true,
            Err(err) => {
                *error = err;
                false
            }
        }
    }

    /// C binding to the on_close_requested() API of Window
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_on_close_requested(
        handle: *const WindowAdapterRcOpaque,
        callback: extern "C" fn(user_data: *mut c_void) -> CloseRequestResponse,
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        user_data: *mut c_void,
    ) {
        struct WithUserData {
            callback: extern "C" fn(user_data: *mut c_void) -> CloseRequestResponse,
            drop_user_data: extern "C" fn(*mut c_void),
            user_data: *mut c_void,
        }

        impl Drop for WithUserData {
            fn drop(&mut self) {
                (self.drop_user_data)(self.user_data)
            }
        }

        impl WithUserData {
            fn call(&self) -> CloseRequestResponse {
                (self.callback)(self.user_data)
            }
        }

        let with_user_data = WithUserData { callback, drop_user_data, user_data };

        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().on_close_requested(move || with_user_data.call());
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_request_redraw(handle: *const WindowAdapterRcOpaque) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.request_redraw();
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &mut euclid::default::Point2D<i32>,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        *pos = window_adapter.position().unwrap_or_default().to_euclid()
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_physical_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &euclid::default::Point2D<i32>,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.set_position(crate::api::PhysicalPosition::new(pos.x, pos.y).into());
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_logical_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &euclid::default::Point2D<f32>,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.set_position(LogicalPosition::new(pos.x, pos.y).into());
    }

    /// Returns the size of the window on the screen, in physical screen coordinates and excluding
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_size(handle: *const WindowAdapterRcOpaque) -> IntSize {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.size().to_euclid().cast()
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_physical_size(
        handle: *const WindowAdapterRcOpaque,
        size: &IntSize,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().set_size(crate::api::PhysicalSize::new(size.width, size.height));
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_logical_size(
        handle: *const WindowAdapterRcOpaque,
        size: &Size,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().set_size(crate::api::LogicalSize::new(size.width, size.height));
    }

    /// Return whether the style is using a dark theme
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_color_scheme(
        handle: *const WindowAdapterRcOpaque,
    ) -> ColorScheme {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter
            .internal(crate::InternalToken)
            .map_or(ColorScheme::Unknown, |x| x.color_scheme())
    }

    /// Return whether the platform supports native menu bars
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_supports_native_menu_bar(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.internal(crate::InternalToken).is_some_and(|x| x.supports_native_menu_bar())
    }

    /// Setup the native menu bar
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_setup_native_menu_bar(
        handle: *const WindowAdapterRcOpaque,
        menu_instance: &vtable::VRc<MenuVTable>,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        if let Some(x) = window_adapter.internal(crate::InternalToken) {
            x.setup_menubar(menu_instance.clone())
        }
    }

    /// Show a native context menu
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_show_native_popup_menu(
        handle: *const WindowAdapterRcOpaque,
        context_menu: &vtable::VRc<MenuVTable>,
        position: LogicalPosition,
        parent_item: &ItemRc,
    ) -> bool {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).show_native_popup_menu(
            context_menu.clone(),
            position,
            parent_item,
        )
    }

    /// Return the default-font-size property of the WindowItem
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_resolved_default_font_size(
        item_tree: &ItemTreeRc,
    ) -> f32 {
        WindowItem::resolved_default_font_size(item_tree.clone()).get()
    }

    /// Dispatch a key pressed or release event
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_dispatch_key_event(
        handle: *const WindowAdapterRcOpaque,
        event_type: crate::input::KeyEventType,
        text: &SharedString,
        repeat: bool,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().0.process_key_input(crate::items::KeyEvent {
            text: text.clone(),
            repeat,
            event_type,
            ..Default::default()
        });
    }

    /// Dispatch a mouse event
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_dispatch_pointer_event(
        handle: *const WindowAdapterRcOpaque,
        event: &crate::input::MouseEvent,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().0.process_mouse_input(event.clone());
    }

    /// Dispatch a window event
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_dispatch_event(
        handle: *const WindowAdapterRcOpaque,
        event: &crate::platform::WindowEvent,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event.clone());
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_fullscreen(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().is_fullscreen()
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_minimized(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().is_minimized()
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_maximized(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().is_maximized()
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_fullscreen(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().set_fullscreen(value)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_minimized(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().set_minimized(value)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_maximized(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().set_maximized(value)
    }

    /// Takes a snapshot of the window contents and returns it as RGBA8 encoded pixel buffer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_take_snapshot(
        handle: *const WindowAdapterRcOpaque,
        data: &mut SharedVector<Rgba8Pixel>,
        width: &mut u32,
        height: &mut u32,
    ) -> bool {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        if let Ok(snapshot) = window_adapter.window().take_snapshot() {
            *data = snapshot.data.clone();
            *width = snapshot.width();
            *height = snapshot.height();
            true
        } else {
            false
        }
    }
}

#[cfg(feature = "software-renderer")]
#[test]
fn test_empty_window() {
    // Test that when creating an empty window without a component, we don't panic when render() is called.
    // This isn't typically done intentionally, but for example if we receive a paint event in Qt before a component
    // is set, this may happen. Concretely as per #2799 this could happen with popups where the call to
    // QWidget::show() with egl delivers an immediate paint event, before we've had a chance to call set_component.
    // Let's emulate this scenario here using public platform API.

    let msw = crate::software_renderer::MinimalSoftwareWindow::new(
        crate::software_renderer::RepaintBufferType::NewBuffer,
    );
    msw.window().request_redraw();
    let mut region = None;
    let render_called = msw.draw_if_needed(|renderer| {
        let mut buffer =
            crate::graphics::SharedPixelBuffer::<crate::graphics::Rgb8Pixel>::new(100, 100);
        let stride = buffer.width() as usize;
        region = Some(renderer.render(buffer.make_mut_slice(), stride));
    });
    assert!(render_called);
    let region = region.unwrap();
    assert_eq!(region.bounding_box_size(), PhysicalSize::default());
    assert_eq!(region.bounding_box_origin(), PhysicalPosition::default());
}
