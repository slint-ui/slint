// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [WindowAdapter] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use crate::EventResult;
use crate::SharedBackendData;
use crate::winitwindowadapter::WindowVisibility;
use corelib::platform::PlatformError;
use corelib::window::*;
use i_slint_core as corelib;

#[allow(unused_imports)]
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};

/// This enum captures run-time specific events that can be dispatched to the event loop in
/// addition to the winit events.
pub enum CustomEvent {
    /// Slint internal: Invoke the
    UserEvent(Box<dyn FnOnce() + Send>),
    /// Invoke the callback with the [`ActiveEventLoop`], for [`crate::invoke_from_active_event_loop`]
    UserEventWithEventLoop(Box<dyn FnOnce(&dyn ActiveEventLoop) + Send>),
    /// Emitted from quit_event_loop with the current event loop generation
    Exit(usize),
    #[cfg(enable_accesskit)]
    Accesskit(accesskit_winit::Event),
    #[cfg(muda)]
    Muda(muda::MenuEvent),
}

impl std::fmt::Debug for CustomEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserEvent(_) => write!(f, "UserEvent"),
            Self::UserEventWithEventLoop(_) => write!(f, "UserEventWithEventLoop"),
            Self::Exit(_) => write!(f, "Exit"),
            #[cfg(enable_accesskit)]
            Self::Accesskit(a) => write!(f, "AccessKit({a:?})"),
            #[cfg(muda)]
            Self::Muda(e) => write!(f, "Muda({e:?})"),
        }
    }
}

pub struct EventLoopState {
    shared_backend_data: Rc<SharedBackendData>,

    loop_error: Option<PlatformError>,

    /// Set to true when pumping events for the shortest amount of time possible.
    pumping_events_instantly: bool,

    custom_application_handler: Option<Box<dyn crate::CustomApplicationHandler>>,
}

impl EventLoopState {
    pub fn new(
        shared_backend_data: Rc<SharedBackendData>,
        custom_application_handler: Option<Box<dyn crate::CustomApplicationHandler>>,
    ) -> Self {
        Self {
            shared_backend_data,
            loop_error: Default::default(),
            pumping_events_instantly: Default::default(),
            custom_application_handler,
        }
    }

    /// Free graphics resources for any hidden windows. Called when quitting the event loop, to work
    /// around #8795.
    fn suspend_all_hidden_windows(&self) {
        let windows_to_suspend = self
            .shared_backend_data
            .active_windows
            .borrow()
            .values()
            .filter_map(|w| w.upgrade())
            .filter(|w| matches!(w.visibility(), WindowVisibility::Hidden))
            .collect::<Vec<_>>();
        for window in windows_to_suspend.into_iter() {
            let _ = window.suspend();
        }
    }

    /// Hand a native drag built by `WinitWindowAdapter::start_drag` to winit, now that the
    /// `ActiveEventLoop` is in hand.
    ///
    /// Falls back to the in-window drag if the native start fails, so the gesture isn't lost.
    fn start_drag_if_pending(&mut self, event_loop: &dyn ActiveEventLoop) {
        let Some(drag) = self.shared_backend_data.pending_drag.borrow_mut().take() else {
            return;
        };

        if event_loop.start_drag(drag.window_id, drag.data, &drag.actions, drag.icon).is_err()
            && let Some(window) = self.shared_backend_data.window_by_id(drag.window_id)
        {
            WindowInner::from_pub(window.window()).start_in_window_drag();
        }
    }
}

/// Decode the encoded image bytes of an incoming drag, without going through the
/// image cache.
pub(crate) fn decode_dropped_image(
    bytes: &[u8],
    extension_hint: Option<&str>,
) -> Option<corelib::graphics::Image> {
    corelib::graphics::load_image_from_dynamic_data(
        bytes.into(),
        extension_hint.unwrap_or_default().as_bytes().into(),
    )
}

/// Map a winit drag action to Slint's `DragAction`. A `None` (e.g. unknown) action becomes
/// `DragAction::None`.
pub(crate) fn dnd_action_to_slint(
    action: Option<winit::event_loop::DndAction>,
) -> corelib::items::DragAction {
    use corelib::items::DragAction;
    use winit::event_loop::DndAction;
    match action {
        Some(DndAction::Move) => DragAction::Move,
        Some(DndAction::Copy) => DragAction::Copy,
        Some(DndAction::Link) => DragAction::Link,
        Some(DndAction::Ask) | Some(DndAction::Private) | None => DragAction::None,
    }
}

