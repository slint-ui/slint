// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [WindowAdapter] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use crate::EventResult;
use crate::winitwindowadapter::WindowVisibility;
use crate::{SharedBackendData, SlintEvent};
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
    /// On wasm request_redraw doesn't wake the event loop, so we need to manually send an event
    /// so that the event loop can run
    #[cfg(target_arch = "wasm32")]
    WakeEventLoopWorkaround,
    /// Slint internal: Invoke the
    UserEvent(Box<dyn FnOnce() + Send>),
    /// Invoke the callback with the [`ActiveEventLoop`], for [`crate::invoke_from_active_event_loop`]
    UserEventWithEventLoop(Box<dyn FnOnce(&ActiveEventLoop) + Send>),
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
            #[cfg(target_arch = "wasm32")]
            Self::WakeEventLoopWorkaround => write!(f, "WakeEventLoopWorkaround"),
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
}

impl winit::application::ApplicationHandler<SlintEvent> for EventLoopState {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(
            self.custom_application_handler
                .as_mut()
                .map_or(EventResult::Propagate, |handler| { handler.resumed(event_loop) }),
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
        event_loop: &ActiveEventLoop,
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

        if let Err(err) = window.dispatch_winit_window_event(event_loop, &winit_window, &event) {
            self.loop_error = Some(err);
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: SlintEvent) {
        match event.0 {
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
                        .expect("internal error: accesskit adapter must exist when window exists")
                        .borrow_mut()
                        .process_accesskit_event(window_event);
                    // access kit adapter not borrowed anymore, now invoke the deferred action
                    if let Some(deferred_action) = deferred_action {
                        deferred_action.invoke(window.window());
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            CustomEvent::WakeEventLoopWorkaround => {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            #[cfg(muda)]
            CustomEvent::Muda(event) => {
                if let Some((window, eid, muda_type)) =
                    event.id().0.split_once('|').and_then(|(w, e)| {
                        let (e, muda_type) = e.split_once('|')?;
                        Some((
                            self.shared_backend_data.window_by_id(
                                winit::window::WindowId::from(w.parse::<u64>().ok()?),
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

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
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
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.device_event(event_loop, device_id, event);
        }
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.suspended(event_loop);
        }
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.exiting(event_loop);
        }
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
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
                winit_loop
                    .run_app(&mut self)
                    .map_err(|e| format!("Error running winit event loop: {e}"))?;
                // This can't really happen, as run() doesn't return
                Ok(Self::new(self.shared_backend_data.clone(), None))
            } else {
                use winit::platform::run_on_demand::EventLoopExtRunOnDemand as _;
                winit_loop
                    .run_app_on_demand(&mut self)
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
    ) -> Result<(Self, winit::platform::pump_events::PumpStatus), corelib::platform::PlatformError>
    {
        use winit::platform::pump_events::EventLoopExtPumpEvents;

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
        use winit::platform::web::EventLoopExtWebSys;
        let not_running_loop_instance = self
            .shared_backend_data
            .not_running_event_loop
            .take()
            .ok_or_else(|| PlatformError::from("Nested event loops are not supported"))?;

        not_running_loop_instance.spawn_app(self);

        Ok(())
    }
}
