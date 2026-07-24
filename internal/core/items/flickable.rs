// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore tmax tmin
//! The implementation details behind the Flickable

//! The `Flickable` item

use super::{
    Item, ItemConsts, ItemRc, ItemRendererRef, KeyEventResult, PointerEventButton, RenderingResult,
    VoidArg,
};
use crate::animations::Instant;
use crate::animations::simulations::constant_deceleration::ConstantDecelerationParameters;
use crate::input::InternalKeyEvent;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, MouseEvent, TouchPhase,
};
use crate::item_rendering::CachedRenderingData;
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
    PointLengths, RectLengths,
};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Callback, Coord, Property};
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
mod data_ringbuffer;
use data_ringbuffer::VelocityRingBuffer;

/// Deceleration during the animation. It slows down the initial velocity of the simulation
/// so that the simulation stops at some point if it didn't reach the limit
/// The unit is: LogicalPixel/s^2
const DECELERATION: f32 = 2000.;
/// Fixed-duration animation used for wheel scrolling, where we don't have enough phase
/// information to derive a fling velocity.
/// The unit is: millisecond
const WHEEL_SCROLL_DURATION: Duration = Duration::from_millis(180);
/// The maximum duration between a move and a release event to start an animation
/// If the duration is larger than this value, no animation will be executed because
/// it is not desired
const MAX_DURATION: Duration = Duration::from_millis(100);

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
    pub mouse_drag_pan_enabled: Property<bool>,

    pub flicked: Callback<VoidArg>,

    data: FlickableDataBox,

    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Flickable {
    fn init(self: Pin<&Self>, self_rc: &ItemRc) {
        self.data.in_bound_change_handler.init_delayed(
            self_rc.downgrade(),
            // Binding that returns if the Flickable is out of bounds:
            |self_weak| {
                let Some(flick_rc) = self_weak.upgrade() else {
                    return (false, false);
                };
                let Some(flick) = flick_rc.downcast::<Flickable>() else {
                    return (false, false);
                };
                let flick = flick.as_pin_ref();
                let geo = Self::geometry_without_virtual_keyboard(&flick_rc);

                let zero = LogicalLength::zero();
                let vpx = flick.viewport_x();
                let vpy = flick.viewport_y();
                let x_out_of_bounds =
                    vpx > zero || vpx < (geo.width_length() - flick.viewport_width()).min(zero);
                let y_out_of_bounds =
                    vpy > zero || vpy < (geo.height_length() - flick.viewport_height()).min(zero);

                (x_out_of_bounds, y_out_of_bounds)
            },
            // Change event handler that puts the Flickable in bounds if it's not already
            |self_weak, (x_out_of_bounds, y_out_of_bounds)| {
                let Some(flick_rc) = self_weak.upgrade() else { return };
                let Some(flick) = flick_rc.downcast::<Flickable>() else { return };
                let flick = flick.as_pin_ref();
                let vpx = flick.viewport_x();
                let vpy = flick.viewport_y();
                let p = ensure_in_bound(flick, LogicalPoint::from_lengths(vpx, vpy), &flick_rc);

                let x = (Flickable::FIELD_OFFSETS.viewport_x()).apply_pin(flick);
                if *x_out_of_bounds && !x.has_binding() {
                    x.set(p.x_length());
                }

                let y = (Flickable::FIELD_OFFSETS.viewport_y()).apply_pin(flick);
                if *y_out_of_bounds && !y.has_binding() {
                    y.set(p.y_length());
                }
            },
        );
    }

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        _: &mut super::MouseCursorInner,
    ) -> InputEventFilterResult {
        if let Some(pos) = event.position() {
            let geometry = Self::geometry_without_virtual_keyboard(self_rc);

            if (pos.x < 0 as _
                || pos.y < 0 as _
                || pos.x_length() > geometry.width_length()
                || pos.y_length() > geometry.height_length())
                && self.data.inner.borrow().pressed_mouse_state.is_none()
            {
                return InputEventFilterResult::Intercept;
            }
        }
        if !self.accepts_pan_event(event) {
            return InputEventFilterResult::ForwardAndIgnore;
        }
        self.data.handle_mouse_filter(self, event, window_adapter, self_rc)
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        _: &mut super::MouseCursorInner,
    ) -> InputEventResult {
        if !self.accepts_pan_event(event) {
            return InputEventResult::EventIgnored;
        }
        if let Some(pos) = event.position() {
            let geometry = Self::geometry_without_virtual_keyboard(self_rc);
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

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
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
        Self::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

impl Flickable {
    /// Whether the event may pan this Flickable, given that `interactive` and
    /// `mouse-drag-pan-enabled` can disable it.
    fn accepts_pan_event(self: Pin<&Self>, event: &MouseEvent) -> bool {
        match event {
            MouseEvent::Wheel { .. } => true,
            MouseEvent::Pressed { .. } | MouseEvent::Moved { .. } | MouseEvent::Released { .. } => {
                self.interactive() && (event.is_from_touch() || self.mouse_drag_pan_enabled())
            }
            MouseEvent::Exit
            | MouseEvent::DragMove { .. }
            | MouseEvent::Drop { .. }
            | MouseEvent::PinchGesture { .. }
            | MouseEvent::RotationGesture { .. } => self.interactive(),
        }
    }

    fn choose_min_move(
        current_view_start: Coord, // vx or vy
        view_len: Coord,           // w or h
        content_len: Coord,        // vw or vh
        points: impl Iterator<Item = Coord>,
    ) -> Coord {
        // Feasible translations t such that for all p: vx+t <= p <= vx+t+w
        // -> t in [max_i(p_i - (vx + w)), min_i(p_i - vx)]
        let zero = 0 as Coord;
        let mut lower = Coord::MIN;
        let mut upper = Coord::MAX;

        for p in points {
            lower = lower.max(p - (current_view_start + view_len));
            upper = upper.min(p - current_view_start);
        }

        if lower > upper {
            // No translation can include all points simultaneously; pick nearest bound direction.
            // This happens only with NaNs; guard anyway.
            return zero;
        }

        // Allowed translation interval due to scroll limits
        let max_scroll = (content_len - view_len).max(zero);
        let tmin = -current_view_start; // cannot scroll before 0
        let tmax = max_scroll - current_view_start; // cannot scroll past max

        let i_min = lower.max(tmin);
        let i_max = upper.min(tmax);

        if i_min <= i_max {
            if zero < i_min {
                i_min
            } else if zero > i_max {
                i_max
            } else {
                zero
            }
        // Intervals disjoint: choose closest allowed translation to feasible interval
        // either entirely left or right
        } else if tmax < lower {
            tmax
        } else {
            tmin
        }
    }

    /// Scroll the Flickable so that all of the points are visible at the same time (if possible).
    /// The points have to be in the parent's coordinate space.
    pub(crate) fn reveal_points(self: Pin<&Self>, self_rc: &ItemRc, pts: &[LogicalPoint]) {
        if pts.is_empty() {
            return;
        }

        // visible viewport size from base Item
        let geo = Self::geometry_without_virtual_keyboard(self_rc);

        // content extents and current viewport origin (content coords)
        let vw = Self::FIELD_OFFSETS.viewport_width().apply_pin(self).get().0;
        let vh = Self::FIELD_OFFSETS.viewport_height().apply_pin(self).get().0;
        let vx = -Self::FIELD_OFFSETS.viewport_x().apply_pin(self).get().0;
        let vy = -Self::FIELD_OFFSETS.viewport_y().apply_pin(self).get().0;

        // choose minimal translation along each axis
        let tx = Self::choose_min_move(vx, geo.width(), vw, pts.iter().map(|p| p.x));
        let ty = Self::choose_min_move(vy, geo.height(), vh, pts.iter().map(|p| p.y));

        let new_vx = vx + tx;
        let new_vy = vy + ty;

        Self::FIELD_OFFSETS.viewport_x().apply_pin(self).set(euclid::Length::new(-new_vx));
        Self::FIELD_OFFSETS.viewport_y().apply_pin(self).set(euclid::Length::new(-new_vy));
    }

    fn geometry_without_virtual_keyboard(self_rc: &ItemRc) -> LogicalRect {
        let mut geometry = self_rc.geometry();

        // subtract keyboard rect if needed
        if let Some(keyboard_rect) = self_rc.window_adapter().and_then(|window_adapter| {
            window_adapter.window().virtual_keyboard(crate::InternalToken)
        }) {
            let keyboard_pos = keyboard_rect.0;

            let self_in_window_coordinates = self_rc.map_to_native_window(geometry.origin);
            if (keyboard_pos.y as Coord) < (self_in_window_coordinates.y + geometry.height()) {
                // Keyboard is below the flickable and overlapping
                geometry.size.height = keyboard_pos.y as Coord - self_in_window_coordinates.y;
            }
        }
        geometry
    }
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
/// Duration to filter scroll events from children after receiving a scroll event
/// Note: This needs to be rather long, as that makes it more intuitive when scrolling with the
/// mouse in concrete steps.
/// The user can always override this by moving the mouse
/// The value was tuned by hand, could be adjusted with further user feedback
pub(super) const SCROLL_FILTER_DURATION: Duration = Duration::from_millis(800);
/// Short duration for scroll event filtering, used when the end of the flickable is reached.
pub(super) const SHORT_SCROLL_FILTER_DURATION: Duration =
    Duration::from_millis(SCROLL_FILTER_DURATION.as_millis() as u64 / 2);
/// How far the user has to move the mouse to stop filtering scroll event from children after receiving a scroll event
pub(super) const SCROLL_FILTER_DISTANCE_SQUARED: LogicalLength = LogicalLength::new(4 as _);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CaptureEvents {
    MouseOrTouchScreen,
    MouseWheel,
}

#[derive(Default)]
struct FlickableDataInner {
    /// The time and position in which the press was made
    ///
    /// The position is in the coordinate system of the flickable, not of the viewport.
    pressed_mouse_state: Option<(Instant, LogicalPoint)>,
    /// The last mouse position received, used to calculate the delta when flicking with the mouse.
    ///
    /// This position is in the coordinate system of the flickable, not of the viewport.
    last_mouse_position: LogicalPoint,
    /// Set to true if the flickable is flicking and capturing all mouse event, not forwarding back to the children
    capture_events: Option<CaptureEvents>,
    /// Heuristics for filtering scroll events from children after we have scrolled ourselves.
    /// We want to filter those to prevent the case where the user scrolls with the mouse wheel,
    /// but the mouse now moves over a child item, and that item captures the scroll event.
    /// We use two heuristics: First, a timeout after we received a scroll event, and second, if the mouse moves we
    /// stop filtering scroll event until the next scroll event.
    last_scroll_event: Option<(Instant, LogicalPoint)>,

    /// Ringbuffer to store the last move deltas. From those data the velocity can be
    /// calculated required for the animation after the release event
    velocity_rb: VelocityRingBuffer<5>,

    /// The animation details of the currently running animation for smooth mouse wheel scrolling.
    /// This allows us to add the missing delta of the animation to the next scroll event if the user scrolls again
    /// before the animation is finished.
    running_animation: Option<(Instant, [Option<ConstantDecelerationParameters>; 2])>,
}

impl FlickableDataInner {
    fn should_capture_scroll(&self, timeout: Duration, position: LogicalPoint) -> bool {
        self.last_scroll_event.is_some_and(|(last_time, last_position)| {
            // Note: Squared length for MCU support, which use i32 coords.
            crate::animations::current_tick() - last_time < timeout
                && LogicalLength::new((last_position - position).square_length().abs())
                    < SCROLL_FILTER_DISTANCE_SQUARED
        })
    }

    /// Whether the delta is a scroll in a orthogonal direction than what is allowed by the Flickable
    #[allow(clippy::nonminimal_bool)] // more readable this way
    fn is_allowed_scroll_direction(
        flick: Pin<&Flickable>,
        delta: LogicalVector,
        flick_rc: &ItemRc,
    ) -> bool {
        let geo = Flickable::geometry_without_virtual_keyboard(flick_rc);

        (delta.y != 0 as Coord && flick.viewport_height() > geo.height_length())
            || (delta.x != 0 as Coord && flick.viewport_width() > geo.width_length())
    }

    fn process_wheel_event(
        &mut self,
        flick: Pin<&Flickable>,
        mut delta: LogicalVector,
        position: LogicalPoint,
        phase: TouchPhase,
        flick_rc: &ItemRc,
    ) -> InputEventResult {
        if phase != TouchPhase::Started
            && delta != LogicalVector::default()
            && !Self::is_allowed_scroll_direction(flick, delta, flick_rc)
        {
            // Release the capture immediately, this event is not meant for this Flickable.
            self.capture_events = None;
            self.last_scroll_event = None;
            self.running_animation = None;
            self.velocity_rb = VelocityRingBuffer::default();
            return InputEventResult::EventIgnored;
        }

        let viewport_x = (Flickable::FIELD_OFFSETS.viewport_x()).apply_pin(flick);
        let viewport_y = (Flickable::FIELD_OFFSETS.viewport_y()).apply_pin(flick);
        let current_pos = LogicalPoint::from_lengths(viewport_x.get(), viewport_y.get());

        if self.capture_events.is_none()
            && matches!(phase, TouchPhase::Moved)
            && let Some((start_time, [x_simulation, y_simulation])) = &self.running_animation
        {
            // If the animation is not finished, we add the remaining animations delta.
            let animation_duration = crate::animations::current_tick().duration_since(*start_time);

            if let Some(x_simulation) = x_simulation {
                delta.x += x_simulation.remaining_distance(animation_duration);
            }
            if let Some(y_simulation) = y_simulation {
                delta.y += y_simulation.remaining_distance(animation_duration);
            }
        }

        let new_pos = ensure_in_bound(flick, current_pos + delta, flick_rc);
        delta = new_pos - current_pos;

        if phase != TouchPhase::Ended {
            viewport_x.remove_binding();
            viewport_y.remove_binding();
            self.running_animation = None;
        }

        match phase {
            TouchPhase::Cancelled => {
                viewport_x.set(new_pos.x_length());
                viewport_y.set(new_pos.y_length());
                self.last_scroll_event = Some((crate::animations::current_tick(), position));
            }
            TouchPhase::Started => {
                self.velocity_rb = VelocityRingBuffer::default();
                self.capture_events = Some(CaptureEvents::MouseWheel);
                self.last_scroll_event = Some((crate::animations::current_tick(), position));
            }
            TouchPhase::Moved => {
                if self.capture_events.is_some_and(|capture| capture == CaptureEvents::MouseWheel) {
                    // Touchpad case with different phases
                    self.velocity_rb.push(crate::animations::current_tick(), new_pos - current_pos);
                    viewport_x.set(new_pos.x_length());
                    viewport_y.set(new_pos.y_length());
                } else {
                    // Mousewheel case with no phase
                    // Add a short animation that covers the delta for smooth scrolling
                    //
                    // Note that this animation must support the viewport_x/_y and width/height
                    // changing, as e.g. the ListView might resize the viewport if it gets a new size
                    // estimate.
                    //
                    // At the time of writing, in practice this means we must use a physics animation.
                    let [limit_x, limit_y] = Self::flick_limits(flick_rc, delta);

                    let x_simulation = (delta.x != Coord::default()).then(|| {
                        let simulation = ConstantDecelerationParameters::new_with_distance(
                            delta.x as f32,
                            WHEEL_SCROLL_DURATION.as_secs_f32(),
                        );
                        viewport_x.set_physic_animation_value(limit_x, simulation.clone());
                        simulation
                    });

                    let y_simulation = (delta.y != Coord::default()).then(|| {
                        let simulation = ConstantDecelerationParameters::new_with_distance(
                            delta.y as f32,
                            WHEEL_SCROLL_DURATION.as_secs_f32(),
                        );
                        viewport_y.set_physic_animation_value(limit_y, simulation.clone());
                        simulation
                    });

                    if delta.x != 0 as Coord || delta.y != 0 as Coord {
                        (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
                    }

                    self.running_animation =
                        Some((crate::animations::current_tick(), [x_simulation, y_simulation]));
                }
                self.last_scroll_event = Some((crate::animations::current_tick(), position));
            }
            TouchPhase::Ended => {
                if self.capture_events.is_some_and(|capture| capture == CaptureEvents::MouseWheel) {
                    self.animate(flick, flick_rc);
                }
                self.capture_events = None;
                return if self.should_capture_scroll(SHORT_SCROLL_FILTER_DURATION, position) {
                    InputEventResult::EventAccepted
                } else {
                    InputEventResult::EventIgnored
                };
            }
        }

        let flicked = current_pos.x_length() != new_pos.x_length()
            || current_pos.y_length() != new_pos.y_length();
        if flicked {
            (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
            InputEventResult::EventAccepted
        } else if self.should_capture_scroll(SHORT_SCROLL_FILTER_DURATION, position) {
            // After reaching the end, keep accepting the input event for a while longer, then time
            // out (by not updating the last_scroll_event)
            InputEventResult::EventAccepted
        } else {
            self.last_scroll_event = None;
            InputEventResult::EventIgnored
        }
    }

    fn flick_limits(
        flick_rc: &ItemRc,
        flick_velocity: LogicalVector,
    ) -> [Pin<Box<Property<f32>>>; 2] {
        let flick_weak = flick_rc.downgrade();
        let calculate_limits = move || {
            flick_weak
                .upgrade()
                .and_then(|flick_rc| {
                    flick_rc.downcast::<Flickable>().map(move |flick| (flick_rc, flick))
                })
                .map(|(flick_rc, flick)| {
                    let flick = flick.as_pin_ref();
                    ensure_in_bound(
                        flick,
                        LogicalPoint::from_lengths(
                            -flick.viewport_width(),
                            -flick.viewport_height(),
                        ),
                        &flick_rc,
                    )
                })
        };

        let limit_x = if flick_velocity.x < 0 as Coord {
            let property = Box::pin(Property::new(0.0));
            property.set_binding({
                let calculate_limits = calculate_limits.clone();
                move || calculate_limits().map(|limit| limit.x_length().get() as f32).unwrap_or(0.0)
            });
            property
        } else {
            Box::pin(Property::new(0.0))
        };

        let limit_y = if flick_velocity.y < 0 as Coord {
            let property = Box::pin(Property::new(0.0));
            property.set_binding(move || {
                calculate_limits().map(|limit| limit.y_length().get() as f32).unwrap_or(0.0)
            });
            property
        } else {
            Box::pin(Property::new(0.0))
        };

        [limit_x, limit_y]
    }

    fn animate(&self, flick: Pin<&Flickable>, flick_rc: &ItemRc) {
        if let Some(last_time) = self.velocity_rb.last_time() {
            let mean_velocity = self.velocity_rb.mean_velocity();
            if self.capture_events.is_some()
                && mean_velocity.square_length() > 0 as Coord
                && crate::animations::current_tick().duration_since(last_time) < MAX_DURATION
            {
                let viewport_x = (Flickable::FIELD_OFFSETS.viewport_x()).apply_pin(flick);
                let viewport_y = (Flickable::FIELD_OFFSETS.viewport_y()).apply_pin(flick);

                let [limit_x, limit_y] = Self::flick_limits(flick_rc, mean_velocity);

                {
                    let simulation =
                        ConstantDecelerationParameters::new(mean_velocity.x as f32, DECELERATION);
                    viewport_x.set_physic_animation_value(limit_x, simulation);
                }

                {
                    let animation_y =
                        ConstantDecelerationParameters::new(mean_velocity.y as f32, DECELERATION);
                    viewport_y.set_physic_animation_value(limit_y, animation_y);
                }

                if mean_velocity.x != 0 as Coord || mean_velocity.y != 0 as Coord {
                    (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
                }
            }
        }
    }
}

#[derive(Default)]
pub struct FlickableData {
    inner: RefCell<FlickableDataInner>,
    /// Tracker that tracks the property to make sure that the flickable is in bounds
    in_bound_change_handler: crate::properties::ChangeTracker,
}

impl FlickableData {
    fn scroll_delta(
        window_adapter: &Rc<dyn WindowAdapter>,
        delta_x: Coord,
        delta_y: Coord,
    ) -> LogicalVector {
        if window_adapter.window().0.context().0.modifiers.get().shift()
            && !cfg!(target_os = "macos")
        {
            // Shift invert coordinate for the purpose of scrolling.
            // But not on macOs because there the OS already take care of the change
            LogicalVector::new(delta_y, delta_x)
        } else {
            LogicalVector::new(delta_x, delta_y)
        }
    }

    fn handle_mouse_filter(
        &self,
        flick: Pin<&Flickable>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        flick_rc: &ItemRc,
    ) -> InputEventFilterResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::Pressed { position, button: PointerEventButton::Left, .. } => {
                inner.velocity_rb = VelocityRingBuffer::default();
                inner.pressed_mouse_state = Some((crate::animations::current_tick(), *position));
                inner.last_mouse_position = *position;
                let viewport_x = (Flickable::FIELD_OFFSETS.viewport_x()).apply_pin(flick);
                viewport_x.remove_binding(); // Stop animation by removing the binding
                let viewport_y = (Flickable::FIELD_OFFSETS.viewport_y()).apply_pin(flick);
                viewport_y.remove_binding(); // Stop animation by removing the binding

                if inner.capture_events.is_some() {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::DelayForwarding(FORWARD_DELAY.as_millis() as _)
                }
            }
            MouseEvent::Exit | MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                inner.pressed_mouse_state = None;
                if inner.capture_events.is_some() {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::Moved { position, .. } => {
                let do_intercept = inner.capture_events.is_some()
                    || inner.pressed_mouse_state.is_some_and(
                        |(pressed_time, pressed_mouse_position)| {
                            let mouse_delta = *position - pressed_mouse_position;

                            crate::animations::current_tick() - pressed_time <= DURATION_THRESHOLD
                                && self.should_capture_mouse_direction(mouse_delta, flick, flick_rc)
                        },
                    );
                if do_intercept {
                    InputEventFilterResult::Intercept
                } else if inner.pressed_mouse_state.is_some() {
                    InputEventFilterResult::ForwardAndInterceptGrab
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::Wheel { position, delta_x, delta_y, phase } => {
                match phase {
                    TouchPhase::Cancelled => {
                        // Qt sends the Cancelled Phase
                        // If we recently handled a wheel event, intercept it to prevent children from grabbing
                        // the scroll event
                        let delta = Self::scroll_delta(window_adapter, *delta_x, *delta_y);
                        if FlickableDataInner::is_allowed_scroll_direction(flick, delta, flick_rc)
                            && inner.should_capture_scroll(SCROLL_FILTER_DURATION, *position)
                        {
                            InputEventFilterResult::Intercept
                        } else {
                            inner.last_scroll_event = None;
                            InputEventFilterResult::ForwardEvent
                        }
                    }
                    TouchPhase::Started => InputEventFilterResult::Intercept,
                    TouchPhase::Moved => {
                        if inner.capture_events.is_some() {
                            InputEventFilterResult::Intercept
                        } else {
                            // If we recently handled a wheel event, intercept it to prevent children from grabbing
                            // the scroll event
                            let delta = Self::scroll_delta(window_adapter, *delta_x, *delta_y);
                            if FlickableDataInner::is_allowed_scroll_direction(
                                flick, delta, flick_rc,
                            ) && inner.should_capture_scroll(SCROLL_FILTER_DURATION, *position)
                            {
                                InputEventFilterResult::Intercept
                            } else {
                                inner.last_scroll_event = None;
                                InputEventFilterResult::ForwardEvent
                            }
                        }
                    }
                    TouchPhase::Ended => {
                        if inner.capture_events.is_some() {
                            InputEventFilterResult::Intercept
                        } else {
                            InputEventFilterResult::ForwardEvent
                        }
                    }
                }
            }
            // Not the left button
            MouseEvent::Pressed { .. } | MouseEvent::Released { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
            MouseEvent::PinchGesture { .. } | MouseEvent::RotationGesture { .. } => {
                InputEventFilterResult::ForwardEvent
            }
            MouseEvent::DragMove { .. } | MouseEvent::Drop { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
        }
    }

    fn should_capture_mouse_direction(
        &self,
        mouse_delta: LogicalVector,
        flick: Pin<&Flickable>,
        flick_rc: &ItemRc,
    ) -> bool {
        let flickable_geometry = Flickable::geometry_without_virtual_keyboard(flick_rc);
        let flickable_width = flickable_geometry.width_length();
        let flickable_height = flickable_geometry.height_length();
        let viewport_width = flick.viewport_width();
        let viewport_height = flick.viewport_height();
        let zero = LogicalLength::zero();

        // We should capture the mouse movement, if the flickable can move in this
        // axis, and the mouse has moved more than the threshold in this axis.
        ((viewport_width > flickable_width || flick.viewport_x() != zero)
            && abs(mouse_delta.x_length()) > DISTANCE_THRESHOLD)
            || ((viewport_height > flickable_height || flick.viewport_y() != zero)
                && abs(mouse_delta.y_length()) > DISTANCE_THRESHOLD)
    }

    fn handle_mouse(
        &self,
        flick: Pin<&Flickable>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        flick_rc: &ItemRc,
    ) -> InputEventResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::Pressed { .. } => {
                inner.capture_events = Some(CaptureEvents::MouseOrTouchScreen);
                InputEventResult::GrabMouse
            }
            MouseEvent::Exit | MouseEvent::Released { .. } => {
                if inner.capture_events.is_some_and(|f| f == CaptureEvents::MouseOrTouchScreen) {
                    let was_capturing = true;
                    inner.animate(flick, flick_rc);
                    inner.capture_events = None;
                    inner.pressed_mouse_state = None;
                    if was_capturing {
                        InputEventResult::EventAccepted
                    } else {
                        InputEventResult::EventIgnored
                    }
                } else if inner.capture_events.is_none() {
                    inner.pressed_mouse_state = None;
                    InputEventResult::EventIgnored
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Moved { position, .. } => {
                // Important constraint: The viewport_y might not be stable, and might jump around
                // wildly!
                // This is especially the case if a ListView is involved, which will continuously
                // update its own viewport_y to keep the current item visible, which can cause the
                // viewport_y to jump.
                //
                // So to correctly calculate the mouse delta, we need to use the position of
                // the mouse in the flickables coordinate system and never the viewport coordinate
                // system.
                if let Some((_pressed_time, _pressed_mouse_position)) = inner.pressed_mouse_state {
                    let mouse_delta = *position - inner.last_mouse_position;
                    inner.velocity_rb.push(crate::animations::current_tick(), mouse_delta);

                    let is_capturing = inner
                        .capture_events
                        .is_some_and(|f| f == CaptureEvents::MouseOrTouchScreen);
                    if is_capturing
                        || self.should_capture_mouse_direction(mouse_delta, flick, flick_rc)
                    {
                        // The drag event is meant to move the viewport, set it to the new position
                        // and start capturing mouse events.
                        let viewport_x = (Flickable::FIELD_OFFSETS.viewport_x()).apply_pin(flick);
                        let viewport_y = (Flickable::FIELD_OFFSETS.viewport_y()).apply_pin(flick);
                        let current_viewport_position =
                            LogicalPoint::from_lengths(viewport_x.get(), viewport_y.get());

                        // We calculate the new viewport position by adding the mouse delta in the flickable
                        // coordinate system to the current viewport position.
                        // Do not rely on the existing viewport position to be stable, as e.g. the
                        // ListView will continuously update it.
                        // So we cannot calculate the delta in viewport coordinates.
                        let new_viewport_position = current_viewport_position + mouse_delta;
                        let new_viewport_position =
                            ensure_in_bound(flick, new_viewport_position, flick_rc);

                        viewport_x.set(new_viewport_position.x_length());
                        viewport_y.set(new_viewport_position.y_length());
                        if current_viewport_position != new_viewport_position {
                            (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
                        }

                        // Only update the mouse position if we are actually applying the delta.
                        // When the drag starts, there is a short dead zone that is determined by the
                        // DISTANCE_THRESHOLD. We want to apply that threshold to the
                        // delta once we've overcome it, so we need to update the position that we
                        // calculate the delta from only after we've cleared the dead zone and are
                        // actually moving.
                        //
                        // Note: As an alternative to updating the last_mouse_position to the new mouse position,
                        // we could also update it by the amount that the viewport actually moved.
                        // This would cause the mouse to stick to a given position in the viewport
                        // instead of starting to drift if the drag goes into the viewport limits.
                        // Then this code would need to be:
                        //
                        //  inner.last_mouse_position += new_viewport_position - current_viewport_position;
                        //
                        // But at least for a touchscreen, the current behavior is more intuitive.
                        inner.last_mouse_position = *position;

                        inner.capture_events = Some(CaptureEvents::MouseOrTouchScreen);

                        InputEventResult::GrabMouse
                    } else if abs(mouse_delta.x_length()) > DISTANCE_THRESHOLD
                        || abs(mouse_delta.y_length()) > DISTANCE_THRESHOLD
                    {
                        // drag in a unsupported direction gives up the grab
                        InputEventResult::EventIgnored
                    } else {
                        // the mouse was moved, but not enough to start the drag, we still want to accept further events
                        // so that we may pass the threshold at some point
                        InputEventResult::EventAccepted
                    }
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Wheel { delta_x, delta_y, position, phase } => {
                let delta = Self::scroll_delta(window_adapter, *delta_x, *delta_y);
                inner.process_wheel_event(flick, delta, *position, *phase, flick_rc)
            }
            MouseEvent::PinchGesture { .. } | MouseEvent::RotationGesture { .. } => {
                InputEventResult::EventIgnored
            }
            MouseEvent::DragMove { .. } | MouseEvent::Drop { .. } => InputEventResult::EventIgnored,
        }
    }
}

fn abs(l: LogicalLength) -> LogicalLength {
    LogicalLength::new(l.get().abs())
}

/// Make sure that the point is within the bounds
fn ensure_in_bound(flick: Pin<&Flickable>, p: LogicalPoint, flick_rc: &ItemRc) -> LogicalPoint {
    let geo = Flickable::geometry_without_virtual_keyboard(flick_rc);
    let w = geo.width_length();
    let h = geo.height_length();
    let vw = (Flickable::FIELD_OFFSETS.viewport_width()).apply_pin(flick).get();
    let vh = (Flickable::FIELD_OFFSETS.viewport_height()).apply_pin(flick).get();

    let min = LogicalPoint::from_lengths(w - vw, h - vh);
    let max = LogicalPoint::default();
    p.max(min).min(max)
}

/// # Safety
/// This must be called using a non-null pointer pointing to a chunk of memory big enough to
/// hold a FlickableDataBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_flickable_data_init(data: *mut FlickableDataBox) {
    unsafe { core::ptr::write(data, FlickableDataBox::default()) };
}

/// # Safety
/// This must be called using a non-null pointer pointing to an initialized FlickableDataBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_flickable_data_free(data: *mut FlickableDataBox) {
    unsafe {
        core::ptr::drop_in_place(data);
    }
}
