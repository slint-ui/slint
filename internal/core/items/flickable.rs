// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The implementation details behind the Flickable

//! The `Flickable` item

use super::{
    Item, ItemConsts, ItemRc, ItemRendererRef, KeyEventResult, PointerEventButton, RenderingResult,
    VoidArg,
};
use crate::animations::{EasingCurve, Instant};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::items::PropertyAnimation;
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    PointLengths, RectLengths,
};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::Callback;
use crate::Property;
use alloc::boxed::Box;
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::RefCell;
use core::pin::Pin;
use core::time::Duration;
#[allow(unused)]
use euclid::num::Ceil;
use euclid::num::Zero;
use i_slint_core_macros::*;
#[allow(unused)]
use num_traits::Float;

/// The implementation of the `Flickable` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct Flickable {
    pub viewport_x: Property<LogicalLength>,
    pub viewport_y: Property<LogicalLength>,
    pub viewport_width: Property<LogicalLength>,
    pub viewport_height: Property<LogicalLength>,

    pub interactive: Property<bool>,

    pub flicked: Callback<VoidArg>,

    data: FlickableDataBox,

    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Flickable {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if let Some(pos) = event.position() {
            let geometry = self_rc.geometry();
            if pos.x < 0 as _
                || pos.y < 0 as _
                || pos.x_length() > geometry.width_length()
                || pos.y_length() > geometry.height_length()
            {
                return InputEventFilterResult::Intercept;
            }
        }
        if !self.interactive() && !matches!(event, MouseEvent::Wheel { .. }) {
            return InputEventFilterResult::ForwardAndIgnore;
        }
        self.data.handle_mouse_filter(self, event, self_rc)
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.interactive() && !matches!(event, MouseEvent::Wheel { .. }) {
            return InputEventResult::EventIgnored;
        }
        if let Some(pos) = event.position() {
            let geometry = self_rc.geometry();
            if matches!(event, MouseEvent::Wheel { .. } | MouseEvent::Pressed { .. })
                && (pos.x < 0 as _
                    || pos.y < 0 as _
                    || pos.x_length() > geometry.width_length()
                    || pos.y_length() > geometry.height_length())
            {
                return InputEventResult::EventIgnored;
            }
        }

        self.data.handle_mouse(self, event, window_adapter, self_rc)
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).combine_clip(
            LogicalRect::new(LogicalPoint::default(), size),
            LogicalBorderRadius::zero(),
            LogicalLength::zero(),
        );
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
        true
    }
}