/// The action proposed by the OS for an incoming drag, defaulting to `Copy` when the platform
/// did not supply one (some platforms, such as X11, only report the action when the drop
/// completes).
pub(crate) fn proposed_action_or_copy(
    action: Option<winit::event_loop::DndAction>,
) -> corelib::items::DragAction {
    let action = dnd_action_to_slint(action);
    if action == corelib::items::DragAction::None {
        corelib::items::DragAction::Copy
    } else {
        action
    }
}

/// Map a `DropArea`'s chosen action to the single valid winit drag action to report to the OS,
/// or `None` to reject the drag. Returned as an `Option` so the per-event report on the drag
/// hot path needs no allocation.
pub(crate) fn slint_action_to_dnd(
    action: corelib::items::DragAction,
) -> Option<winit::event_loop::DndAction> {
    use corelib::items::DragAction;
    match action {
        DragAction::Move => Some(winit::event_loop::DndAction::Move),
        DragAction::Copy => Some(winit::event_loop::DndAction::Copy),
        DragAction::Link => Some(winit::event_loop::DndAction::Link),
        DragAction::None => None,
        // `DragAction` is `#[non_exhaustive]`, so a catch-all is still required.
        #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
        _ => None,
    }
}

impl winit::application::ApplicationHandler for EventLoopState {
    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.resumed(event_loop);
        }
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if matches!(
            self.custom_application_handler.as_mut().map_or(EventResult::Propagate, |handler| {
                handler.can_create_surfaces(event_loop)
            }),
            EventResult::PreventDefault
        ) {
            return;
        }
        if let Err(err) = self.shared_backend_data.create_inactive_windows(event_loop) {
            self.loop_error = Some(err);
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.shared_backend_data.window_by_id(window_id) else {
            if let Some(handler) = self.custom_application_handler.as_mut() {
                handler.window_event(event_loop, window_id, None, None, &event);
            }
            return;
        };

        let Some(winit_window) = window.winit_window() else {
            return;
        };

        if matches!(
            self.custom_application_handler.as_mut().map_or(EventResult::Propagate, |handler| {
                handler.window_event(
                    event_loop,
                    window_id,
                    Some(&*winit_window),
                    Some(window.window()),
                    &event,
                )
            }),
            EventResult::PreventDefault
        ) {
            return;
        }

        let result = window.dispatch_winit_window_event(event_loop, &*winit_window, event);

        // A `DragArea` may have requested a native drag above; start it now while the
        // `ActiveEventLoop` is in hand.
        self.start_drag_if_pending(event_loop);

        if let Err(err) = result {
            self.loop_error = Some(err);
            event_loop.exit();
        }
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut()
            && matches!(handler.proxy_wake_up(event_loop), EventResult::PreventDefault)
        {
            return;
        }
        let events = std::mem::take(&mut *self.shared_backend_data.event_queue.lock().unwrap());
        for event in events {
            match event {
                CustomEvent::UserEvent(user_callback) => user_callback(),
                CustomEvent::UserEventWithEventLoop(user_callback) => user_callback(event_loop),
                CustomEvent::Exit(generation) => {
                    if self
                        .shared_backend_data
                        .event_loop_generation
                        .load(std::sync::atomic::Ordering::Relaxed)
                        == generation
                    {
                        self.suspend_all_hidden_windows();
                        event_loop.exit()
                    }
                    // else ignore the event, since it's from a previous run of the event loop
                }
                #[cfg(enable_accesskit)]
                CustomEvent::Accesskit(accesskit_winit::Event { window_id, window_event }) => {
                    if let Some(window) = self.shared_backend_data.window_by_id(window_id) {
                        let deferred_action = window
                            .accesskit_adapter()
                            .expect(
                                "internal error: accesskit adapter must exist when window exists",
                            )
                            .borrow_mut()
                            .process_accesskit_event(window_event);
                        // access kit adapter not borrowed anymore, now invoke the deferred action
                        if let Some(deferred_action) = deferred_action {
                            deferred_action.invoke(window.window());
                        }
                    }
                }
                #[cfg(muda)]
                CustomEvent::Muda(event) => {
                    if let Some((window, eid, muda_type)) =
                        event.id().0.split_once('|').and_then(|(w, e)| {
                            let (e, muda_type) = e.split_once('|')?;
                            Some((
                                self.shared_backend_data.window_by_id(
                                    winit::window::WindowId::from_raw(w.parse::<usize>().ok()?),
                                )?,
                                e.parse::<usize>().ok()?,
                                muda_type.parse::<crate::muda::MudaType>().ok()?,
                            ))
                        })
                    {
                        window.muda_event(eid, muda_type);
                    };
                }
            }
        }
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: winit::event::StartCause) {
        if matches!(
            self.custom_application_handler.as_mut().map_or(EventResult::Propagate, |handler| {
                handler.new_events(event_loop, cause)
            }),
            EventResult::PreventDefault
        ) {
            return;
        }

        event_loop.set_control_flow(ControlFlow::Wait);

        self.shared_backend_data.context().update_timers_and_animations();
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.shared_backend_data.flush_pending_mouse_move();

        if matches!(
            self.custom_application_handler
                .as_mut()
                .map_or(EventResult::Propagate, |handler| { handler.about_to_wait(event_loop) }),
            EventResult::PreventDefault
        ) {
            return;
        }

        if let Err(err) = self.shared_backend_data.create_inactive_windows(event_loop) {
            self.loop_error = Some(err);
        }

        if !event_loop.exiting() {
            for w in self
                .shared_backend_data
                .active_windows
                .borrow()
                .values()
                .filter_map(|w| w.upgrade())
            {
                if w.window().has_active_animations() {
                    w.request_redraw();
                }
            }
        }

        if event_loop.control_flow() == ControlFlow::Wait
            && let Some(next_timer) =
                self.shared_backend_data.context().duration_until_next_timer_update()
        {
            event_loop.set_control_flow(ControlFlow::wait_duration(next_timer));
        }

        if self.pumping_events_instantly {
            event_loop.set_control_flow(ControlFlow::Poll);
        }
    }

    fn device_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        device_id: Option<winit::event::DeviceId>,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.device_event(event_loop, device_id, event);
        }
    }

    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.suspended(event_loop);
        }
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.destroy_surfaces(event_loop);
        }
    }

    fn memory_warning(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.memory_warning(event_loop);
        }
    }
}

