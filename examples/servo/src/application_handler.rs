use std::{cell::RefCell, rc::Rc};

use slint::winit_030::{CustomApplicationHandler, EventResult};

use crate::adapter::SlintServoAdapter;

pub struct ApplicationHandler {
    pub state: Rc<RefCell<Option<Rc<SlintServoAdapter>>>>,
}

impl ApplicationHandler {
    pub fn new(state: Rc<RefCell<Option<Rc<SlintServoAdapter>>>>) -> Self {
        Self { state }
    }
}

impl CustomApplicationHandler for ApplicationHandler {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _winit_window: Option<&winit::window::Window>,
        _slint_window: Option<&slint::Window>,
        _event: &winit::event::WindowEvent,
    ) -> EventResult {
        let state = self.state.borrow();
        let state = state.as_ref().unwrap();

        let _ = state.waker_sender().try_send(());

        return EventResult::Propagate;
    }
}
