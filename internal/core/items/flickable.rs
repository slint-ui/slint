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
use crate::animations::simulations::PositionSimulation;
use crate::animations::simulations::scroll_spring::SpringSimulation;
use crate::input::InputEventFilterResult::ForwardEvent;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, MouseEvent, TouchPhase,
};
use crate::input::{InternalKeyEvent, TouchHistory};
use crate::item_rendering::CachedRenderingData;
use crate::item_tree::ItemWeak;
use crate::items::AutoBool;
#[cfg(not(any(
    target_os = "ios",
    target_os = "linux",
    target_os = "none",
    target_os = "macos"
)))]
use crate::items::flickable::velocity_tracker::GeneralVelocityTracker;
#[cfg(any(target_os = "ios", target_os = "linux", target_os = "none"))]
use crate::items::flickable::velocity_tracker::IOsVelocityTracker;
#[cfg(target_os = "macos")]
use crate::items::flickable::velocity_tracker::MacOsVelocityTracker;
use crate::items::flickable::velocity_tracker::{Velocity, VelocityTracker as _};
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
use alloc::rc::{Rc, Weak};
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
mod animation;
mod velocity_tracker;
use animation::{FlickAnimation, FlickAnimationParameter};

/// Fixed-duration animation used for wheel scrolling, where we don't have enough phase
/// information to derive a fling velocity.
/// The unit is: millisecond
const WHEEL_SCROLL_DURATION: Duration = Duration::from_millis(180);
#[cfg(not(any(target_os = "ios", target_os = "linux", target_os = "none", target_os = "macos")))]
const VELOCITY_TRACKER_SAMPLES: usize = 20;
const MOMENTUM_RETAIN_TIMEOUT: Duration = Duration::from_millis(20);

// We use for linux and no os the ios velocity tracker because embedded is
// computational power constraint and embedded linux can be as well and the
// velocity estimation is much easier than for the general velocity tracker
#[cfg(any(target_os = "ios", target_os = "linux", target_os = "none"))]
type VelocityTracker = IOsVelocityTracker;
#[cfg(target_os = "macos")]
type VelocityTracker = MacOsVelocityTracker;
#[cfg(not(any(target_os = "ios", target_os = "linux", target_os = "none", target_os = "macos")))]
type VelocityTracker = GeneralVelocityTracker<VELOCITY_TRACKER_SAMPLES>;

enum Dimension {
    X,
    Y,
}