impl EventLoopState {
    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[allow(unused_mut)] // mut need changes for wasm
    pub fn run(mut self) -> Result<Self, corelib::platform::PlatformError> {
        let not_running_loop_instance = self
            .shared_backend_data
            .not_running_event_loop
            .take()
            .ok_or_else(|| PlatformError::from("Nested event loops are not supported"))?;
        let mut winit_loop = not_running_loop_instance;

        cfg_if::cfg_if! {
            if #[cfg(any(target_arch = "wasm32", ios_and_friends))] {
                let shared_backend_data = self.shared_backend_data.clone();
                winit_loop
                    .run_app(self)
                    .map_err(|e| format!("Error running winit event loop: {e}"))?;
                // On wasm, run_app registers the app and returns immediately.
                // On iOS, run_app blocks until the app exits.
                Ok(Self::new(shared_backend_data, None))
            } else {
                winit::event_loop::run_on_demand::EventLoopExtRunOnDemand::run_app_on_demand(&mut winit_loop, &mut self)
                    .map_err(|e| format!("Error running winit event loop: {e}"))?;

                // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
                // Winit does not support creating multiple instances of the event loop.
                self.shared_backend_data.not_running_event_loop.replace(Some(winit_loop));

                if let Some(error) = self.loop_error {
                    return Err(error);
                }
                Ok(self)
            }
        }
    }

    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[cfg(all(not(target_arch = "wasm32"), not(ios_and_friends)))]
    pub fn pump_events(
        mut self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(Self, winit::event_loop::pump_events::PumpStatus), corelib::platform::PlatformError>
    {
        use winit::event_loop::pump_events::EventLoopExtPumpEvents as _;

        let not_running_loop_instance = self
            .shared_backend_data
            .not_running_event_loop
            .take()
            .ok_or_else(|| PlatformError::from("Nested event loops are not supported"))?;
        let mut winit_loop = not_running_loop_instance;

        self.pumping_events_instantly = timeout.is_some_and(|duration| duration.is_zero());

        let result = winit_loop.pump_app_events(timeout, &mut self);

        self.pumping_events_instantly = false;

        // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
        // Winit does not support creating multiple instances of the event loop.
        self.shared_backend_data.not_running_event_loop.replace(Some(winit_loop));

        if let Some(error) = self.loop_error {
            return Err(error);
        }
        Ok((self, result))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn spawn(self) -> Result<(), corelib::platform::PlatformError> {
        let not_running_loop_instance = self
            .shared_backend_data
            .not_running_event_loop
            .take()
            .ok_or_else(|| PlatformError::from("Nested event loops are not supported"))?;

        not_running_loop_instance
            .run_app(self)
            .map_err(|e| format!("Error running winit event loop: {e}"))?;

        Ok(())
    }
}
