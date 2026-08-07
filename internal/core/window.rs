// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore backtab componentrc datastructure subelements unmaximized unminimized

#![warn(missing_docs)]
//! Exposed Window API
use crate::api::{
    CloseRequestResponse, LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize,
    PlatformError, Window, WindowPosition, WindowSize,
};
use crate::cursor::MouseCursorInner;
use crate::input::{
    ClickState, DragData, FocusEvent, FocusReason, InternalKeyEvent, KeyEventResult, KeyEventType,
    Keys, MouseEvent, MouseInputState, PointerEventButton, TextCursorBlinker, TouchPhase,
    TouchState, key_codes,
};
use crate::item_tree::{
    ItemRc, ItemTreeRc, ItemTreeRef, ItemTreeRefPin, ItemTreeVTable, ItemTreeWeak, ItemWeak,
    ParentItemTraversalMode,
};
use crate::items::{
    BuiltInMouseCursor, InputMethodHints, InputType, ItemRef, MenuEntry, PopupClosePolicy,
};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalVector, SizeLengths};
use crate::menus::MenuVTable;
use crate::properties::{ChangeTracker, Property, PropertyTracker};
use crate::renderer::Renderer;
use crate::{Callback, Coord, SharedString, SharedVector};
use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::num::NonZeroU32;
use core::pin::Pin;
use euclid::num::Zero;
use vtable::{VRc, VRcMapped};

pub mod popup;

fn next_focus_item(item: ItemRc) -> ItemRc {
    item.next_focus_item()
}

fn previous_focus_item(item: ItemRc) -> ItemRc {
    item.previous_focus_item()
}

/// The window kind when creating a new child window
#[repr(C)]
pub enum WindowKind {
    /// Tooltip
    ToolTip,
    /// Popup Window
    Popup,
    /// Popup Menu
    Menu,
}

/// This trait represents the adaptation layer between the [`Window`] API and then
/// windowing specific window representation, such as a Win32 `HWND` handle or a `wayland_surface_t`.
///
/// Implement this trait to establish the link between the two, and pass messages in both
/// directions:
///
/// - When receiving messages from the windowing system about state changes, such as the window being resized,
///   the user requested the window to be closed, input being received, etc. you need to create a
///   [`WindowEvent`](crate::platform::WindowEvent) and send it to Slint via [`Window::dispatch_event_with_result()`].
///
/// - Slint sends requests to change visibility, position, size, etc. via functions such as [`Self::set_visible`],
///   [`Self::set_size`], [`Self::set_position`], or [`Self::update_window_properties()`]. Re-implement these functions
///   and delegate the requests to the windowing system.
///
/// If the implementation of this bi-directional message passing protocol is incomplete, the user may
/// experience unexpected behavior, or the intention of the developer calling functions on the [`Window`]
/// API may not be fulfilled.
///
/// Your implementation must hold a renderer, such as `SoftwareRenderer` or `FemtoVGRenderer`.
/// In the [`Self::renderer()`] function, you must return a reference to it.
///
/// It is also required to hold a [`Window`] and return a reference to it in your
/// implementation of [`Self::window()`].
///
/// See also `slint::platform::software_renderer::MinimalSoftwareWindow`
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

/// What a `DragArea` offers to start a native (OS-level) drag, passed to
/// [`WindowAdapterInternal::start_drag`].
///
/// A read-only view: the backend reads the payload, allowed actions, and drag image; the source
/// item and other routing data stay in the core.
#[derive(Clone)]
#[doc(hidden)]
pub struct DragRequest {
    pub(crate) data: crate::data_transfer::DataTransfer,
    pub(crate) allowed: crate::items::AllowedDragActions,
    pub(crate) drag_image: crate::graphics::Image,
    pub(crate) drag_image_offset: euclid::default::Vector2D<i32>,
}

impl DragRequest {
    /// The data being transferred.
    pub fn data(&self) -> &crate::data_transfer::DataTransfer {
        &self.data
    }
    /// The set of actions the drag source permits.
    pub fn allowed_actions(&self) -> crate::items::AllowedDragActions {
        self.allowed
    }
    /// The image to show under the cursor while dragging.
    pub fn drag_image(&self) -> &crate::graphics::Image {
        &self.drag_image
    }
    /// The offset of the drag image relative to the cursor, in pixels.
    pub fn drag_image_offset(&self) -> euclid::default::Vector2D<i32> {
        self.drag_image_offset
    }
}

/// A drag a `DragArea` started, tracked by the core while in flight.
/// The backend sees only the [`DragRequest`]; the source and seed position stay here to report
/// completion and to arm the in-window fallback.
#[derive(Clone)]
pub(crate) struct NativePendingDrag {
    pub(crate) request: DragRequest,
    /// The `DragArea` that initiated the drag.
    pub(crate) source: ItemWeak,
    /// The pointer position that crossed the drag threshold, used to seed the in-window drag.
    pub(crate) seed_position: LogicalPosition,
}

/// Implementation details behind [`WindowAdapter`], but since this
/// trait is not exported in the public API, it is not possible for the
/// users to call or re-implement these functions.
// TODO: add events for window receiving and loosing focus
#[doc(hidden)]
pub trait WindowAdapterInternal: core::any::Any {
    /// This function is called by the generated code when a component and therefore its tree of items are created.
    fn register_item_tree(&self, _: ItemTreeRefPin) {}

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources.
    fn unregister_item_tree(
        &self,
        _component: ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>,
    ) {
    }

    /// Get the parent window adapter of this window adapter
    fn get_parent(&self) -> Option<Rc<dyn WindowAdapter>> {
        None
    }

    /// Create a window for a popup.
    /// This function will create only the window adapter but does not show the popup it self
    /// Use this window adapter to create a new popup window and show it with `show_popup()`
    ///
    /// If this function return None (the default implementation), then the
    /// popup will be rendered within the window itself.
    fn create_child_window_adapter(
        &self,
        _window_kind: WindowKind,
    ) -> Option<Rc<dyn WindowAdapter>> {
        None
    }

    /// Set the mouse cursor
    // TODO: Make the enum public and make public
    fn set_mouse_cursor(&self, _cursor: MouseCursorInner) {}

    /// This method allow editable input field to communicate with the platform about input methods
    fn input_method_request(&self, _: InputMethodRequest) {}

    /// Handle focus change
    // used for accessibility
    fn handle_focus_change(&self, _old: Option<ItemRc>, _new: Option<ItemRc>) {}

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

    /// Return the inset of the safe area of the Window in physical pixels.
    /// This is necessary to avoid overlapping system UI such as notches or system bars.
    fn safe_area_inset(&self) -> crate::lengths::PhysicalEdges {
        Default::default()
    }

    /// Start a native (OS-level) drag-and-drop operation.
    ///
    /// Returns `true` if the backend took the drag over (it may defer the actual start).
    /// Returns `false` (the default) when native drag is unsupported; the caller then arms the
    /// in-window drag.
    ///
    /// On completion the backend calls [`WindowInner::report_drag_finished`]
    /// (`DragAction::None` if cancelled), or [`WindowInner::start_in_window_drag`] if a native
    /// start fails after returning `true`.
    fn start_drag(&self, _request: &DragRequest) -> bool {
        false
    }

