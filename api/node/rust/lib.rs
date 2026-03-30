// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod interpreter;
use std::path::PathBuf;

pub use interpreter::*;

mod types;
pub use types::*;

use napi::Env;
use napi::bindgen_prelude::*;

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn mock_elapsed_time(ms: f64) {
    i_slint_core::tests::slint_mock_elapsed_time(ms as _);
}

#[napi]
pub fn get_mocked_time() -> f64 {
    i_slint_core::tests::slint_get_mocked_time() as f64
}

#[napi]
pub enum ProcessEventsResult {
    Continue,
    Exited,
}

#[napi]
pub fn process_events() -> napi::Result<ProcessEventsResult> {
    i_slint_backend_selector::with_platform(|b| {
        b.process_events(std::time::Duration::ZERO, i_slint_core::InternalToken)
    })
    .map_err(|e| napi::Error::from_reason(e.to_string()))
    .map(|result| match result {
        core::ops::ControlFlow::Continue(()) => ProcessEventsResult::Continue,
        core::ops::ControlFlow::Break(()) => ProcessEventsResult::Exited,
    })
}

#[napi]
pub fn invoke_from_event_loop(env: &Env, callback: DynFunction<'_>) -> napi::Result<()> {
    i_slint_backend_selector::with_platform(|_b| {
        // Nothing to do, just make sure a backend was created
        Ok(())
    })
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let stored_fn = StoredFunction::new(&callback)?;
    let env = *env;
    let wrapper = send_wrapper::SendWrapper::new((stored_fn, env));
    i_slint_core::api::invoke_from_event_loop(move || {
        let (stored_fn, env) = wrapper.take();
        if stored_fn.call(&env, vec![]).is_err() {
            eprintln!("Node.js: JavaScript invoke_from_event_loop threw an exception");
        }
    })
    .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn set_quit_on_last_window_closed(quit_on_last_window_closed: bool) -> napi::Result<()> {
    if !quit_on_last_window_closed {
        i_slint_backend_selector::with_platform(|b| {
            #[allow(deprecated)]
            b.set_event_loop_quit_on_last_window_closed(false);
            Ok(())
        })
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    }
    Ok(())
}

#[napi]
pub fn init_testing() {
    #[cfg(feature = "testing")]
    i_slint_backend_testing::init_integration_test_with_mock_time();
}

#[napi]
pub fn init_translations(domain: String, dir_name: String) -> napi::Result<()> {
    i_slint_core::translations::gettext_bindtextdomain(domain.as_str(), PathBuf::from(dir_name))
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn set_xdg_app_id(app_id: String) -> napi::Result<()> {
    i_slint_backend_selector::with_global_context(|ctx| ctx.set_xdg_app_id(app_id.into()))
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

pub fn print_to_console(env: Env, function: &str, arguments: core::fmt::Arguments) {
    let Ok(global) = env.get_global() else {
        eprintln!("Unable to obtain global object");
        return;
    };

    let console_object: Object = match global.get_named_property("console") {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Unable to obtain console object for logging");
            return;
        }
    };

    let log_fn: Function<Unknown, Unknown> = match console_object.get_named_property(function) {
        Ok(f) => f,
        Err(_) => {
            eprintln!("Unable to obtain console.{function}");
            return;
        }
    };

    let message = arguments.to_string();
    let Ok(js_message) = env.create_string(&message) else {
        eprintln!("Unable to provide log message to JS env");
        return;
    };

    let Ok(js_message_unknown) = js_message.into_unknown(&env) else {
        eprintln!("Unable to convert log message to unknown");
        return;
    };

    if let Err(err) = log_fn.apply(console_object, js_message_unknown) {
        eprintln!("Unable to invoke console.{function}: {err}");
    }
}

#[macro_export]
macro_rules! console_err {
    ($env:expr, $($t:tt)*) => ($crate::print_to_console($env, "error", format_args!($($t)*)))
}
