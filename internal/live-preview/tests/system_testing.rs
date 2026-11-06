// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(feature = "system-testing")]

use i_slint_live_preview::protocol::{
    PreviewToDebuggerMessage, PreviewToLsp, PreviewToLspMessage, Result,
};
use i_slint_live_preview::system_testing::answer_request;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct CaptureToDebugger(RefCell<Vec<PreviewToDebuggerMessage>>);

impl PreviewToLsp for CaptureToDebugger {
    fn send(&self, _: &PreviewToLspMessage) -> Result<()> {
        unreachable!("System-test responses go to the debugger")
    }

    fn send_to_debugger(&self, message: &PreviewToDebuggerMessage) -> Result<()> {
        self.0.borrow_mut().push(message.clone());
        Ok(())
    }
}

#[test]
fn answers_on_the_event_loop() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let capture = Rc::new(CaptureToDebugger::default());
    let to_editor: Rc<dyn PreviewToLsp> = capture.clone();
    answer_request(7, vec![0xff], Rc::downgrade(&to_editor));
    i_slint_backend_selector::with_global_context(|context| {
        context.spawn_local(async { i_slint_core::api::quit_event_loop().unwrap() }).unwrap();
        context.run_event_loop().unwrap();
    })
    .unwrap();
    assert!(matches!(
        capture.0.take().as_slice(),
        [PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Ok(response) }]
            if !response.is_empty()
    ));
}
