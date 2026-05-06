// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod interpreter;
use std::path::PathBuf;

pub use interpreter::*;

mod types;
pub use types::*;

mod uv_event_loop;
pub use uv_event_loop::*;

use napi::Env;
use napi::bindgen_prelude::*;

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn mock_elapsed_time(_ms: f64) {
    #[cfg(feature = "testing")]
    i_slint_backend_testing::mock_elapsed_time(_ms as u64);
}

#[napi]
pub fn get_mocked_time() -> f64 {
    #[cfg(feature = "testing")]
    return i_slint_backend_testing::get_mocked_time() as f64;
    #[cfg(not(feature = "testing"))]
    return 0.0;
}

#[napi]
pub enum ProcessEventsResult {
    Continue,
    Exited,
}

fn process_events_with_timeout(timeout: std::time::Duration) -> napi::Result<ProcessEventsResult> {
    i_slint_backend_selector::with_platform(|b| {
        b.process_events(timeout, i_slint_core::InternalToken)
    })
    .map_err(|e| napi::Error::from_reason(e.to_string()))
    .map(|result| match result {
        core::ops::ControlFlow::Break(()) => ProcessEventsResult::Exited,
        core::ops::ControlFlow::Continue(()) => ProcessEventsResult::Continue,
    })
}

#[napi]
pub fn process_events() -> napi::Result<ProcessEventsResult> {
    process_events_with_timeout(std::time::Duration::ZERO)
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