/// The implementation of the `Flickable` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct Flickable {
    pub content_x: Property<LogicalLength>,
    pub content_y: Property<LogicalLength>,
    pub content_width: Property<LogicalLength>,
    pub content_height: Property<LogicalLength>,

    pub interactive: Property<bool>,
    pub mouse_drag_pan_enabled: Property<bool>,

    pub bounce: Property<AutoBool>,
    pub carry_momentum: Property<AutoBool>,

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
                    return (false, false, Default::default());
                };
                let Some(flick) = flick_rc.downcast::<Flickable>() else {
                    return (false, false, Default::default());
                };
                let flick = flick.as_pin_ref();
                let geo = Self::geometry_without_virtual_keyboard(&flick_rc);

                let zero = LogicalLength::zero();
                let vpx = flick.content_x();
                let vpy = flick.content_y();
                let x_out_of_bounds =
                    vpx > zero || vpx < (geo.width_length() - flick.content_width()).min(zero);
                let y_out_of_bounds =
                    vpy > zero || vpy < (geo.height_length() - flick.content_height()).min(zero);

                (x_out_of_bounds, y_out_of_bounds, geo)
            },
            // Change event handler that puts the Flickable in bounds if it's not already
            |self_weak, (x_out_of_bounds, y_out_of_bounds, geo)| {
                let Some(flick_rc) = self_weak.upgrade() else { return };
                let Some(flick) = flick_rc.downcast::<Flickable>() else { return };
                if !x_out_of_bounds && !*y_out_of_bounds {
                    return;
                }
                let flick = flick.as_pin_ref();
                let use_bounce_x =
                    FlickAnimation::use_bounce(effective_bounce(flick, geo, Dimension::X));
                let use_bounce_y =
                    FlickAnimation::use_bounce(effective_bounce(flick, geo, Dimension::Y));
                let vpx = flick.content_x();
                let vpy = flick.content_y();
                let p = ensure_in_bound(
                    flick,
                    LogicalPoint::from_lengths(vpx, vpy),
                    geo,
                    use_bounce_x,
                    use_bounce_y,
                );

                let x = (Flickable::FIELD_OFFSETS.content_x()).apply_pin(flick);
                if *x_out_of_bounds && !x.has_binding() && !use_bounce_x {
                    x.set(p.x_length());
                }

                let y = (Flickable::FIELD_OFFSETS.content_y()).apply_pin(flick);
                if *y_out_of_bounds && !y.has_binding() && !use_bounce_y {
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
        current_view_start: Coord, // cx or cy
        view_len: Coord,           // w or h
        content_len: Coord,        // cw or ch
        points: impl Iterator<Item = Coord>,
    ) -> Coord {
        // Feasible translations t such that for all p: cx+t <= p <= cx+t+w
        // -> t in [max_i(p_i - (cx + w)), min_i(p_i - cx)]
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

        // content extents and current content origin
        let cw = Self::FIELD_OFFSETS.content_width().apply_pin(self).get().0;
        let ch = Self::FIELD_OFFSETS.content_height().apply_pin(self).get().0;
        let cx = -Self::FIELD_OFFSETS.content_x().apply_pin(self).get().0;
        let cy = -Self::FIELD_OFFSETS.content_y().apply_pin(self).get().0;

        // choose minimal translation along each axis
        let tx = Self::choose_min_move(cx, geo.width(), cw, pts.iter().map(|p| p.x));
        let ty = Self::choose_min_move(cy, geo.height(), ch, pts.iter().map(|p| p.y));

        let new_cx = cx + tx;
        let new_cy = cy + ty;

        Self::FIELD_OFFSETS.content_x().apply_pin(self).set(euclid::Length::new(-new_cx));
        Self::FIELD_OFFSETS.content_y().apply_pin(self).set(euclid::Length::new(-new_cy));
    }

    pub(crate) fn geometry_without_virtual_keyboard(self_rc: &ItemRc) -> LogicalRect {
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
    MouseStart,
    MouseMove,
    WheelMove,
    /// We captured a mouse wheel event but we did not yet decide if we are taking it or
    /// if a child is taking it
    WheelStart,
}

struct RunningSimulation {
    start_time: Instant,
    #[expect(unused, reason = "Will be used in a future pr for the listview")]
    weak: ItemWeak,
    x_simulation: Option<Rc<RefCell<dyn PositionSimulation>>>,
    y_simulation: Option<Rc<RefCell<dyn PositionSimulation>>>,
}

#[derive(Default)]
struct FlickableDataInner {
    /// The time and position in which the press was made
    ///
    /// The position is in the coordinate system of the flickable, not of the content element.
    pressed_mouse_state: Option<(Instant, LogicalPoint)>,
    /// The last mouse position received, used to calculate the delta when flicking with the mouse.
    ///
    /// This position is in the coordinate system of the flickable, not of the content element.
    last_mouse_position: Option<LogicalPoint>,
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
    velocity_rb: VelocityTracker,

    /// The animation details of the currently running animation for smooth mouse wheel scrolling.
    /// This allows us to add the missing delta of the animation to the next scroll event if the user scrolls again
    /// before the animation is finished.
    running_animation: Option<RunningSimulation>,

    retained_velocity: Velocity,
}

impl FlickableDataInner {
    /// Lose momentum if certain conditions are not fulfilled
    fn maybe_lose_momentum(&mut self, tick: &Instant) {
        if self
            .last_scroll_event
            .is_none_or(|(time, _)| tick.0.saturating_sub(time.0) > MOMENTUM_RETAIN_TIMEOUT)
        {
            self.retained_velocity = Default::default();
        }
    }

    fn subtract_distance_threshold(delta: Coord) -> Coord {
        if delta >= 0 as Coord {
            (delta - DISTANCE_THRESHOLD.0).max(0 as Coord)
        } else {
            (delta + DISTANCE_THRESHOLD.0).min(0 as Coord)
        }
    }

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

        let allowed_y = FlickAnimation::use_bounce(effective_bounce(flick, &geo, Dimension::Y))
            || (delta.y != 0 as Coord && flick.content_height() > geo.height_length());
        let allowed_x = FlickAnimation::use_bounce(effective_bounce(flick, &geo, Dimension::X))
            || (delta.x != 0 as Coord && flick.content_width() > geo.width_length());

        allowed_x || allowed_y
    }

    /// Calculate the position offset of this scroll move. If we would go beyond the limits and bouncing is enabled
    /// a friction will be applied so the user cannot go far beyond the limits
    fn calculate_move_offset(
        &self,
        current_pos: LogicalPoint,
        delta: LogicalVector,
        flick: Pin<&Flickable>,
        flick_rc: &ItemRc,
    ) -> LogicalVector {
        let geo = Flickable::geometry_without_virtual_keyboard(flick_rc);
        let use_bounce_x = FlickAnimation::use_bounce(effective_bounce(flick, &geo, Dimension::X));
        let use_bounce_y = FlickAnimation::use_bounce(effective_bounce(flick, &geo, Dimension::Y));
        let new_pos = ensure_in_bound(flick, current_pos + delta, &geo, use_bounce_x, use_bounce_y);
        FlickAnimation::apply_friction(current_pos, new_pos - current_pos, flick, flick_rc)
    }

    fn track_press(&mut self, event_time: Instant) {
        self.last_mouse_position = None;
        self.velocity_rb = VelocityTracker::default();
        self.velocity_rb.push(event_time, LogicalVector::default());
    }

    fn track_move(&mut self, event_time: Instant, position: LogicalPoint, history: &TouchHistory) {
        let Some(mut previous_position) =
            self.last_mouse_position.or(self.pressed_mouse_state.map(|(_, position)| position))
        else {
            return;
        };
        let mut last_time = self.velocity_rb.last_time().unwrap_or_default();
        for (sample_position, sample_time) in
            history.history.iter().copied().chain(core::iter::once((position, event_time)))
        {
            last_time = last_time.max(sample_time);
            self.track_delta(last_time, sample_position - previous_position);
            previous_position = sample_position;
        }
        self.last_mouse_position = Some(position);
    }

    fn track_delta(&mut self, event_time: Instant, delta: LogicalVector) {
        self.maybe_lose_momentum(&event_time);
        self.velocity_rb.push(event_time, delta);
    }

    /// Execute a scroll move
    fn scroll_move(
        &mut self,
        flick: Pin<&Flickable>,
        flick_rc: &ItemRc,
        position: LogicalPoint,
        delta: LogicalVector,
        content_x: &Pin<&Property<LogicalLength>>,
        content_y: &Pin<&Property<LogicalLength>>,
    ) -> bool {
        let current_tick = crate::animations::current_tick();
        let current_pos = LogicalPoint::from_lengths(content_x.get(), content_y.get());

        // We calculate the new content position by adding the mouse delta in the flickable
        // coordinate system to the current content position.
        // Do not rely on the existing content position to be stable, as e.g. the
        // ListView will continuously update it.
        // So we cannot calculate the delta in content coordinates.
        let new_pos = current_pos + self.calculate_move_offset(current_pos, delta, flick, flick_rc);
        content_x.set(new_pos.x_length());
        content_y.set(new_pos.y_length());

        self.last_scroll_event = Some((current_tick, position));

        // Indicate if flicked
        current_pos.x_length() != new_pos.x_length() || current_pos.y_length() != new_pos.y_length()
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
            self.velocity_rb = Default::default();
            return InputEventResult::EventIgnored;
        }

        let content_x = (Flickable::FIELD_OFFSETS.content_x()).apply_pin(flick);
        let content_y = (Flickable::FIELD_OFFSETS.content_y()).apply_pin(flick);

        if self.capture_events.is_none()
            && matches!(phase, TouchPhase::Moved)
            && let Some(RunningSimulation { start_time, x_simulation, y_simulation, .. }) =
                &self.running_animation
        {
            // If the animation is not finished, we add the remaining animations delta.
            let animation_duration = crate::animations::current_tick().duration_since(*start_time);

            if let Some(x_simulation) = x_simulation {
                delta.x += x_simulation.borrow().remaining_distance(animation_duration) as Coord;
            }
            if let Some(y_simulation) = y_simulation {
                delta.y += y_simulation.borrow().remaining_distance(animation_duration) as Coord;
            }
        }

        if phase != TouchPhase::Ended {
            if phase == TouchPhase::Started {
                self.capture_momentum();
            }

            content_x.remove_binding();
            content_y.remove_binding();
            self.running_animation = None;
        }

        let mut flicked = false;
        match phase {
            TouchPhase::Cancelled => {
                self.track_delta(crate::animations::current_tick(), delta);
                flicked =
                    self.scroll_move(flick, flick_rc, position, delta, &content_x, &content_y);
            }
            TouchPhase::Started => {
                self.velocity_rb = VelocityTracker::default();
                self.capture_events = Some(CaptureEvents::WheelStart);
                self.last_scroll_event = Some((crate::animations::current_tick(), position));
            }
            TouchPhase::Moved => {
                if let Some(capture) = self.capture_events
                    && matches!(capture, CaptureEvents::WheelStart | CaptureEvents::WheelMove)
                {
                    if matches!(capture, CaptureEvents::WheelStart) {
                        // Otherwise we'd jump instead of starting the drag smoothly.
                        delta.x = Self::subtract_distance_threshold(delta.x);
                        delta.y = Self::subtract_distance_threshold(delta.y);
                    }

                    self.track_delta(crate::animations::current_tick(), delta);
                    // Touchpad case with different phases
                    flicked =
                        self.scroll_move(flick, flick_rc, position, delta, &content_x, &content_y);
                    self.capture_events = Some(CaptureEvents::WheelMove);
                } else {
                    // Mousewheel case with no phase
                    // Add a short animation that covers the delta for smooth scrolling
                    //
                    // Note that this animation must support the content_x/_y and width/height
                    // changing, as e.g. the ListView might resize the content if it gets a new size
                    // estimate.
                    //
                    // At the time of writing, in practice this means we must use a physics animation.
                    let limit_x = Self::flick_limits(flick_rc, delta.x as f32, Dimension::X);
                    let limit_y = Self::flick_limits(flick_rc, delta.y as f32, Dimension::Y);
                    let geo = Flickable::geometry_without_virtual_keyboard(flick_rc);

                    let x_simulation: Option<Rc<RefCell<dyn PositionSimulation>>> = (delta.x
                        != Coord::default())
                    .then(|| {
                        let simulation = Rc::new_cyclic(|weak: &Weak<RefCell<FlickAnimation>>| {
                            let curr_val = content_x.get().0 as f32;
                            content_x.set_physic_animation_value(weak.clone());
                            RefCell::new(FlickAnimation::create_animation(
                                FlickAnimationParameter::Distance {
                                    delta: delta.x as f32,
                                    duration: WHEEL_SCROLL_DURATION,
                                },
                                effective_bounce(flick, &geo, Dimension::X),
                                curr_val,
                                limit_x,
                            ))
                        });
                        simulation as Rc<RefCell<dyn PositionSimulation>>
                    });

                    let y_simulation: Option<Rc<RefCell<dyn PositionSimulation>>> = (delta.y
                        != Coord::default())
                    .then(|| {
                        let simulation = Rc::new_cyclic(|weak: &Weak<RefCell<FlickAnimation>>| {
                            let curr_val = content_y.get().0 as f32;
                            content_y.set_physic_animation_value(weak.clone());
                            RefCell::new(FlickAnimation::create_animation(
                                FlickAnimationParameter::Distance {
                                    delta: delta.y as f32,
                                    duration: WHEEL_SCROLL_DURATION,
                                },
                                effective_bounce(flick, &geo, Dimension::Y),
                                curr_val,
                                limit_y,
                            ))
                        });
                        simulation as Rc<RefCell<dyn PositionSimulation>>
                    });

                    if delta.x != 0 as Coord || delta.y != 0 as Coord {
                        (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
                    }

                    self.running_animation = Some(RunningSimulation {
                        start_time: crate::animations::current_tick(),
                        x_simulation,
                        y_simulation,
                        weak: flick_rc.downgrade(),
                    });
                    self.last_scroll_event = Some((crate::animations::current_tick(), position));
                }
            }
            TouchPhase::Ended => {
                if self.capture_events.is_some_and(|capture| capture == CaptureEvents::WheelMove) {
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

        if flicked {
            (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
            InputEventResult::EventAccepted
        } else if self.should_capture_scroll(SHORT_SCROLL_FILTER_DURATION, position) {
            // After reaching the end, keep accepting the input event for a while longer, then time
            // out (by not updating the last_scroll_event)
            if phase == TouchPhase::Started {
                InputEventResult::EventRecognized
            } else {
                InputEventResult::EventAccepted
            }
        } else {
            self.last_scroll_event = None;
            InputEventResult::EventIgnored
        }
    }

    fn capture_momentum(&mut self) {
        self.retained_velocity = self
            .running_animation
            .as_ref()
            .map(|sim| {
                let dt = crate::animations::current_tick().duration_since(sim.start_time);
                Velocity::new(
                    sim.x_simulation
                        .as_ref()
                        .map(|sim| sim.borrow().remaining_velocity(dt))
                        .unwrap_or_default(),
                    sim.y_simulation
                        .as_ref()
                        .map(|sim| sim.borrow().remaining_velocity(dt))
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();
    }

    fn flick_limits(
        flick_rc: &ItemRc,
        flick_velocity: f32,
        dimension: Dimension,
    ) -> Pin<Box<Property<f32>>> {
        let flick_weak = flick_rc.downgrade();
        let calculate_limits = move || {
            flick_weak
                .upgrade()
                .and_then(|flick_rc| {
                    flick_rc.downcast::<Flickable>().map(move |flick| (flick_rc, flick))
                })
                .map(|(flick_rc, flick)| {
                    let flick = flick.as_pin_ref();
                    let geo = Flickable::geometry_without_virtual_keyboard(&flick_rc);
                    ensure_in_bound(
                        flick,
                        LogicalPoint::from_lengths(-flick.content_width(), -flick.content_height()),
                        &geo,
                        false,
                        false,
                    )
                })
        };

        if flick_velocity < 0. {
            let property = Box::pin(Property::new(0.0));
            property.set_binding({
                let calculate_limits = calculate_limits.clone();
                move || {
                    calculate_limits()
                        .map(|limit| match dimension {
                            Dimension::X => limit.x_length().get() as f32,
                            Dimension::Y => limit.y_length().get() as f32,
                        })
                        .unwrap_or(0.0)
                }
            });
            property
        } else {
            Box::pin(Property::new(0.0))
        }
    }

    fn animate(&mut self, flick: Pin<&Flickable>, flick_rc: &ItemRc) {
        if self.capture_events.is_some() {
            let (inside_bounds_x, inside_bounds_y) = inside_bounds(
                flick,
                LogicalPoint::new(flick.content_x().get(), flick.content_y().get()),
                flick_rc,
            );
            let velocity_estimation = self.velocity_rb.estimate_velocity();
            let geo = Flickable::geometry_without_virtual_keyboard(flick_rc);

            let x_simulation = if inside_bounds_x {
                match velocity_estimation.as_ref() {
                    Some(velocity_estimation)
                        if velocity_estimation.velocity.x.abs()
                            >= FlickAnimation::minimum_flick_velocity_animation() =>
                    {
                        let content_x = (Flickable::FIELD_OFFSETS.content_x()).apply_pin(flick);
                        let carried_velocity_x = FlickAnimation::carried_momentum(
                            velocity_estimation.velocity.x,
                            self.retained_velocity.x,
                            flick.carry_momentum(),
                        );
                        let limit_x = Self::flick_limits(
                            flick_rc,
                            velocity_estimation.velocity.x,
                            Dimension::X,
                        );
                        let x_simulation =
                            Rc::new_cyclic(|weak: &Weak<RefCell<FlickAnimation>>| {
                                let curr_val = content_x.get().0 as f32;
                                content_x.set_physic_animation_value(weak.clone());
                                RefCell::new(FlickAnimation::create_animation(
                                    FlickAnimationParameter::Velocity {
                                        velocity: velocity_estimation.velocity.x
                                            + carried_velocity_x,
                                    },
                                    effective_bounce(flick, &geo, Dimension::X),
                                    curr_val,
                                    limit_x,
                                ))
                            });
                        Some(x_simulation as Rc<RefCell<dyn PositionSimulation>>)
                    }
                    _ => None,
                }
            } else {
                let content_x = (Flickable::FIELD_OFFSETS.content_x()).apply_pin(flick);
                let curr_val = content_x.get().0 as f32;
                // Spring back to whichever edge we're already past, not
                // wherever the release velocity happens to point.
                let limit_x = Self::flick_limits(flick_rc, curr_val, Dimension::X);
                let x_simulation = Rc::new_cyclic(|weak: &Weak<RefCell<SpringSimulation>>| {
                    content_x.set_physic_animation_value(weak.clone());
                    RefCell::new(FlickAnimation::create_spring_animation(curr_val, limit_x))
                });
                Some(x_simulation as Rc<RefCell<dyn PositionSimulation>>)
            };

            let y_simulation = if inside_bounds_y {
                match velocity_estimation.as_ref() {
                    Some(velocity_estimation)
                        if velocity_estimation.velocity.y.abs()
                            >= FlickAnimation::minimum_flick_velocity_animation() =>
                    {
                        let content_y = (Flickable::FIELD_OFFSETS.content_y()).apply_pin(flick);
                        let carried_velocity_y = FlickAnimation::carried_momentum(
                            velocity_estimation.velocity.y,
                            self.retained_velocity.y,
                            flick.carry_momentum(),
                        );
                        let limit_y = Self::flick_limits(
                            flick_rc,
                            velocity_estimation.velocity.y,
                            Dimension::Y,
                        );
                        let y_simulation =
                            Rc::new_cyclic(|weak: &Weak<RefCell<FlickAnimation>>| {
                                let curr_val = content_y.get().0 as f32;
                                content_y.set_physic_animation_value(weak.clone());
                                RefCell::new(FlickAnimation::create_animation(
                                    FlickAnimationParameter::Velocity {
                                        velocity: velocity_estimation.velocity.y
                                            + carried_velocity_y,
                                    },
                                    effective_bounce(flick, &geo, Dimension::Y),
                                    curr_val,
                                    limit_y,
                                ))
                            });
                        Some(y_simulation as Rc<RefCell<dyn PositionSimulation>>)
                    }
                    _ => None,
                }
            } else {
                let content_y = (Flickable::FIELD_OFFSETS.content_y()).apply_pin(flick);
                let curr_val = content_y.get().0 as f32;
                // Spring back to whichever edge we're already past, not
                // wherever the release velocity happens to point.
                let limit_y = Self::flick_limits(flick_rc, curr_val, Dimension::Y);
                let y_simulation = Rc::new_cyclic(|weak: &Weak<RefCell<SpringSimulation>>| {
                    content_y.set_physic_animation_value(weak.clone());
                    RefCell::new(FlickAnimation::create_spring_animation(curr_val, limit_y))
                });
                Some(y_simulation as Rc<RefCell<dyn PositionSimulation>>)
            };

            if x_simulation.is_some() || y_simulation.is_some() {
                (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
            }

            self.running_animation = Some(RunningSimulation {
                start_time: crate::animations::current_tick(),
                weak: flick_rc.downgrade(),
                x_simulation,
                y_simulation,
            });
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
            MouseEvent::Pressed {
                position, button: PointerEventButton::Left, event_time, ..
            } => {
                if inner.capture_events.is_none() && !Self::can_pan(flick, flick_rc) {
                    // There is nothing to pan in either direction: don't hold up the press waiting to see if it turns into a drag,
                    // just let it fall through to whatever is underneath,
                    // the same way wheel events already are when the Flickable can't scroll in their direction.
                    return InputEventFilterResult::ForwardAndIgnore;
                }

                inner.pressed_mouse_state = Some((crate::animations::current_tick(), *position));
                inner.track_press(event_time.get().unwrap_or_else(crate::animations::current_tick));
                inner.capture_momentum();
                inner.last_scroll_event =
                    Some((crate::animations::current_tick(), Default::default())); // The position is not important
                let content_x = (Flickable::FIELD_OFFSETS.content_x()).apply_pin(flick);
                content_x.remove_binding(); // Stop animation by removing the binding
                let content_y = (Flickable::FIELD_OFFSETS.content_y()).apply_pin(flick);
                content_y.remove_binding(); // Stop animation by removing the binding

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
                    TouchPhase::Started => InputEventFilterResult::ForwardEvent,
                    TouchPhase::Moved => {
                        if inner.capture_events.is_some_and(|v| v == CaptureEvents::WheelMove) {
                            InputEventFilterResult::Intercept
                        } else {
                            // If we recently handled a wheel event, intercept it to prevent children from grabbing
                            // the scroll event
                            let delta = Self::scroll_delta(window_adapter, *delta_x, *delta_y);
                            if !FlickableDataInner::is_allowed_scroll_direction(
                                flick, delta, flick_rc,
                            ) {
                                inner.last_scroll_event = None;
                                InputEventFilterResult::ForwardEvent
                            } else if inner.should_capture_scroll(SCROLL_FILTER_DURATION, *position)
                                && inner.capture_events.is_none()
                            {
                                InputEventFilterResult::Intercept
                            } else {
                                ForwardEvent
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
        let content_width = flick.content_width();
        let content_height = flick.content_height();
        let zero = LogicalLength::zero();

        // We should capture the mouse movement, if the flickable can move in this
        // axis, and the mouse has moved more than the threshold in this axis.
        let should_capture_x = (FlickAnimation::use_bounce(effective_bounce(
            flick,
            &flickable_geometry,
            Dimension::X,
        )) || content_width > flickable_width
            || flick.content_x() != zero)
            && abs(mouse_delta.x_length()) > DISTANCE_THRESHOLD;
        let should_capture_y = (FlickAnimation::use_bounce(effective_bounce(
            flick,
            &flickable_geometry,
            Dimension::Y,
        )) || content_height > flickable_height
            || flick.content_y() != zero)
            && abs(mouse_delta.y_length()) > DISTANCE_THRESHOLD;
        should_capture_x || should_capture_y
    }

    /// Whether the flickable has any content to pan in either direction, regardless of mouse movement.
    /// Used to decide whether a press that no descendant claimed is worth grabbing,
    /// mirroring the direction check wheel events already get in `handle_mouse_filter`.
    fn can_pan(flick: Pin<&Flickable>, flick_rc: &ItemRc) -> bool {
        let flickable_geometry = Flickable::geometry_without_virtual_keyboard(flick_rc);
        let flickable_width = flickable_geometry.width_length();
        let flickable_height = flickable_geometry.height_length();
        let content_width = flick.content_width();
        let content_height = flick.content_height();
        let zero = LogicalLength::zero();

        let can_pan_x =
            FlickAnimation::use_bounce(effective_bounce(flick, &flickable_geometry, Dimension::X))
                || content_width > flickable_width
                || flick.content_x() != zero;
        let can_pan_y =
            FlickAnimation::use_bounce(effective_bounce(flick, &flickable_geometry, Dimension::Y))
                || content_height > flickable_height
                || flick.content_y() != zero;

        can_pan_x || can_pan_y
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
                inner.capture_events = Some(CaptureEvents::MouseStart);
                inner.capture_momentum();
                inner.last_scroll_event =
                    Some((crate::animations::current_tick(), Default::default()));
                InputEventResult::GrabMouse
            }
            MouseEvent::Exit | MouseEvent::Released { .. } => {
                inner.pressed_mouse_state = None;
                if let Some(c) = inner.capture_events {
                    if c == CaptureEvents::MouseMove {
                        inner.animate(flick, flick_rc);
                        inner.capture_events = None;
                        InputEventResult::EventAccepted
                    } else if c == CaptureEvents::MouseStart {
                        inner.capture_events = None;
                        InputEventResult::EventAccepted
                    } else {
                        // an accepted wheel event is followed by an Exit, so the wheel states must survive it
                        InputEventResult::EventIgnored
                    }
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Moved { position, event_time, history, .. } => {
                // Important constraint: The content_y might not be stable, and might jump around
                // wildly!
                // This is especially the case if a ListView is involved, which will continuously
                // update its own content_y to keep the current item visible, which can cause the
                // content_y to jump.
                //
                // So to correctly calculate the mouse delta, we need to use the position of
                // the mouse in the flickables coordinate system and never the content coordinate
                // system.
                if let Some((_pressed_time, pressed_position)) = inner.pressed_mouse_state {
                    let is_capturing = inner.capture_events.is_some_and(|f| {
                        matches!(f, CaptureEvents::MouseStart | CaptureEvents::MouseMove)
                    });
                    let tracking_delta =
                        *position - inner.last_mouse_position.unwrap_or(pressed_position);
                    let mut mouse_delta =
                        if is_capturing { tracking_delta } else { *position - pressed_position };
                    inner.track_move(
                        event_time.get().unwrap_or_else(crate::animations::current_tick),
                        *position,
                        history.get().unwrap_or(&TouchHistory::default()),
                    );

                    if is_capturing
                        || self.should_capture_mouse_direction(mouse_delta, flick, flick_rc)
                    {
                        // The drag event is meant to move the content, set it to the new position
                        // and start capturing mouse events.
                        let content_x = (Flickable::FIELD_OFFSETS.content_x()).apply_pin(flick);
                        let content_y = (Flickable::FIELD_OFFSETS.content_y()).apply_pin(flick);

                        if !is_capturing && event.is_from_touch() {
                            // Otherwise we'd jump instead of starting the drag smoothly.
                            mouse_delta.x =
                                FlickableDataInner::subtract_distance_threshold(mouse_delta.x);
                            mouse_delta.y =
                                FlickableDataInner::subtract_distance_threshold(mouse_delta.y);
                        }

                        let flicked = inner.scroll_move(
                            flick,
                            flick_rc,
                            *position,
                            mouse_delta,
                            &content_x,
                            &content_y,
                        );
                        if flicked {
                            (Flickable::FIELD_OFFSETS.flicked()).apply_pin(flick).call(&());
                        }
                        inner.capture_events = Some(CaptureEvents::MouseMove);

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

/// The effective bounce setting for one axis: the shared `bounce` property,
/// forced off when that axis has nothing to overflow into (the content
/// doesn't exceed the viewport on that axis, so there's nothing to
/// overscroll past).
fn effective_bounce(flick: Pin<&Flickable>, geo: &LogicalRect, dimension: Dimension) -> AutoBool {
    let has_overflow = match dimension {
        Dimension::X => flick.content_width() > geo.width_length(),
        Dimension::Y => flick.content_height() > geo.height_length(),
    };
    if has_overflow { flick.bounce() } else { AutoBool::Off }
}

/// Make sure that the point is within the bounds
fn ensure_in_bound(
    flick: Pin<&Flickable>,
    mut p: LogicalPoint,
    geo: &LogicalRect,
    use_bounce_x: bool,
    use_bounce_y: bool,
) -> LogicalPoint {
    let w = geo.width_length();
    let h = geo.height_length();
    let cw = flick.content_width();
    let ch = flick.content_height();

    if !use_bounce_x {
        p.x = p.x.max((w - cw).get()).min(Default::default());
    }
    if !use_bounce_y {
        p.y = p.y.max((h - ch).get()).min(Default::default());
    }
    p
}

fn inside_bounds(flick: Pin<&Flickable>, p: LogicalPoint, flick_rc: &ItemRc) -> (bool, bool) {
    let geo = Flickable::geometry_without_virtual_keyboard(flick_rc);
    let w = geo.width_length();
    let h = geo.height_length();
    let cw = flick.content_width();
    let ch = flick.content_height();

    let inside_bounds_x = p.x >= (w - cw).get() && p.x <= 0 as Coord;
    let inside_bounds_y = p.y >= (h - ch).get() && p.y <= 0 as Coord;

    (inside_bounds_x, inside_bounds_y)
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

#[cfg(test)]
mod velocity_history_tests {
    use super::*;

    #[test]
    fn original_sample_time_is_independent_of_delivery_delay() {
        let start = crate::animations::current_tick();
        crate::animations::update_animations(start + Duration::from_millis(100));
        let mut inner = FlickableDataInner {
            pressed_mouse_state: Some((start, LogicalPoint::new(50., 100.))),
            ..Default::default()
        };
        inner.track_press(start);
        inner.track_move(
            start + Duration::from_millis(20),
            LogicalPoint::new(50., 220.),
            &TouchHistory::default(),
        );
        assert_eq!(inner.velocity_rb.last_time(), Some(start + Duration::from_millis(20)));
    }

    #[test]
    fn delayed_touch_press_preserves_short_flick_velocity() {
        use crate::input::TouchState;

        for frame_offset_ms in [-8, 6, 20] {
            for batched in [false, true] {
                for sign in [-1., 1.] {
                    let start = crate::animations::current_tick() + Duration::from_millis(100);
                    let press_frame = if frame_offset_ms < 0 {
                        start - Duration::from_millis((-frame_offset_ms) as u64)
                    } else {
                        start + Duration::from_millis(frame_offset_ms as u64)
                    };
                    let first = start + Duration::from_micros(7000);
                    let end = start + Duration::from_micros(10500);
                    let press_position = LogicalPoint::new(100., 200.);
                    let first_position = press_position + LogicalVector::new(0., sign * 42.);
                    let end_position = press_position + LogicalVector::new(0., sign * 63.);
                    let mut touch = TouchState::default();
                    let pressed = touch.process(
                        0,
                        press_position,
                        TouchPhase::Started,
                        Some(start),
                        TouchHistory::default(),
                    );
                    let mut inner = FlickableDataInner::default();
                    for event in pressed.into_iter() {
                        if let MouseEvent::Pressed { position, event_time, .. } = event {
                            inner.pressed_mouse_state = Some((press_frame, position));
                            inner.track_press(event_time.get().unwrap_or(press_frame));
                        }
                    }
                    let moves = if batched {
                        alloc::vec![(
                            end_position,
                            end,
                            TouchHistory { history: alloc::vec![(first_position, first)] }
                        )]
                    } else {
                        alloc::vec![
                            (first_position, first, TouchHistory::default()),
                            (end_position, end, TouchHistory::default()),
                        ]
                    };
                    for (position, time, history) in moves {
                        let events =
                            touch.process(0, position, TouchPhase::Moved, Some(time), history);
                        for event in events.into_iter() {
                            if let MouseEvent::Moved { position, event_time, history, .. } = event {
                                assert_eq!(event_time.get(), Some(time));
                                inner.track_move(
                                    event_time.get().unwrap_or(start + Duration::from_millis(100)),
                                    position,
                                    history.get().unwrap_or(&TouchHistory::default()),
                                );
                            }
                        }
                    }
                    let mut reference = VelocityTracker::default();
                    reference.push(start, LogicalVector::default());
                    reference.push(first, LogicalVector::new(0., sign * 42.));
                    reference.push(end, LogicalVector::new(0., sign * 21.));
                    let actual = inner.velocity_rb.estimate_velocity().unwrap().velocity;
                    let expected = reference.estimate_velocity().unwrap().velocity;
                    assert_eq!(
                        actual, expected,
                        "frame offset {frame_offset_ms}, batched {batched}, sign {sign}"
                    );
                }
            }
        }
    }

    #[test]
    fn coalesced_history_preserves_leading_segment() {
        for sign in [-1., 1.] {
            for historical_positions in [alloc::vec![], alloc::vec![15], alloc::vec![12, 15]] {
                let start = crate::animations::current_tick();
                let press_position = LogicalPoint::new(45., 125.);
                let mut inner = FlickableDataInner {
                    pressed_mouse_state: Some((start, press_position)),
                    ..Default::default()
                };
                inner.track_press(start);
                inner.track_move(
                    start + Duration::from_millis(5),
                    press_position + LogicalVector::new(sign * 5., sign * 10.),
                    &TouchHistory::default(),
                );
                inner.track_move(
                    start + Duration::from_millis(10),
                    press_position + LogicalVector::new(sign * 10., sign * 20.),
                    &TouchHistory::default(),
                );
                let end = start + Duration::from_millis(20);
                let history = TouchHistory {
                    history: historical_positions
                        .into_iter()
                        .map(|time| {
                            (
                                press_position
                                    + LogicalVector::new(
                                        sign * time as f32,
                                        sign * time as f32 * 2.,
                                    ),
                                start + Duration::from_millis(time),
                            )
                        })
                        .collect(),
                };
                inner.track_move(
                    end,
                    press_position + LogicalVector::new(sign * 20., sign * 40.),
                    &history,
                );
                crate::animations::update_animations(end);
                let estimate = inner.velocity_rb.estimate_velocity().unwrap();
                assert!(
                    (estimate.velocity.x - sign * 1000.).abs() < 0.01,
                    "{}",
                    estimate.velocity.x
                );
                assert!(
                    (estimate.velocity.y - sign * 2000.).abs() < 0.01,
                    "{}",
                    estimate.velocity.y
                );
            }
        }
    }

    #[test]
    fn coalesced_history_preserves_zero_leading_movement() {
        let start = crate::animations::current_tick();
        let press_position = LogicalPoint::new(100., 200.);
        let mut with_history = FlickableDataInner::default();
        let mut without_history = FlickableDataInner::default();
        for inner in [&mut with_history, &mut without_history] {
            inner.pressed_mouse_state = Some((start, press_position));
            inner.track_press(start);
            inner.track_move(
                start + Duration::from_millis(10),
                press_position + LogicalVector::new(0., 10.),
                &TouchHistory::default(),
            );
        }
        let middle = start + Duration::from_millis(15);
        let end = start + Duration::from_millis(20);
        with_history.track_move(
            end,
            press_position + LogicalVector::new(0., 20.),
            &TouchHistory {
                history: alloc::vec![(press_position + LogicalVector::new(0., 10.), middle)],
            },
        );
        without_history.track_move(
            middle,
            press_position + LogicalVector::new(0., 10.),
            &TouchHistory::default(),
        );
        without_history.track_move(
            end,
            press_position + LogicalVector::new(0., 20.),
            &TouchHistory::default(),
        );
        crate::animations::update_animations(end);
        assert_eq!(
            with_history.velocity_rb.estimate_velocity().unwrap().velocity,
            without_history.velocity_rb.estimate_velocity().unwrap().velocity,
        );
    }

    #[test]
    fn transformed_history_preserves_velocity() {
        use crate::input::TouchState;
        use crate::lengths::ItemTransform;

        for translation in [LogicalVector::default(), LogicalVector::new(-90., -180.)] {
            for (transform, expected_velocity) in [
                (ItemTransform::identity(), LogicalVector::new(1000., -2000.)),
                (ItemTransform::new(0., 2., -3., 0., 50., 60.), LogicalVector::new(6000., 2000.)),
            ] {
                for batched in [false, true] {
                    let start = crate::animations::current_tick();
                    let press_position = LogicalPoint::new(100., 200.);
                    let mut touch = TouchState::default();
                    let mut inner = FlickableDataInner::default();
                    let pressed = touch.process(
                        0,
                        press_position,
                        TouchPhase::Started,
                        Some(start),
                        TouchHistory::default(),
                    );
                    for mut event in pressed.into_iter() {
                        event.translate(translation);
                        event.transform(transform);
                        if let MouseEvent::Pressed { position, .. } = event {
                            inner.pressed_mouse_state = Some((start, position));
                            inner.track_press(start);
                        }
                    }
                    let samples = [5, 10, 15, 20].map(|time| {
                        (
                            press_position + LogicalVector::new(time as f32, -2. * time as f32),
                            start + Duration::from_millis(time),
                        )
                    });
                    for batch in samples.chunks(if batched { 2 } else { 1 }) {
                        let (&(position, time), historical_positions) = batch.split_last().unwrap();
                        let events = touch.process(
                            0,
                            position,
                            TouchPhase::Moved,
                            Some(time),
                            TouchHistory { history: historical_positions.to_vec() },
                        );
                        for mut event in events.into_iter() {
                            event.translate(translation);
                            event.transform(transform);
                            if let MouseEvent::Moved { position, event_time, history, .. } = event {
                                inner.track_move(
                                    event_time.get().unwrap(),
                                    position,
                                    history.get().unwrap_or(&TouchHistory::default()),
                                );
                            }
                        }
                    }
                    crate::animations::update_animations(start + Duration::from_millis(20));
                    let actual = inner.velocity_rb.estimate_velocity().unwrap().velocity;
                    assert!(
                        (actual - expected_velocity).length() < 0.01,
                        "velocity {actual:?}, expected {expected_velocity:?}, batched {batched}"
                    );
                }
            }
        }
    }
}