impl ItemConsts for Flickable {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

#[repr(C)]
/// Wraps the internal data structure for the Flickable
pub struct FlickableDataBox(core::ptr::NonNull<FlickableData>);

impl Default for FlickableDataBox {
    fn default() -> Self {
        FlickableDataBox(Box::leak(Box::<FlickableData>::default()).into())
    }
}
impl Drop for FlickableDataBox {
    fn drop(&mut self) {
        // Safety: the self.0 was constructed from a Box::leak in FlickableDataBox::default
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}

impl core::ops::Deref for FlickableDataBox {
    type Target = FlickableData;
    fn deref(&self) -> &Self::Target {
        // Safety: initialized in FlickableDataBox::default
        unsafe { self.0.as_ref() }
    }
}

/// The distance required before it starts flicking if there is another item intercepting the mouse.
pub(super) const DISTANCE_THRESHOLD: LogicalLength = LogicalLength::new(8 as _);
/// Time required before we stop caring about child event if the mouse hasn't been moved
pub(super) const DURATION_THRESHOLD: Duration = Duration::from_millis(500);
/// The delay to which press are forwarded to the inner item
pub(super) const FORWARD_DELAY: Duration = Duration::from_millis(100);

#[derive(Default, Debug)]
struct FlickableDataInner {
    /// The position in which the press was made
    pressed_pos: LogicalPoint,
    pressed_time: Option<Instant>,
    pressed_viewport_pos: LogicalPoint,
    /// Set to true if the flickable is flicking and capturing all mouse event, not forwarding back to the children
    capture_events: bool,
}

#[derive(Default, Debug)]
pub struct FlickableData {
    inner: RefCell<FlickableDataInner>,
}

impl FlickableData {
    fn handle_mouse_filter(
        &self,
        flick: Pin<&Flickable>,
        event: MouseEvent,
        flick_rc: &ItemRc,
    ) -> InputEventFilterResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::Pressed { position, button: PointerEventButton::Left, .. } => {
                inner.pressed_pos = position;
                inner.pressed_time = Some(crate::animations::current_tick());
                inner.pressed_viewport_pos = LogicalPoint::from_lengths(
                    (Flickable::FIELD_OFFSETS.viewport_x).apply_pin(flick).get(),
                    (Flickable::FIELD_OFFSETS.viewport_y).apply_pin(flick).get(),
                );
                if inner.capture_events {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::DelayForwarding(FORWARD_DELAY.as_millis() as _)
                }
            }
            MouseEvent::Exit | MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                let was_capturing = inner.capture_events;
                Self::mouse_released(&mut inner, flick, event, flick_rc);
                if was_capturing {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::Moved { position } => {
                let do_intercept = inner.capture_events
                    || inner.pressed_time.is_some_and(|pressed_time| {
                        if crate::animations::current_tick() - pressed_time > DURATION_THRESHOLD {
                            return false;
                        }
                        // Check if the mouse was moved more than the DISTANCE_THRESHOLD in a
                        // direction in which the flickable can flick
                        let diff = position - inner.pressed_pos;
                        let geo = flick_rc.geometry();
                        let w = geo.width_length();
                        let h = geo.height_length();
                        let vw = (Flickable::FIELD_OFFSETS.viewport_width).apply_pin(flick).get();
                        let vh = (Flickable::FIELD_OFFSETS.viewport_height).apply_pin(flick).get();
                        let x = (Flickable::FIELD_OFFSETS.viewport_x).apply_pin(flick).get();
                        let y = (Flickable::FIELD_OFFSETS.viewport_y).apply_pin(flick).get();
                        let zero = LogicalLength::zero();
                        ((vw > w || x != zero) && abs(diff.x_length()) > DISTANCE_THRESHOLD)
                            || ((vh > h || y != zero) && abs(diff.y_length()) > DISTANCE_THRESHOLD)
                    });
                if do_intercept {
                    InputEventFilterResult::Intercept
                } else if inner.pressed_time.is_some() {
                    InputEventFilterResult::ForwardAndInterceptGrab
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::Wheel { .. } => InputEventFilterResult::ForwardEvent,
            // Not the left button
            MouseEvent::Pressed { .. } | MouseEvent::Released { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
        }
    }

    fn handle_mouse(
        &self,
        flick: Pin<&Flickable>,
        event: MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        flick_rc: &ItemRc,
    ) -> InputEventResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::Pressed { .. } => {
                inner.capture_events = true;
                InputEventResult::GrabMouse
            }
            MouseEvent::Exit | MouseEvent::Released { .. } => {
                let was_capturing = inner.capture_events;
                Self::mouse_released(&mut inner, flick, event, flick_rc);
                if was_capturing {
                    InputEventResult::EventAccepted
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Moved { position } => {
                if inner.pressed_time.is_some() {
                    let new_pos = inner.pressed_viewport_pos + (position - inner.pressed_pos);
                    let x = (Flickable::FIELD_OFFSETS.viewport_x).apply_pin(flick);
                    let y = (Flickable::FIELD_OFFSETS.viewport_y).apply_pin(flick);
                    let should_capture = || {
                        let geo = flick_rc.geometry();
                        let w = geo.width_length();
                        let h = geo.height_length();
                        let vw = (Flickable::FIELD_OFFSETS.viewport_width).apply_pin(flick).get();
                        let vh = (Flickable::FIELD_OFFSETS.viewport_height).apply_pin(flick).get();
                        let zero = LogicalLength::zero();
                        ((vw > w || x.get() != zero)
                            && abs(x.get() - new_pos.x_length()) > DISTANCE_THRESHOLD)
                            || ((vh > h || y.get() != zero)
                                && abs(y.get() - new_pos.y_length()) > DISTANCE_THRESHOLD)
                    };

                    if inner.capture_events || should_capture() {
                        let new_pos = ensure_in_bound(flick, new_pos, flick_rc);

                        let old_pos = (x.get(), y.get());
                        x.set(new_pos.x_length());
                        y.set(new_pos.y_length());
                        if old_pos.0 != new_pos.x_length() || old_pos.1 != new_pos.y_length() {
                            (Flickable::FIELD_OFFSETS.flicked).apply_pin(flick).call(&());
                        }

                        inner.capture_events = true;
                        InputEventResult::GrabMouse
                    } else if abs(x.get() - new_pos.x_length()) > DISTANCE_THRESHOLD
                        || abs(y.get() - new_pos.y_length()) > DISTANCE_THRESHOLD
                    {
                        // drag in a unsupported direction gives up the grab
                        InputEventResult::EventIgnored
                    } else {
                        InputEventResult::EventAccepted
                    }
                } else {
                    inner.capture_events = false;
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Wheel { delta_x, delta_y, .. } => {
                let old_pos = LogicalPoint::from_lengths(
                    (Flickable::FIELD_OFFSETS.viewport_x).apply_pin(flick).get(),
                    (Flickable::FIELD_OFFSETS.viewport_y).apply_pin(flick).get(),
                );
                let delta = if window_adapter.window().0.modifiers.get().shift()
                    && !cfg!(target_os = "macos")
                {
                    // Shift invert coordinate for the purpose of scrolling. But not on macOs because there the OS already take care of the change
                    LogicalVector::new(delta_y, delta_x)
                } else {
                    LogicalVector::new(delta_x, delta_y)
                };
                let new_pos = ensure_in_bound(flick, old_pos + delta, flick_rc);

                let viewport_x = (Flickable::FIELD_OFFSETS.viewport_x).apply_pin(flick);
                let viewport_y = (Flickable::FIELD_OFFSETS.viewport_y).apply_pin(flick);
                let old_pos = (viewport_x.get(), viewport_y.get());
                viewport_x.set(new_pos.x_length());
                viewport_y.set(new_pos.y_length());
                if old_pos.0 != new_pos.x_length() || old_pos.1 != new_pos.y_length() {
                    (Flickable::FIELD_OFFSETS.flicked).apply_pin(flick).call(&());
                }
                InputEventResult::EventAccepted
            }
        }
    }

    fn mouse_released(
        inner: &mut FlickableDataInner,
        flick: Pin<&Flickable>,
        event: MouseEvent,
        flick_rc: &ItemRc,
    ) {
        if let (Some(pressed_time), Some(pos)) = (inner.pressed_time, event.position()) {
            let dist = (pos - inner.pressed_pos).cast::<f32>();

            let millis = (crate::animations::current_tick() - pressed_time).as_millis();
            if inner.capture_events
                && dist.square_length() > (DISTANCE_THRESHOLD.get() * DISTANCE_THRESHOLD.get()) as _
                && millis > 1
            {
                let speed = dist / (millis as f32);

                let duration = 250;
                let final_pos = ensure_in_bound(
                    flick,
                    (inner.pressed_viewport_pos.cast() + dist + speed * (duration as f32)).cast(),
                    flick_rc,
                );
                let anim = PropertyAnimation {
                    duration,
                    easing: EasingCurve::CubicBezier([0.0, 0.0, 0.58, 1.0]),
                    ..PropertyAnimation::default()
                };

                let viewport_x = (Flickable::FIELD_OFFSETS.viewport_x).apply_pin(flick);
                let viewport_y = (Flickable::FIELD_OFFSETS.viewport_y).apply_pin(flick);
                let old_pos = (viewport_x.get(), viewport_y.get());
                viewport_x.set_animated_value(final_pos.x_length(), anim.clone());
                viewport_y.set_animated_value(final_pos.y_length(), anim);
                if old_pos.0 != final_pos.x_length() || old_pos.1 != final_pos.y_length() {
                    (Flickable::FIELD_OFFSETS.flicked).apply_pin(flick).call(&());
                }
            }
        }
        inner.capture_events = false; // FIXME: should only be set to false once the flick animation is over
        inner.pressed_time = None;
    }
}

fn abs(l: LogicalLength) -> LogicalLength {
    LogicalLength::new(l.get().abs())
}

/// Make sure that the point is within the bounds
fn ensure_in_bound(flick: Pin<&Flickable>, p: LogicalPoint, flick_rc: &ItemRc) -> LogicalPoint {
    let geo = flick_rc.geometry();
    let w = geo.width_length();
    let h = geo.height_length();
    let vw = (Flickable::FIELD_OFFSETS.viewport_width).apply_pin(flick).get();
    let vh = (Flickable::FIELD_OFFSETS.viewport_height).apply_pin(flick).get();

    let min = LogicalPoint::from_lengths(w - vw, h - vh);
    let max = LogicalPoint::default();
    p.max(min).min(max)
}

/// # Safety
/// This must be called using a non-null pointer pointing to a chunk of memory big enough to
/// hold a FlickableDataBox
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_flickable_data_init(data: *mut FlickableDataBox) {
    core::ptr::write(data, FlickableDataBox::default());
}

/// # Safety
/// This must be called using a non-null pointer pointing to an initialized FlickableDataBox
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_flickable_data_free(data: *mut FlickableDataBox) {
    core::ptr::drop_in_place(data);
}