    /// Ask the windowing system to start an interactive, user-driven move of the window,
    /// as if the user had dragged the window's title bar.
    ///
    /// This is called while the user holds a mouse button pressed.
    /// The default implementation does nothing; backends without support ignore the request.
    fn start_window_move(&self) {}
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
    /// The hints for the input method for the text edit.
    pub input_method_hints: InputMethodHints,
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
        self.0.is_maximized()
    }

    /// true if the window is in a minimized state, otherwise false
    pub fn is_minimized(&self) -> bool {
        self.0.is_minimized()
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

pub(crate) struct PopupWindowPropertiesTracker {
    /// Weak reference to the parent window that owns the active_popups list
    parent_window_adapter_weak: Weak<dyn WindowAdapter>,
    /// ID of the popup this tracker belongs to, used to re-evaluate after notification
    popup_id: NonZeroU32,
}

impl crate::properties::PropertyDirtyHandler for PopupWindowPropertiesTracker {
    fn notify(self: Pin<&Self>) {
        let parent = self.parent_window_adapter_weak.clone();
        let popup_id = self.popup_id;
        // Use a timer here, so if we change multiple properties at the same time not multiple notifications are send
        // This timer will delay for the next evaluation
        crate::timers::Timer::single_shot(Default::default(), move || {
            if let Some(parent_adapter) = parent.upgrade() {
                WindowInner::from_pub(parent_adapter.window()).update_popup_properties(popup_id);
            }
        });
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
pub enum PopupWindowLocation {
    /// The popup is rendered in its own top-level window that is know to the windowing system.
    TopLevel(Rc<dyn WindowAdapter>),
    /// The popup is rendered as an embedded child window at the given position.
    ChildWindow(LogicalPoint),
}

/// This structure defines a graphical element that is designed to pop up from the surrounding
/// UI content, for example to show a context menu.
pub struct PopupWindow {
    /// The ID of the associated popup.
    pub popup_id: NonZeroU32,
    /// The location defines where the pop up is rendered.
    pub location: PopupWindowLocation,
    /// The component that is responsible for providing the popup content.
    pub component: ItemTreeRc,
    /// Defines the close behavior of the popup.
    pub close_policy: PopupClosePolicy,
    /// the item that had the focus in the parent window when the popup was opened
    focus_item_in_parent: ItemWeak,
    /// The item from where the Popup was invoked from
    pub parent_item: ItemWeak,
    /// Overlay tooltip: no focus steal, unclamped placement, skipped in main mouse routing.
    /// Context / popup menu: participates in menu-chain hit testing and cascading close.
    pub window_kind: WindowKind,
    /// Callback that returns the current desired logical position of the popup.
    /// Called during re-evaluation of the position tracker to re-subscribe to dependencies.
    /// IMPORTANT: This position is relative to the parent
    position_access: Box<dyn Fn() -> LogicalPosition>,
    /// Keeps the parent component's `PopupWindow::is-open` property in sync. Provided to
    /// [`WindowInner::show_popup`], invoked with `true` when the popup is shown and with `false` when
    /// this `PopupWindow` is dropped (see the `Drop` impl below). It is a no-op for popups whose
    /// parent does not read `is-open` (menus and tooltips).
    is_open_setter: Box<dyn Fn(bool)>,
    // tracks all relevant properties and reacts on changes
    properties_tracker: Pin<Box<PropertyTracker<true, PopupWindowPropertiesTracker>>>,
}

impl Drop for PopupWindow {
    fn drop(&mut self) {
        // Dropping the `PopupWindow` is the single choke point that every close path funnels through
        // (click-outside, selection, programmatic `close()`, sibling replacement, window change,
        // Escape, and tearing down the window itself), so flip the parent's `is-open` back to false
        // here rather than in any individual close function.
        (self.is_open_setter)(false);
    }
}

#[pin_project::pin_project]
struct WindowPinnedFields {
    #[pin]
    redraw_tracker: PropertyTracker<false, WindowRedrawTracker>,
    /// Gets dirty when the layout restrictions, or some other property of the windows change
    #[pin]
    window_properties_tracker: PropertyTracker<true, WindowPropertiesTracker>,
    #[pin]
    scale_factor: Property<f32>,
    #[pin]
    active: Property<bool>,
    #[pin]
    text_input_focused: Property<bool>,
    #[pin]
    menubar_shortcuts: Property<SharedVector<MenuEntry>>,
}

/// The outcome of dispatching a [`MouseEvent`] through [`WindowInner::process_mouse_input`].
#[derive(Copy, Clone, Debug)]
pub struct MouseDispatchResult {
    /// For `MouseEvent::DragMove` / `MouseEvent::Drop` events, the action negotiated with
    /// the accepting `DropArea` (or `None` if no `DropArea` accepted). Always `None` for
    /// other event kinds.
    pub drag_action: Option<crate::items::DragAction>,
    /// `true` if an item consumed the event (`EventAccepted`, `GrabMouse`, `StartDrag`, or
    /// a `DropArea` accepting a drag/drop). `false` if the event fell through without a taker.
    pub accepted: bool,
}

/// Inner datastructure for the [`crate::api::Window`]
pub struct WindowInner {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    component: RefCell<ItemTreeWeak>,
    /// When the window is visible, keep a strong reference
    strong_component_ref: RefCell<Option<ItemTreeRc>>,
    mouse_input_state: Cell<MouseInputState>,
    touch_state: RefCell<TouchState>,

    /// ItemRC that currently have the focus (possibly an instance of TextInput)
    pub focus_item: RefCell<crate::item_tree::ItemWeak>,
    focus_item_visibility_tracker: ChangeTracker,
    /// The last text that was sent to the input method
    pub(crate) last_ime_text: RefCell<SharedString>,
    /// Don't let ComponentContainers's instantiation change the focus.
    /// This is a workaround for a recursion when instantiating ComponentContainer because the
    /// init code for the component might have code that sets the focus, but we don't want that
    /// for the ComponentContainer
    pub(crate) prevent_focus_change: Cell<bool>,
    cursor_blinker: RefCell<pin_weak::rc::PinWeak<crate::input::TextCursorBlinker>>,

    pinned_fields: Pin<Box<WindowPinnedFields>>,

    menubar: RefCell<Option<vtable::VWeak<MenuVTable>>>,

    /// Stack of currently active popups
    pub active_popups: RefCell<Vec<PopupWindow>>,
    next_popup_id: Cell<NonZeroU32>,
    had_popup_on_press: Cell<bool>,
    close_requested: Callback<(), CloseRequestResponse>,
    click_state: ClickState,
    ctx: core::cell::OnceCell<crate::SlintContext>,
    /// The native drag we started, if one is in flight.
    /// It holds the source and seed position to report completion and to arm the in-window
    /// fallback, and lets a drop back onto this same window restore the source's `DataTransfer`:
    /// the OS round-trip can't carry in-app `user_data`.
    native_drag: RefCell<Option<NativePendingDrag>>,
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
            touch_state: Default::default(),
            pinned_fields: Box::pin(WindowPinnedFields {
                redraw_tracker,
                window_properties_tracker,
                scale_factor: Property::new_named(1., "i_slint_core::Window::scale_factor"),
                active: Property::new_named(false, "i_slint_core::Window::active"),
                text_input_focused: Property::new_named(
                    false,
                    "i_slint_core::Window::text_input_focused",
                ),
                menubar_shortcuts: Property::new_named(
                    SharedVector::default(),
                    "i_slint_core::Window::menubar_shortcuts",
                ),
            }),
            focus_item: Default::default(),
            focus_item_visibility_tracker: Default::default(),
            last_ime_text: Default::default(),
            cursor_blinker: Default::default(),
            active_popups: Default::default(),
            next_popup_id: Cell::new(NonZeroU32::MIN),
            had_popup_on_press: Default::default(),
            close_requested: Default::default(),
            click_state: ClickState::default(),
            prevent_focus_change: Default::default(),
            ctx: Default::default(),
            menubar: Default::default(),
            native_drag: Default::default(),
        }
    }

    /// Associates this window with the specified component. Further event handling and rendering, etc. will be
    /// done with that component.
    pub fn set_component(&self, component: &ItemTreeRc) {
        self.close_all_popups();
        self.focus_item_visibility_tracker.clear();
        self.focus_item.replace(Default::default());
        self.mouse_input_state.replace(Default::default());
        self.touch_state.replace(Default::default());
        self.component.replace(ItemTreeRc::downgrade(component));
        self.pinned_fields.window_properties_tracker.set_dirty(); // component changed, layout constraints for sure must be re-calculated
        let window_adapter = self.window_adapter();
        window_adapter.renderer().set_window_adapter(&window_adapter);
        let scale_factor = self.scale_factor();
        self.set_window_item_geometry(window_adapter.size().to_logical(scale_factor).to_euclid());
        let inset = window_adapter
            .internal(crate::InternalToken)
            .map(|internal| internal.safe_area_inset())
            .unwrap_or_default();
        self.set_window_item_safe_area(inset.to_logical(scale_factor));
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

    /// Walk the component tree and every active popup to materialize every
    /// Repeater, Conditional and ComponentContainer.  Runs change handlers
    /// and the instantiation pass in a loop because init callbacks may set
    /// properties that trigger change handlers, and change handlers may
    /// make new conditionals/repeaters dirty.
    pub fn ensure_tree_instantiated(&self) {
        // Instantiation runs first so that ListView's ensure_updated_listview
        // sees the model property before any change handler can reset it.
        for _ in 0..10 {
            let mut changed = false;
            if let Some(component) = self.try_component() {
                changed |= crate::item_tree::ensure_item_tree_instantiated(&component);
            }
            for popup in self.active_popups.borrow().iter() {
                changed |= crate::item_tree::ensure_item_tree_instantiated(&popup.component);
            }
            changed |= crate::properties::ChangeTracker::run_change_handlers_once();
            if !changed {
                return;
            }
        }
        crate::debug_log!("Slint: long callback/instantiation chain detected");
    }

    /// Returns a slice of the active popups.
    pub fn active_popups(&self) -> core::cell::Ref<'_, [PopupWindow]> {
        core::cell::Ref::map(self.active_popups.borrow(), |v| v.as_slice())
    }

    /// Receive a mouse event and pass it to the items of the component to
    /// change their state.
    ///
    /// Returns `None` when there is no component to dispatch to; otherwise returns a
    /// [`MouseDispatchResult`] carrying:
    /// - `accepted`: whether an item consumed the event, and
    /// - `drag_action`: for `DragMove`/`Drop` events, the negotiated
    ///   [`DragAction`](crate::items::DragAction) (or `None` if no `DropArea` accepted).
    ///
    /// Note: when a drag is in flight, the runtime rewrites a `Released` into either a
    /// `Drop` (if a `DropArea` had previously accepted the matching `DragMove`) or an
    /// `Exit` (if not). The reported `accepted` reflects the rewritten event, so a
    /// `Released` that completes a drop on a non-accepting target reports `accepted = false`.
    pub fn process_mouse_input(&self, mut event: MouseEvent) -> Option<MouseDispatchResult> {
        crate::animations::update_animations();

        let item_tree = self.try_component()?;
        self.ensure_tree_instantiated();

        // If the focused item became invisible (e.g. a TabWidget switched away from
        // the tab holding it), drop the focus so that input methods get torn down.
        // The key-event handler does the same, but a tab is switched with a pointer
        // tap, not a key press, so it must also happen here.
        if self.focus_item.borrow().upgrade().is_some_and(|i| !i.is_visible()) {
            self.take_focus_item(&FocusEvent::FocusOut(FocusReason::TabNavigation));
        }

        // handle multiple press release
        event = self.click_state.check_repeat(event, self.context().platform().click_interval());

        let window_adapter = self.window_adapter();
        let mut mouse_input_state = self.mouse_input_state.take();

        let was_dragging = mouse_input_state.drag_data.is_some();
        let old_cursor = core::mem::replace(
            &mut mouse_input_state.cursor,
            MouseCursorInner::BuiltIn(BuiltInMouseCursor::Default),
        );

        // drag-finished firing is deferred until after dispatch so the DropArea has had
        // a chance to fire its own `dropped` callback first; that callback returns the
        // final action, which the runtime then forwards to the source.
        let mut pending_drag_finished: Option<(
            crate::item_tree::ItemWeak,
            Option<crate::item_tree::ItemWeak>,
        )> = None;

        if let Some(DragData { event: mut drop_event, allowed }) =
            mouse_input_state.drag_data.clone()
        {
            match &event {
                MouseEvent::Released { position, button: PointerEventButton::Left, .. } => {
                    mouse_input_state.drag_data = None;
                    let source = mouse_input_state.drag_source.take();
                    if let Some(target_weak) = mouse_input_state.drop_target.take() {
                        // Seed `proposed-action` for the dropped callback with the action the
                        // target last chose during hover; the callback's return value will
                        // become the final action reported to the source.
                        let hovered = target_weak
                            .upgrade()
                            .and_then(|t| t.downcast::<crate::items::DropArea>())
                            .map(|d| d.as_pin_ref().current_action())
                            .unwrap_or(crate::items::DragAction::None);
                        drop_event.proposed_action = hovered;
                        drop_event.position = crate::lengths::logical_position_to_api(*position);
                        event = MouseEvent::Drop { event: drop_event, allowed };
                        if let Some(s) = source {
                            pending_drag_finished = Some((s, Some(target_weak)));
                        }
                    } else {
                        // No DropArea accepted the most recent DragMove. Tear the drag
                        // down via Exit instead of converting to Drop so a non-accepting
                        // DropArea under the cursor doesn't fire `dropped`, and so the
                        // underlying Release doesn't reach hit-tested items as a
                        // spurious click.
                        event = MouseEvent::Exit;
                        if let Some(s) = source {
                            pending_drag_finished = Some((s, None));
                        }
                    }
                }
                MouseEvent::Moved { position, .. } => {
                    drop_event.position = crate::lengths::logical_position_to_api(*position);
                    // Recompute the proposed action from current modifier state so the target's
                    // `can-drop` callback sees an up-to-date `event.proposed-action`.
                    drop_event.proposed_action = crate::items::compute_proposed_action(
                        self.context().0.modifiers.get().into(),
                        allowed,
                    );
                    // Mirror the position and proposed action into the persistent state so the
                    // renderer can place the drag-image overlay without re-deriving the cursor
                    // location, and so a subsequent synthetic Moved (e.g. fired from a modifier
                    // key press) starts from the right position.
                    if let Some(d) = mouse_input_state.drag_data.as_mut() {
                        d.event.position = drop_event.position;
                        d.event.proposed_action = drop_event.proposed_action;
                    }
                    mouse_input_state.cursor =
                        MouseCursorInner::BuiltIn(BuiltInMouseCursor::NoDrop);
                    event = MouseEvent::DragMove { event: drop_event, allowed };
                }
                MouseEvent::Exit => {
                    mouse_input_state.drag_data = None;
                    mouse_input_state.drop_target = None;
                    if let Some(s) = mouse_input_state.drag_source.take() {
                        pending_drag_finished = Some((s, None));
                    }
                }
                _ => {}
            }
        } else if let MouseEvent::DragMove { event, .. } | MouseEvent::Drop { event, .. } =
            &mut event
        {
            // An incoming native drag while our own is in flight: the same operation looping
            // back onto the source window. Restore the full source data so a same-window drop
            // sees the `user_data` the OS round-trip dropped.
            if let Some(pending) = self.native_drag.borrow().as_ref() {
                event.data = pending.request.data.clone();
            }
        }

        let pressed_event = matches!(event, MouseEvent::Pressed { .. });
        let released_event = matches!(event, MouseEvent::Released { .. });
        let had_delay = mouse_input_state.has_delayed_event();

        let last_top_item = mouse_input_state.top_item_including_delayed();
        if released_event {
            mouse_input_state =
                crate::input::process_delayed_event(&window_adapter, mouse_input_state);
        }

        let parent_adapter = window_adapter
            .internal(crate::InternalToken)
            .and_then(|internal| internal.get_parent())
            .unwrap_or_else(|| window_adapter.clone());
        let active_popups = &WindowInner::from_pub(parent_adapter.window()).active_popups;
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

        let grab_result =
            crate::input::handle_mouse_grab(&event, &window_adapter, &mut mouse_input_state);
        let grab_accepted = grab_result.accepted;

        let mut dispatch_accepted = false;
        mouse_input_state = if let Some(mut event) = grab_result.event {
            // The grab handler may have fired callbacks that modified models or
            // other state, so materialize any pending repeater/conditional
            // changes before hit-testing with the returned event.
            self.ensure_tree_instantiated();
            let mut item_tree = self.component.borrow().upgrade();
            let mut offset = LogicalPoint::default();
            let mut menubar_item = None;
            for (idx, popup) in active_popups.borrow().iter().enumerate().rev() {
                if matches!(popup.window_kind, WindowKind::ToolTip) {
                    continue;
                }
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

                if !matches!(popup.window_kind, WindowKind::Menu) {
                    break;
                } else if popup_to_close.is_some() {
                    // clicking outside of a popup menu should close all the menus
                    popup_to_close = Some(popup.popup_id);
                }

                menubar_item = popup.parent_item.upgrade();
            }

            let root = match menubar_item {
                None => item_tree.map(|item_tree| ItemRc::new_root(item_tree.clone())),
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
                let crate::input::MouseInputResult { mut state, accepted } =
                    crate::input::process_mouse_input(
                        root,
                        &event,
                        &window_adapter,
                        mouse_input_state,
                    );
                state.offset = offset;
                dispatch_accepted = accepted;
                state
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

        let accepted = dispatch_accepted | grab_accepted;

        if last_top_item != mouse_input_state.top_item_including_delayed() {
            self.click_state.reset();
            self.click_state.check_repeat(event, self.context().platform().click_interval());
        }

        if !had_delay && mouse_input_state.has_delayed_event() {
            // A delay was just set up, preserve the old cursor
            mouse_input_state.cursor = old_cursor;
        } else if old_cursor != mouse_input_state.cursor
            && let Some(window_adapter) = window_adapter.internal(crate::InternalToken)
        {
            window_adapter.set_mouse_cursor(mouse_input_state.cursor.clone());
        }

        let is_dragging = mouse_input_state.drag_data.is_some();
        let drag_action = mouse_input_state.drop_target_action();
        self.mouse_input_state.set(mouse_input_state);

        // The drag-image overlay follows the cursor and lives outside any item tree, so
        // partial renderers won't otherwise know to repaint it on mouse motion or after
        // the drag ends. `render_drag_image_overlay` marks its painted rect dirty itself,
        // we just need to schedule the redraw.
        if was_dragging || is_dragging {
            window_adapter.request_redraw();
        }

        if pending_drag_finished.is_some() {
            // A drag ended in-window (including after a native start fell back), so drop the
            // stash.
            self.native_drag.borrow_mut().take();
        }
        if let Some((source_weak, target_weak)) = pending_drag_finished
            && let Some(source) = source_weak.upgrade()
            && let Some(drag_area) = source.downcast::<crate::items::DragArea>()
        {
            // The action `dropped` returned is now sitting on the target's `current_action`.
            // For a cancelled drag (no target) we just report None.
            let target = target_weak
                .and_then(|w| w.upgrade())
                .and_then(|i| i.downcast::<crate::items::DropArea>());
            let action = target
                .as_ref()
                .map(|d| d.as_pin_ref().current_action())
                .unwrap_or(crate::items::DragAction::None);
            drag_area.as_pin_ref().finish_drag(action);
            // The drag is over: reset the target's `current_action` so it matches
            // `has_drag` and the docstring ("none when no drag is hovering").
            if let Some(target) = target {
                target.as_pin_ref().current_action.set(crate::items::DragAction::None);
            }
        }

        if let Some(popup_id) = popup_to_close {
            WindowInner::from_pub(parent_adapter.window()).close_popup(popup_id);
        }

        self.ensure_tree_instantiated();

        Some(MouseDispatchResult { drag_action, accepted })
    }

    /// Remember (or clear) the in-flight native drag, so a backend can report completion or fall
    /// back, and a drop back onto this window can restore the data. Set by `offer_native_drag`.
    pub(crate) fn set_native_drag(&self, drag: Option<NativePendingDrag>) {
        *self.native_drag.borrow_mut() = drag;
    }

    /// Report that the in-flight native drag finished with `action`.
    ///
    /// Backends call this when the OS drag completes (`DragAction::None` if cancelled); the
    /// source `DragArea` clears `dragging` and fires `drag-finished`.
    pub fn report_drag_finished(&self, action: crate::items::DragAction) {
        let Some(pending) = self.native_drag.borrow_mut().take() else {
            return;
        };
        if let Some(drag_area) =
            pending.source.upgrade().and_then(|i| i.downcast::<crate::items::DragArea>())
        {
            drag_area.as_pin_ref().finish_drag(action);
        }
    }

    /// Fall back to the in-window drag for the in-flight native drag.
    ///
    /// Backends call this when a native start fails after taking the drag over; subsequent mouse
    /// moves then drive `DragMove`/`Drop` and the drag-image overlay, in-process.
    pub fn start_in_window_drag(&self) {
        let (source, seed_position) = {
            let native_drag = self.native_drag.borrow();
            let Some(drag) = native_drag.as_ref() else {
                return;
            };
            (drag.source.clone(), drag.seed_position)
        };
        let Some(drag_area) = source.upgrade().and_then(|i| i.downcast::<crate::items::DragArea>())
        else {
            return;
        };
        let mut state = self.mouse_input_state.take();
        state.arm_in_window_drag(drag_area.as_pin_ref(), source, seed_position);
        self.mouse_input_state.set(state);
        self.window_adapter().request_redraw();
    }

    /// Receive a raw touch event from a backend and either forward it as a mouse
    /// event (single finger) or synthesize `PinchGesture`/`RotationGesture` events
    /// (two fingers), producing the same events as platform gesture recognition.
    ///
    /// `position` must be in **logical coordinates** (i.e., already divided by the
    /// scale factor). Passing physical coordinates will produce incorrect gesture
    /// geometry and hit-testing.
    ///
    /// `drag_action` is taken from the *last* sub-event (including `None`), because
    /// `drag_action` reflects the current drop-target negotiation, not a per-event
    /// verdict to aggregate. For touch sequences that never produce a `DragMove`/`Drop`
    /// (the common case), this stays `None` throughout.
    pub fn process_touch_input(
        &self,
        id: i32,
        position: LogicalPoint,
        phase: TouchPhase,
    ) -> Option<MouseDispatchResult> {
        let events = self.touch_state.borrow_mut().process(id, position, phase);
        let mut aggregate: Option<MouseDispatchResult> = None;
        for event in events.into_iter() {
            if let Some(r) = self.process_mouse_input(event) {
                let agg = aggregate
                    .get_or_insert(MouseDispatchResult { drag_action: None, accepted: false });
                agg.accepted |= r.accepted;
                agg.drag_action = r.drag_action;
            }
        }
        aggregate
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
    pub fn process_key_input(
        &self,
        mut internal_key_event: InternalKeyEvent,
    ) -> crate::input::KeyEventResult {
        self.ensure_tree_instantiated();
        // NFC-normalize the event text so that shortcut matching works consistently
        // regardless of the composed/decomposed form the backend provides
        // (e.g. é as U+00E9 vs e + U+0301).
        // Note: icu_normalizer is currently only enabled if parley is enabled
        #[cfg(feature = "shared-parley")]
        {
            let normalizer = icu_normalizer::ComposingNormalizer::new_nfc();
            let normalized = normalizer.normalize(&internal_key_event.key_event.text);
            // Only replace the event text if normalization actually changed it,
            // to avoid unnecessary allocations.
            if let alloc::borrow::Cow::Owned(normalized) = normalized {
                internal_key_event.key_event.text = normalized.into();
            }
        }

        if let Some(updated_modifier) = self.context().0.modifiers.get().state_update(
            internal_key_event.event_type == KeyEventType::KeyPressed,
            &internal_key_event.key_event.text,
        ) {
            // Updates the key modifiers depending on the key code and pressed state.
            self.context().0.modifiers.set(updated_modifier);

            // If a drag is in flight, synthesize a Moved at the last drag position so
            // the new modifier state flows into `event.proposed-action` and the target's
            // `can-drop` re-runs — letting the user change copy/move/link with Ctrl/Shift
            // without having to move the mouse first.
            let drag_pos = {
                let state = self.mouse_input_state.take();
                let pos = state.drag_data.as_ref().map(|d| d.event.position);
                self.mouse_input_state.replace(state);
                pos
            };
            if let Some(pos) = drag_pos {
                self.process_mouse_input(MouseEvent::Moved {
                    position: crate::lengths::logical_point_from_api(pos),
                    touch_finger_id: 0,
                });
            }
        }

        internal_key_event.key_event.modifiers =
            self.context().0.modifiers.get().modifiers_for(&internal_key_event);

        // Emulate macOS menubar behavior: The OS consumes the event before it reaches any
        // Slint widgets. Therefore we process the menubar shortcuts here first and abort event
        // propagation if a shortcut matches.
        if self.process_menubar_shortcuts(&internal_key_event) == KeyEventResult::EventAccepted {
            self.ensure_tree_instantiated();
            return crate::input::KeyEventResult::EventAccepted;
        }

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
            if i.borrow().as_ref().capture_key_event(&internal_key_event, &self.window_adapter(), i)
                == crate::input::KeyEventResult::EventAccepted
            {
                self.ensure_tree_instantiated();
                return crate::input::KeyEventResult::EventAccepted;
            }
        }

        drop(item_list);

        // Deliver key_event (to focused item, going up towards the window):
        while let Some(focus_item) = item {
            if focus_item.borrow().as_ref().key_event(
                &internal_key_event,
                &self.window_adapter(),
                &focus_item,
            ) == crate::input::KeyEventResult::EventAccepted
            {
                self.ensure_tree_instantiated();
                return crate::input::KeyEventResult::EventAccepted;
            }
            item = focus_item.parent_item(ParentItemTraversalMode::StopAtPopups);
        }

        // Make Tab/Backtab handle keyboard focus
        let extra_mod = internal_key_event.key_event.modifiers.control
            || internal_key_event.key_event.modifiers.meta
            || internal_key_event.key_event.modifiers.alt;
        if internal_key_event.key_event.text.starts_with(key_codes::Tab)
            && !internal_key_event.key_event.modifiers.shift
            && !extra_mod
            && internal_key_event.event_type == KeyEventType::KeyPressed
        {
            self.focus_next_item();
            self.ensure_tree_instantiated();
            return crate::input::KeyEventResult::EventAccepted;
        } else if (internal_key_event.key_event.text.starts_with(key_codes::Backtab)
            || (internal_key_event.key_event.text.starts_with(key_codes::Tab)
                && internal_key_event.key_event.modifiers.shift))
            && internal_key_event.event_type == KeyEventType::KeyPressed
            && !extra_mod
        {
            self.focus_previous_item();
            self.ensure_tree_instantiated();
            return crate::input::KeyEventResult::EventAccepted;
        } else if internal_key_event.event_type == KeyEventType::KeyPressed
            && internal_key_event.key_event.text.starts_with(key_codes::Escape)
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
            self.ensure_tree_instantiated();
            return crate::input::KeyEventResult::EventAccepted;
        }

        self.ensure_tree_instantiated();
        crate::input::KeyEventResult::EventIgnored
    }

    fn process_menubar_shortcuts(
        &self,
        internal_key_event: &InternalKeyEvent,
    ) -> crate::input::KeyEventResult {
        let event_type = internal_key_event.event_type;
        let menubar = self.menubar.borrow().as_ref().and_then(vtable::VWeak::upgrade);

        if (event_type == KeyEventType::KeyReleased || event_type == KeyEventType::KeyPressed)
            && let Some(menubar) = menubar
        {
            let shortcuts = self.pinned_fields.as_ref().project_ref().menubar_shortcuts.get();
            let mut matches = shortcuts
                .into_iter()
                .filter(|entry| entry.shortcut.matches(&internal_key_event.key_event));
            if let Some(entry) = matches.next() {
                if internal_key_event.event_type == KeyEventType::KeyPressed {
                    VRc::borrow(&menubar).activate(&entry);
                    if matches.next().is_some() {
                        crate::debug_log!(
                            "Warning: Ambiguous menubar shortcut: {}",
                            entry.shortcut
                        );
                    }
                }
                return crate::input::KeyEventResult::EventAccepted;
            }
        }
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

        TextCursorBlinker::set_binding(
            blinker,
            prop,
            self.context().platform().cursor_flash_cycle(),
        );
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
        self.focus_item_visibility_tracker.clear();
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
                self.track_focus_item_visibility(item);
                let result = item.borrow().as_ref().focus_event(
                    &FocusEvent::FocusIn(reason),
                    &self.window_adapter(),
                    item,
                );
                // Reveal offscreen item when it gains focus
                if result == crate::input::FocusEventResult::FocusAccepted {
                    item.try_scroll_into_visible();
                }

                result
            }
            None => {
                self.focus_item_visibility_tracker.clear();
                *self.focus_item.borrow_mut() = Default::default();
                crate::input::FocusEventResult::FocusAccepted // We were removing the focus, treat that as OK
            }
        }
    }

    fn track_focus_item_visibility(&self, item: &ItemRc) {
        let visibility_clips = item.visibility_clips();
        self.focus_item_visibility_tracker.init(
            (item.downgrade(), self.window_adapter_weak.clone(), visibility_clips),
            |(_, _, visibility_clips)| {
                visibility_clips
                    .iter()
                    .all(|clip| clip.upgrade().is_some_and(|clip| !clip.as_pin_ref().clip()))
            },
            |(item, window_adapter, _), visible| {
                if *visible {
                    return;
                }
                let Some(item) = item.upgrade() else { return };
                let Some(window_adapter) = window_adapter.upgrade() else { return };
                WindowInner::from_pub(window_adapter.window()).set_focus_item(
                    &item,
                    false,
                    FocusReason::Programmatic,
                );
            },
        );
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
            let can_receive_focus = match reason {
                FocusReason::Programmatic => true,
                FocusReason::TabNavigation => current_item.is_visible_or_clipped_by_flickable(),
                _ => current_item.is_visible(),
            };
            if can_receive_focus
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
            self.context().0.modifiers.take();
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

    /// Re-evaluates the position tracker for the popup with the given ID, re-subscribing to its
    /// property dependencies so subsequent changes continue to trigger notifications.
    fn update_popup_properties(&self, popup_id: NonZeroU32) {
        let offset = {
            let active_popups = self.active_popups.borrow();
            let Some(popup) = active_popups.iter().find(|p| p.popup_id == popup_id) else { return };
            if let Some(parent) = popup.parent_item.clone().upgrade() {
                parent.map_to_native_window(
                    parent.geometry().origin + (popup.position_access)().to_euclid().to_vector(),
                )
            } else {
                LogicalPoint::zero()
            }
        };
        let mut active_popups = self.active_popups.borrow_mut();
        let Some(popup) = active_popups.iter_mut().find(|p| p.popup_id == popup_id) else { return };
        match &mut popup.location {
            PopupWindowLocation::ChildWindow(old_location) => {
                let (old_popup_region, new_popup_region) =
                    popup.properties_tracker.as_ref().evaluate_as_dependency_root(|| {
                        let component = ItemTreeRc::borrow_pin(&popup.component);
                        let root_item = component.as_ref().get_item_ref(0);
                        let window_item =
                            ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
                                .expect("Popup component is a Window item");
                        // Access the properties to set them as dependencies
                        let old_popup_region = LogicalRect::new(
                            *old_location,
                            crate::lengths::LogicalSize::new(
                                window_item.width().0,
                                window_item.height().0,
                            ),
                        );

                        let width = {
                            let layout_info_h = component
                                .as_ref()
                                .layout_info(crate::layout::Orientation::Horizontal);
                            let w = layout_info_h.min.min(layout_info_h.max);
                            window_item.width.set(LogicalLength::new(w));
                            w
                        };

                        let height = {
                            let layout_info_v = component
                                .as_ref()
                                .layout_info(crate::layout::Orientation::Vertical);
                            let h = layout_info_v.min.min(layout_info_v.max);
                            window_item.height.set(LogicalLength::new(h));
                            h
                        };

                        let clip_region = Some(LogicalRect::new(
                            LogicalPoint::new(0.0 as crate::Coord, 0.0 as crate::Coord),
                            self.window_adapter()
                                .size()
                                .to_logical(self.scale_factor())
                                .to_euclid(),
                        ));

                        let new_region_clipped = popup::place_popup(
                            popup::Placement::Fixed(LogicalRect::new(
                                offset,
                                crate::lengths::LogicalSize::new(width, height),
                            )),
                            &clip_region,
                        );

                        (old_popup_region, new_region_clipped)
                    });

                self.window_adapter().request_redraw();

                // Set new location
                *old_location = new_popup_region.origin;

                if let Some(adapter) = self.window_adapter_weak.upgrade() {
                    if !old_popup_region.is_empty() {
                        adapter.renderer().mark_dirty_region(old_popup_region.into());
                    }

                    if !new_popup_region.is_empty() {
                        adapter.renderer().mark_dirty_region(new_popup_region.into());
                    }
                    adapter.request_redraw();
                }
            }
            PopupWindowLocation::TopLevel(adapter) => {
                // The size is already tracked in the windowadapter
                let mut new_position: Option<LogicalPosition> = None;
                popup.properties_tracker.as_ref().evaluate_as_dependency_root(|| {
                    (popup.position_access)(); // Dummy access to track position changes
                    new_position = Some(LogicalPosition::from_euclid(offset));
                });
                if let Some(pos) = new_position {
                    adapter.window().set_position(pos);
                }
            }
        }
    }

    /// Calls the render_components to render the main component and any sub-window components, tracked by a
    /// property dependency tracker.
    ///
    /// The closure also receives a `post_render` callback. The renderer must invoke it
    /// once with its `ItemRenderer` after walking the components but before flushing,
    /// so the runtime can draw overlays that sit on top of the scene without being part
    /// of any item tree.
    ///
    /// Returns None if no component is set yet.
    pub fn draw_contents<T>(
        &self,
        render_components: impl FnOnce(
            &[(ItemTreeWeak, LogicalPoint)],
            &dyn Fn(&mut dyn crate::item_rendering::ItemRenderer),
        ) -> T,
    ) -> Option<T> {
        crate::properties::evaluate_no_tracking(|| self.ensure_tree_instantiated());
        let component_weak = ItemTreeRc::downgrade(&self.try_component()?);
        let post_render = |renderer: &mut dyn crate::item_rendering::ItemRenderer| {
            self.render_drag_image_overlay(renderer);
        };
        Some(self.pinned_fields.as_ref().project_ref().redraw_tracker.evaluate_as_dependency_root(
            || {
                if !self
                    .active_popups
                    .borrow()
                    .iter()
                    .any(|p| matches!(p.location, PopupWindowLocation::ChildWindow(..)))
                {
                    render_components(&[(component_weak, LogicalPoint::default())], &post_render)
                } else {
                    let borrow = self.active_popups.borrow();
                    let mut item_trees = Vec::with_capacity(borrow.len() + 1);
                    item_trees.push((component_weak, LogicalPoint::default()));
                    for popup in borrow.iter() {
                        // If the popup is not a real window and does not have its own coordinate system.
                        // We have to draw the popup and consider the location for subelements because everything must
                        // be rendered relative to the main window position
                        if let PopupWindowLocation::ChildWindow(location) = &popup.location {
                            item_trees.push((ItemTreeRc::downgrade(&popup.component), *location));
                        }
                    }
                    drop(borrow);
                    render_components(&item_trees, &post_render)
                }
            },
        ))
    }

    /// Draws the source `DragArea`'s `drag-image` under the cursor when a drag is in flight.
    /// No-op when no drag is active or the source has no image set.
    ///
    /// Marks the painted rect dirty for partial renderers, so the next frame clears the area
    /// before redrawing — same trick the linuxkms cursor injection uses, no per-frame state needed.
    fn render_drag_image_overlay(
        &self,
        item_renderer: &mut dyn crate::item_rendering::ItemRenderer,
    ) {
        let state = self.mouse_input_state.take();
        let cursor = state.drag_data.as_ref().map(|d| d.event.position);
        let source = state.drag_source.as_ref().and_then(|w| w.upgrade());
        self.mouse_input_state.set(state);

        let (Some(cursor), Some(source)) = (cursor, source) else { return };
        let Some(drag_area) = source.downcast::<crate::items::DragArea>() else { return };
        let drag_area = drag_area.as_pin_ref();
        let image = drag_area.drag_image();
        let size = crate::lengths::LogicalSize::from_untyped(image.size().cast());
        if size.is_empty() {
            return;
        }
        let cursor = crate::lengths::logical_point_from_api(cursor);
        let offset = LogicalVector::new(
            drag_area.drag_image_offset_x() as Coord,
            drag_area.drag_image_offset_y() as Coord,
        );
        let top_left = cursor - offset;

        item_renderer.save_state();
        item_renderer.translate(top_left.to_vector());
        item_renderer.draw_image_direct(image);
        item_renderer.restore_state();

        self.window_adapter().renderer().mark_dirty_region(LogicalRect::new(top_left, size).into());
    }

    /// Registers the window with the windowing system, in order to render the component's items and react
    /// to input events once the event loop spins.
    pub fn show(&self) -> Result<(), PlatformError> {
        if let Some(component) = self.try_component() {
            let was_visible = self.strong_component_ref.replace(Some(component)).is_some();
            if !was_visible {
                self.context().acquire_keepalive();
            }
        }

        self.ensure_tree_instantiated();
        self.update_window_properties();
        self.window_adapter().set_visible(true)?;
        // Make sure that the window's inner size is in sync with the root window item's
        // width/height.
        let size = self.window_adapter().size();
        let scale_factor = self.scale_factor();
        self.set_window_item_geometry(size.to_logical(scale_factor).to_euclid());
        let inset = self
            .window_adapter()
            .internal(crate::InternalToken)
            .map(|internal| internal.safe_area_inset())
            .unwrap_or_default();
        self.set_window_item_safe_area(inset.to_logical(scale_factor));
        self.window_adapter().renderer().resize(size).unwrap();
        if let Some(hook) = self.context().0.window_shown_hook.borrow_mut().as_mut() {
            hook(&self.window_adapter());
        }
        Ok(())
    }

    /// De-registers the window with the windowing system.
    pub fn hide(&self) -> Result<(), PlatformError> {
        let result = self.window_adapter().set_visible(false);
        let was_visible = self.strong_component_ref.borrow_mut().take().is_some();
        if was_visible {
            self.context().release_keepalive();
        }
        result
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

    /// Setup the shortcuts for the menubar
    /// Note: We still register the same shortcuts if the native menubar is active.
    /// Generally, the native menubar should capture the shortcuts first,
    /// but in case it doesn't, the window can still match them manually.
    pub fn setup_menubar_shortcuts(&self, menubar: VRc<MenuVTable>) {
        *self.menubar.borrow_mut() = Some(VRc::downgrade(&menubar));
        let weak = VRc::downgrade(&menubar);
        self.pinned_fields.menubar_shortcuts.set_binding(move || {
            fn flatten_menu(
                root: vtable::VRef<'_, MenuVTable>,
                parent: Option<&MenuEntry>,
            ) -> SharedVector<MenuEntry> {
                let mut menu_entries = Default::default();
                root.sub_menu(parent, &mut menu_entries);

                let mut result = menu_entries.clone();

                for entry in menu_entries {
                    result.extend(flatten_menu(root, Some(&entry)));
                }
                result
            }

            let Some(menubar) = weak.upgrade() else {
                return SharedVector::default();
            };
            flatten_menu(VRc::borrow(&menubar), None)
                .into_iter()
                .filter(|entry| entry.enabled && entry.shortcut != Keys::default())
                .collect()
        });
    }
    /// Create a new popup window adapter
    /// This window adapter can be used on a popup component and shown with show_popup()
    pub fn create_child_window_adapter(&self, kind: WindowKind) -> Option<Rc<dyn WindowAdapter>> {
        self.window_adapter()
            .internal(crate::InternalToken)
            .and_then(|s| s.create_child_window_adapter(kind))
    }

    /// Show a popup at the given position relative to the `parent_item` and returns its ID.
    /// The returned ID will always be non-zero.
    ///
    /// `is_open_setter` keeps the parent component's `PopupWindow::is-open` property in sync with this
    /// popup: it is invoked immediately with `true`, and again with `false` when the popup is closed
    /// through any path (the `Drop` impl of [`PopupWindow`] handles the `false`). Pass a no-op closure
    /// for popups (such as menus) that do not expose `is-open`.
    pub fn show_popup(
        &self,
        popup_componentrc: &ItemTreeRc,
        popup_access_position: Box<dyn Fn() -> LogicalPosition>,
        close_policy: PopupClosePolicy,
        parent_item: &ItemRc,
        window_kind: WindowKind,
        is_open_setter: Box<dyn Fn(bool)>,
    ) -> NonZeroU32 {
        // Popups live in their own ItemTree, which was invisible to any
        // earlier instantiation pass; materialize it before the layout queries below.
        crate::item_tree::ensure_item_tree_instantiated(popup_componentrc);
        let position = parent_item.map_to_native_window(
            parent_item.geometry().origin + popup_access_position().to_euclid().to_vector(),
        );
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
                crate::items::WindowItem::FIELD_OFFSETS.width().apply_pin(window_item);
            let height_property =
                crate::items::WindowItem::FIELD_OFFSETS.height().apply_pin(window_item);
            width_property.set(size.width_length());
            height_property.set(size.height_length());
        };

        let popup_id = self.next_popup_id.get();
        self.next_popup_id.set(popup_id.checked_add(1).unwrap());
        let parent_window_adapter_weak = Rc::downgrade(&self.window_adapter());

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

        let root_of = |mut item_tree: ItemTreeRc| loop {
            if ItemRc::new_root(item_tree.clone()).downcast::<crate::items::WindowItem>().is_some()
            {
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
        let parent_window_adapter = if let Some(parent_popup) = self
            .active_popups
            .borrow()
            .iter()
            .find(|p| ItemTreeRc::ptr_eq(&p.component, &parent_root_item_tree))
        {
            // Popup in a popup
            match &parent_popup.location {
                PopupWindowLocation::TopLevel(wa) => wa.clone(),
                PopupWindowLocation::ChildWindow(_) => self.window_adapter(),
            }
        } else {
            self.window_adapter()
        };

        let popup_window_adapter = {
            let mut popup_window_adapter = None;
            ItemTreeRc::borrow_pin(popup_componentrc)
                .as_ref()
                .window_adapter(false, &mut popup_window_adapter);
            popup_window_adapter.expect("It must be there because we set the global")
        };

        // If the window adapter of the popup window and the parent window are equal, create a ChildWindow
        // because we weren't able to create a dedicated popup adapter (for example if the backend does not support it).
        let (location, properties_tracker) =
            if Rc::ptr_eq(&parent_window_adapter, &popup_window_adapter) {
                // Tooltips may extend past the window (e.g. above/left of the anchor); do not clamp.
                let clip_region = Some(LogicalRect::new(
                    LogicalPoint::new(0.0 as crate::Coord, 0.0 as crate::Coord),
                    self.window_adapter().size().to_logical(self.scale_factor()).to_euclid(),
                ));
                let rect = popup::place_popup(
                    popup::Placement::Fixed(LogicalRect::new(position, size)),
                    &clip_region,
                );
                self.window_adapter().request_redraw();
                (
                    PopupWindowLocation::ChildWindow(rect.origin),
                    Box::pin(PropertyTracker::new_with_dirty_handler(
                        PopupWindowPropertiesTracker {
                            parent_window_adapter_weak: parent_window_adapter_weak.clone(),
                            popup_id,
                        },
                    )),
                )
            } else {
                let popup_window = popup_window_adapter.window();
                WindowInner::from_pub(popup_window).set_component(popup_componentrc);
                popup_window.set_position(LogicalPosition::from_euclid(position));
                popup_window.set_size(WindowSize::Logical(LogicalSize::from_euclid(size)));

                popup_window_adapter.set_visible(true).expect("unable to show popup window");
                (
                    PopupWindowLocation::TopLevel(popup_window_adapter),
                    Box::pin(PropertyTracker::new_with_dirty_handler(
                        PopupWindowPropertiesTracker {
                            parent_window_adapter_weak: parent_window_adapter_weak.clone(),
                            popup_id,
                        },
                    )),
                )
            };

        let focus_item = if matches!(window_kind, WindowKind::ToolTip) {
            Default::default()
        } else {
            self.take_focus_item(&FocusEvent::FocusOut(FocusReason::PopupActivation))
                .map(|item| item.downgrade())
                .unwrap_or_default()
        };

        // Reflect the freshly shown popup in the parent's `is-open` property; the matching `false` is
        // emitted when the stored `PopupWindow` is dropped (see its `Drop` impl), which every close
        // path funnels through. Called before the popup is stored so we do not hold a borrow on
        // `active_popups` while running user-provided code.
        is_open_setter(true);

        self.active_popups.borrow_mut().push(PopupWindow {
            popup_id,
            location,
            component: popup_componentrc.clone(),
            close_policy,
            focus_item_in_parent: focus_item,
            parent_item: parent_item.downgrade(),
            window_kind,
            position_access: popup_access_position,
            is_open_setter,
            properties_tracker,
        });

        self.update_popup_properties(popup_id);

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
            let position = parent_item.map_to_native_window(
                parent_item.geometry().origin + position.to_euclid().to_vector(),
            );
            let position = crate::lengths::logical_position_to_api(position);
            x.show_native_popup_menu(context_menu_item, position)
        } else {
            false
        }
    }

    // Close the popup associated with the given popup window.
    // The parent's `is-open` property is reset to false when `current_popup` is dropped (see the
    // `Drop` impl for `PopupWindow`), which every close path eventually does.
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
            if matches!(p.window_kind, WindowKind::Menu) {
                // close all sub-menus
                while self
                    .active_popups
                    .borrow()
                    .get(popup_index)
                    .is_some_and(|p| matches!(p.window_kind, WindowKind::Menu))
                {
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
        if !self.pinned_fields.scale_factor.is_constant() {
            self.pinned_fields.scale_factor.set(factor)
        }
    }

    /// Sets the scale factor for the window.
    /// From that point on, the scale factor is constant and cannot be changed anymore.
    pub fn set_const_scale_factor(&self, factor: f32) {
        if !self.pinned_fields.scale_factor.is_constant() {
            self.pinned_fields.scale_factor.set(factor);
            self.pinned_fields.scale_factor.set_constant();
        }
    }

    /// Reads the global property `TextInputInterface.text-input-focused`
    pub fn text_input_focused(&self) -> bool {
        self.pinned_fields.as_ref().project_ref().text_input_focused.get()
    }

    /// Sets the global property `TextInputInterface.text-input-focused`
    pub fn set_text_input_focused(&self, value: bool) {
        if !value && let Some(window_adapter) = self.window_adapter().internal(crate::InternalToken)
        {
            window_adapter.input_method_request(InputMethodRequest::Disable);
        }
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
            let item_rc = ItemRc::new_root(component_rc);
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
            ItemRc::new_root(component_rc).downcast::<crate::items::WindowItem>()
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

    /// The safe area of the window has changed.
    pub fn set_window_item_safe_area(&self, inset: crate::lengths::LogicalEdges) {
        if let Some(component_rc) = self.try_component() {
            let component = ItemTreeRc::borrow_pin(&component_rc);
            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) = ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
            {
                window_item.safe_area_insets.set(inset);
            }
        }
    }

    pub(crate) fn set_window_item_virtual_keyboard(
        &self,
        origin: crate::lengths::LogicalPoint,
        size: crate::lengths::LogicalSize,
    ) {
        let Some(component_rc) = self.try_component() else {
            return;
        };
        let component = ItemTreeRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);
        let Some(window_item) = ItemRef::downcast_pin::<crate::items::WindowItem>(root_item) else {
            return;
        };
        window_item.virtual_keyboard_position.set(origin);
        window_item.virtual_keyboard_size.set(size);
        if let Some(focus_item) = self.focus_item.borrow().upgrade() {
            focus_item.try_scroll_into_visible();
        }
    }

    // Get geometry of the virtual keyboard if available
    pub(crate) fn window_item_virtual_keyboard(
        &self,
    ) -> Option<(crate::lengths::LogicalPoint, crate::lengths::LogicalSize)> {
        let component_rc = self.try_component()?;
        let component = ItemTreeRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);
        let window_item = ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)?;
        let keyboard_size = window_item.virtual_keyboard_size();
        if keyboard_size.width == 0. as Coord || keyboard_size.height == 0. as Coord {
            None
        } else {
            Some((window_item.virtual_keyboard_position(), keyboard_size))
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

    /// Returns if the window is currently in fullscreen mode
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
        self.window_item().is_some_and(|window_item| window_item.as_pin_ref().maximized())
    }

    /// Set the window as maximized or unmaximized
    pub fn set_maximized(&self, maximized: bool) {
        if let Some(window_item) = self.window_item() {
            window_item.as_pin_ref().maximized.set(maximized);
            self.update_window_properties()
        }
    }

    /// Returns if the window is currently minimized
    pub fn is_minimized(&self) -> bool {
        self.window_item().is_some_and(|window_item| window_item.as_pin_ref().minimized())
    }

    /// Set the window as minimized or unminimized
    pub fn set_minimized(&self, minimized: bool) {
        if let Some(window_item) = self.window_item() {
            window_item.as_pin_ref().minimized.set(minimized);
            self.update_window_properties()
        }
    }

    /// Returns the (context global) xdg app id for use with wayland and x11.
    pub fn xdg_app_id(&self) -> Option<SharedString> {
        self.context().xdg_app_id()
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
        self.ctx
            .get_or_init(|| crate::context::GLOBAL_CONTEXT.with(|ctx| ctx.get().unwrap().clone()))
    }

    /// Like [`Self::context`], but returns `None` instead of panicking when no context is
    /// available yet.
    pub fn try_context(&self) -> Option<&crate::SlintContext> {
        if self.ctx.get().is_none()
            && let Some(ctx) = crate::context::GLOBAL_CONTEXT.with(|ctx| ctx.get().cloned())
        {
            let _ = self.ctx.set(ctx);
        }
        self.ctx.get()
    }

    /// Set the SlintContext.
    /// This needs to be called once before any other functions that would use the context.
    pub fn set_context(&self, ctx: crate::SlintContext) {
        self.ctx.set(ctx).map_err(|_| ()).expect("context shouldn't have been set before")
    }
}

/// Internal alias for `Rc<dyn WindowAdapter>`.
pub type WindowAdapterRc = Rc<dyn WindowAdapter>;

/// Resolve the [`crate::SlintContext`] associated with a component root by
/// asking it for (or creating) its window adapter and reading the context off
/// the resulting window. Returns `None` only when no adapter can be produced.
pub fn context_for_root(root: &ItemTreeRc) -> Option<crate::SlintContext> {
    let comp_ref_pin = vtable::VRc::borrow_pin(root);
    let mut adapter = None;
    comp_ref_pin.as_ref().window_adapter(true, &mut adapter);
    adapter.map(|a| WindowInner::from_pub(a.window()).context().clone())
}

/// Runtime entry point for `BuiltinFunction::AccentColor`. Returns the accent color
/// from the component's [`crate::SlintContext`] reached via its window adapter, or
/// transparent if none is associated.
pub fn accent_color(root: &crate::item_tree::ItemTreeRc) -> crate::graphics::Color {
    let comp_ref_pin = vtable::VRc::borrow_pin(root);
    let mut adapter = None;
    comp_ref_pin.as_ref().window_adapter(true, &mut adapter);
    adapter.map_or(crate::graphics::Color::default(), |a| {
        WindowInner::from_pub(a.window()).context().accent_color()
    })
}

/// This module contains the functions needed to interface with the event loop and window traits
/// from outside the Rust language.
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]
    #![allow(clippy::missing_safety_doc)]
    #![allow(missing_docs)]

    use super::*;
    use crate::SharedVector;
    use crate::api::{RenderingNotifier, RenderingState, SetRenderingNotifierError};
    use crate::graphics::Size;
    use crate::graphics::{IntSize, Rgba8Pixel};
    use crate::items::WindowItem;
    use core::ffi::c_void;

    /// This enum describes a low-level access to specific graphics APIs used
    /// by the renderer.
    #[repr(u8)]
    pub enum GraphicsAPI {
        /// The rendering is done using OpenGL.
        NativeOpenGL,
        /// The rendering is done using APIs inaccessible from C++, such as WGPU.
        Inaccessible,
    }

    struct WithUserData<T> {
        callback: T,
        drop_user_data: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
    }

    impl<T> Drop for WithUserData<T> {
        fn drop(&mut self) {
            (self.drop_user_data)(self.user_data)
        }
    }

    impl WithUserData<extern "C" fn(user_data: *mut c_void, pos: &mut LogicalPosition)> {
        fn call(&self) -> LogicalPosition {
            let mut logical_position = LogicalPosition::default();
            (self.callback)(self.user_data, &mut logical_position);
            logical_position
        }
    }

    impl WithUserData<extern "C" fn(user_data: *mut c_void) -> CloseRequestResponse> {
        fn call(&self) -> CloseRequestResponse {
            (self.callback)(self.user_data)
        }
    }

    impl WithUserData<extern "C" fn(user_data: *mut c_void, is_open: bool)> {
        fn call(&self, is_open: bool) {
            (self.callback)(self.user_data, is_open)
        }
    }

    /// Same layout as WindowAdapterRc
    #[repr(C)]
    pub struct WindowAdapterRcOpaque(*const c_void, *const c_void);

    /// Releases the reference to the windowrc held by handle.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_drop(handle: *mut WindowAdapterRcOpaque) {
        unsafe {
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
    }

    /// Releases the reference to the component window held by handle.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_clone(
        source: *const WindowAdapterRcOpaque,
        target: *mut WindowAdapterRcOpaque,
    ) {
        unsafe {
            assert_eq!(
                core::mem::size_of::<Rc<dyn WindowAdapter>>(),
                core::mem::size_of::<WindowAdapterRcOpaque>()
            );
            let window = &*(source as *const Rc<dyn WindowAdapter>);
            core::ptr::write(target as *mut Rc<dyn WindowAdapter>, window.clone());
        }
    }

    /// Ensure repeaters, conditionals and component containers are instantiated.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_ensure_tree_instantiated(
        handle: *const WindowAdapterRcOpaque,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).ensure_tree_instantiated();
        }
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_show(handle: *const WindowAdapterRcOpaque) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);

            window_adapter.window().show().unwrap();
        }
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_hide(handle: *const WindowAdapterRcOpaque) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().hide().unwrap();
        }
    }

    /// Returns the visibility state of the window. This function can return false even if you previously called show()
    /// on it, for example if the user minimized the window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_visible(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        unsafe {
            let window = &*(handle as *const Rc<dyn WindowAdapter>);
            window.window().is_visible()
        }
    }

    /// Returns the window scale factor.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_get_scale_factor(
        handle: *const WindowAdapterRcOpaque,
    ) -> f32 {
        unsafe {
            assert_eq!(
                core::mem::size_of::<Rc<dyn WindowAdapter>>(),
                core::mem::size_of::<WindowAdapterRcOpaque>()
            );
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).scale_factor()
        }
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_const_scale_factor(
        handle: *const WindowAdapterRcOpaque,
        value: f32,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).set_const_scale_factor(value)
        }
    }

    /// Returns the text-input-focused property value.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_get_text_input_focused(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        unsafe {
            assert_eq!(
                core::mem::size_of::<Rc<dyn WindowAdapter>>(),
                core::mem::size_of::<WindowAdapterRcOpaque>()
            );
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).text_input_focused()
        }
    }

    /// Set the text-input-focused property.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_text_input_focused(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).set_text_input_focused(value)
        }
    }

    /// Sets the focus item.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_focus_item(
        handle: *const WindowAdapterRcOpaque,
        focus_item: &ItemRc,
        set_focus: bool,
        reason: FocusReason,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window())
                .set_focus_item(focus_item, set_focus, reason)
        }
    }

    /// Associates the window with the given component.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_component(
        handle: *const WindowAdapterRcOpaque,
        component: &ItemTreeRc,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).set_component(component)
        }
    }

    /// Show a popup and return its ID. The returned ID will always be non-zero.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_show_popup(
        handle: *const WindowAdapterRcOpaque,
        popup: &ItemTreeRc,
        position: extern "C" fn(user_data: *mut c_void, pos: &mut LogicalPosition),
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        user_data: *mut c_void,
        close_policy: PopupClosePolicy,
        parent_item: &ItemRc,
        window_kind: WindowKind,
        is_open_setter: extern "C" fn(user_data: *mut c_void, is_open: bool),
        is_open_setter_drop_user_data: extern "C" fn(user_data: *mut c_void),
        is_open_setter_user_data: *mut c_void,
    ) -> NonZeroU32 {
        unsafe {
            let with_user_data = WithUserData { callback: position, drop_user_data, user_data };
            let is_open_with_user_data = WithUserData {
                callback: is_open_setter,
                drop_user_data: is_open_setter_drop_user_data,
                user_data: is_open_setter_user_data,
            };
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).show_popup(
                popup,
                Box::new(move || with_user_data.call()),
                close_policy,
                parent_item,
                window_kind,
                Box::new(move |is_open| is_open_with_user_data.call(is_open)),
            )
        }
    }

    /// Create a popup window adapter. Returns true if a new adapter was created and written to result.
    /// Returns false if the backend does not support top-level popups.
    /// This can be used to set the correct window adapter on a popup component before showing it.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_create_child_window_adapter(
        handle: *const WindowAdapterRcOpaque,
        window_kind: WindowKind,
        result: *mut WindowAdapterRcOpaque,
    ) -> bool {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            match WindowInner::from_pub(window_adapter.window())
                .create_child_window_adapter(window_kind)
            {
                Some(wa) => {
                    core::ptr::write(result as *mut Rc<dyn WindowAdapter>, wa);
                    true
                }
                None => false,
            }
        }
    }

    /// Close the popup by the given ID.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_close_popup(
        handle: *const WindowAdapterRcOpaque,
        popup_id: NonZeroU32,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).close_popup(popup_id);
        }
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
        unsafe {
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
                fn notify(
                    &mut self,
                    state: RenderingState,
                    graphics_api: &crate::api::GraphicsAPI,
                ) {
                    let cpp_graphics_api = match graphics_api {
                        crate::api::GraphicsAPI::NativeOpenGL { .. } => GraphicsAPI::NativeOpenGL,
                        crate::api::GraphicsAPI::WebGL { .. } => unreachable!(), // We don't support wasm with C++
                        #[cfg(feature = "unstable-wgpu-29")]
                        crate::api::GraphicsAPI::WGPU29 { .. } => GraphicsAPI::Inaccessible, // There is no C++ API for wgpu (maybe wgpu c in the future?)
                        #[cfg(feature = "unstable-wgpu-30")]
                        crate::api::GraphicsAPI::WGPU30 { .. } => GraphicsAPI::Inaccessible, // There is no C++ API for wgpu (maybe wgpu c in the future?)
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
    }

    /// C binding to the on_close_requested() API of Window
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_on_close_requested(
        handle: *const WindowAdapterRcOpaque,
        callback: extern "C" fn(user_data: *mut c_void) -> CloseRequestResponse,
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        user_data: *mut c_void,
    ) {
        unsafe {
            let with_user_data = WithUserData { callback, drop_user_data, user_data };
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().on_close_requested(move || with_user_data.call());
        }
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_request_redraw(handle: *const WindowAdapterRcOpaque) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.request_redraw();
        }
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &mut euclid::default::Point2D<i32>,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            *pos = window_adapter.position().unwrap_or_default().to_euclid()
        }
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_physical_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &euclid::default::Point2D<i32>,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.set_position(crate::api::PhysicalPosition::new(pos.x, pos.y).into());
        }
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_logical_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &euclid::default::Point2D<f32>,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.set_position(LogicalPosition::new(pos.x, pos.y).into());
        }
    }

    /// Returns the size of the window on the screen, in physical screen coordinates and excluding
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_size(handle: *const WindowAdapterRcOpaque) -> IntSize {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.size().to_euclid().cast()
        }
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_physical_size(
        handle: *const WindowAdapterRcOpaque,
        size: &IntSize,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter
                .window()
                .set_size(crate::api::PhysicalSize::new(size.width, size.height));
        }
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_logical_size(
        handle: *const WindowAdapterRcOpaque,
        size: &Size,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().set_size(crate::api::LogicalSize::new(size.width, size.height));
        }
    }

    /// Return whether the platform supports native menu bars
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_supports_native_menu_bar(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter
                .internal(crate::InternalToken)
                .is_some_and(|x| x.supports_native_menu_bar())
        }
    }

    /// Setup the native menu bar
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_setup_native_menu_bar(
        handle: *const WindowAdapterRcOpaque,
        menu_instance: &vtable::VRc<MenuVTable>,
    ) {
        let window_adapter = unsafe { &*(handle as *const Rc<dyn WindowAdapter>) };
        let window = window_adapter.window();
        window.0.setup_menubar(vtable::VRc::clone(menu_instance));
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_setup_menu_bar_shortcuts(
        handle: *const WindowAdapterRcOpaque,
        menu_instance: &vtable::VRc<MenuVTable>,
    ) {
        let window_adapter = unsafe { &*(handle as *const Rc<dyn WindowAdapter>) };
        let window = window_adapter.window();
        window.0.setup_menubar_shortcuts(vtable::VRc::clone(menu_instance));
    }

    /// Show a native context menu
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_show_native_popup_menu(
        handle: *const WindowAdapterRcOpaque,
        context_menu: &vtable::VRc<MenuVTable>,
        position: LogicalPosition,
        parent_item: &ItemRc,
    ) -> bool {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            WindowInner::from_pub(window_adapter.window()).show_native_popup_menu(
                context_menu.clone(),
                position,
                parent_item,
            )
        }
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
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().0.process_key_input(InternalKeyEvent {
                event_type,
                key_event: crate::items::KeyEvent {
                    text: text.clone(),
                    repeat,
                    ..Default::default()
                },
                ..Default::default()
            });
        }
    }

    /// Dispatch a mouse event
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_dispatch_pointer_event(
        handle: *const WindowAdapterRcOpaque,
        event: &crate::input::MouseEvent,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().0.process_mouse_input(event.clone());
        }
    }

    /// Dispatch a window event
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_dispatch_event(
        handle: *const WindowAdapterRcOpaque,
        event: &crate::platform::WindowEvent,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().dispatch_event(event.clone());
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_fullscreen(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().is_fullscreen()
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_minimized(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().is_minimized()
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_is_maximized(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().is_maximized()
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_fullscreen(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().set_fullscreen(value)
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_minimized(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().set_minimized(value)
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_set_maximized(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        unsafe {
            let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
            window_adapter.window().set_maximized(value)
        }
    }

    /// Takes a snapshot of the window contents and returns it as RGBA8 encoded pixel buffer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_take_snapshot(
        handle: *const WindowAdapterRcOpaque,
        data: &mut SharedVector<Rgba8Pixel>,
        width: &mut u32,
        height: &mut u32,
    ) -> bool {
        unsafe {
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
}

/// This module contains the functions needed to interface with window handles from outside the Rust language.
#[cfg(all(feature = "ffi", feature = "raw-window-handle-06"))]
pub mod ffi_window {
    #![allow(unsafe_code)]
    #![allow(clippy::missing_safety_doc)]

    use super::ffi::WindowAdapterRcOpaque;
    use super::*;
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use std::sync::Arc;

    /// Helper to grab the `HasWindowHandle` for the `WindowAdapter` behind `handle`.
    fn has_window_handle(
        handle: *const WindowAdapterRcOpaque,
    ) -> Option<Arc<dyn raw_window_handle_06::HasWindowHandle>> {
        let window_adapter = unsafe { &*(handle as *const Rc<dyn WindowAdapter>) };
        let window_adapter = window_adapter.internal(crate::InternalToken)?;
        window_adapter.window_handle_06_rc().ok()
    }

    /// Helper to grab the `HasDisplayHandle` for the `WindowAdapter` behind `handle`.
    fn has_display_handle(
        handle: *const WindowAdapterRcOpaque,
    ) -> Option<Arc<dyn raw_window_handle_06::HasDisplayHandle>> {
        let window_adapter = unsafe { &*(handle as *const Rc<dyn WindowAdapter>) };
        let window_adapter = window_adapter.internal(crate::InternalToken)?;
        window_adapter.display_handle_06_rc().ok()
    }

    /// Returns the `HWND` associated with this window, or null if it doesn't exist or isn't created yet.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_hwnd_win32(
        handle: *const WindowAdapterRcOpaque,
    ) -> *mut c_void {
        use raw_window_handle_06::HasWindowHandle;

        if let Some(has_window_handle) = has_window_handle(handle)
            && let Ok(window_handle) = has_window_handle.window_handle()
            && let raw_window_handle_06::RawWindowHandle::Win32(win32) = window_handle.as_raw()
        {
            isize::from(win32.hwnd) as *mut c_void
        } else {
            null_mut()
        }
    }

    /// Returns the `HINSTANCE` associated with this window, or null if it doesn't exist or isn't created yet.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_hinstance_win32(
        handle: *const WindowAdapterRcOpaque,
    ) -> *mut c_void {
        use raw_window_handle_06::HasWindowHandle;

        if let Some(has_window_handle) = has_window_handle(handle)
            && let Ok(window_handle) = has_window_handle.window_handle()
            && let raw_window_handle_06::RawWindowHandle::Win32(win32) = window_handle.as_raw()
        {
            win32
                .hinstance
                .map(|hinstance| isize::from(hinstance) as *mut c_void)
                .unwrap_or_default()
        } else {
            null_mut()
        }
    }

    /// Returns the `wl_surface` associated with this window, or null if it doesn't exist or isn't created yet.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_wlsurface_wayland(
        handle: *const WindowAdapterRcOpaque,
    ) -> *mut c_void {
        use raw_window_handle_06::HasWindowHandle;

        if let Some(has_window_handle) = has_window_handle(handle)
            && let Ok(window_handle) = has_window_handle.window_handle()
            && let raw_window_handle_06::RawWindowHandle::Wayland(wayland) = window_handle.as_raw()
        {
            wayland.surface.as_ptr()
        } else {
            null_mut()
        }
    }

    /// Returns the `wl_display` associated with this window, or null if it doesn't exist or isn't created yet.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_wldisplay_wayland(
        handle: *const WindowAdapterRcOpaque,
    ) -> *mut c_void {
        use raw_window_handle_06::HasDisplayHandle;

        if let Some(has_display_handle) = has_display_handle(handle)
            && let Ok(display_handle) = has_display_handle.display_handle()
            && let raw_window_handle_06::RawDisplayHandle::Wayland(wayland) =
                display_handle.as_raw()
        {
            wayland.display.as_ptr()
        } else {
            null_mut()
        }
    }

    /// Returns the `NSView` associated with this window, or null if it doesn't exist or isn't created yet.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_nsview_appkit(
        handle: *const WindowAdapterRcOpaque,
    ) -> *mut c_void {
        use raw_window_handle_06::HasWindowHandle;

        if let Some(has_window_handle) = has_window_handle(handle)
            && let Ok(window_handle) = has_window_handle.window_handle()
            && let raw_window_handle_06::RawWindowHandle::AppKit(appkit) = window_handle.as_raw()
        {
            appkit.ns_view.as_ptr()
        } else {
            null_mut()
        }
    }
}
