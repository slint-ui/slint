// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Input and time simulation for the tests of `tests/cases`, like `private_api` in Node.js.

use i_slint_backend_testing::{TestingBackend, TestingBackendOptions, testing_backend};
use i_slint_core::window::{WindowAdapterRc, WindowInner};
use slint_interpreter::ComponentHandle;

use crate::instance::DotnetInstance;
use crate::{Utf8Str, guard_bool, utf8_slice};

fn with_adapter(
    instance: &DotnetInstance,
    f: impl FnOnce(&WindowAdapterRc) -> Result<(), String>,
) -> bool {
    guard_bool(|| instance.with(|i| f(&WindowInner::from_pub(i.window()).window_adapter())))
}

/// Installs the testing backend with mocked time, like `initTesting` in Node.js.
/// Call it before anything else uses Slint.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_testing_init() -> bool {
    let options = TestingBackendOptions { mock_time: true, threading: true, ..Default::default() };
    let result = i_slint_core::platform::set_platform(Box::new(TestingBackend::new(options)))
        .map_err(|e| e.to_string());
    // Claims the Slint thread, which picks up the platform.
    guard_bool(|| result)
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_testing_mock_elapsed_time(ms: u64) -> bool {
    guard_bool(|| {
        testing_backend::mock_elapsed_time(ms);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_testing_get_mocked_time() -> u64 {
    testing_backend::get_mocked_time()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_testing_send_mouse_click(
    instance: &DotnetInstance,
    x: f32,
    y: f32,
) -> bool {
    with_adapter(instance, |adapter| {
        testing_backend::send_mouse_click(x, y, adapter);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_testing_send_keyboard_string_sequence(
    instance: &DotnetInstance,
    sequence: Utf8Str,
) -> bool {
    with_adapter(instance, |adapter| {
        let sequence = unsafe { sequence.as_str() }?;
        testing_backend::send_keyboard_string_sequence(&sequence.into(), adapter);
        Ok(())
    })
}

/// Presses the keys in order, then releases them in reverse order.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_testing_send_key_combo(
    instance: &DotnetInstance,
    keys: *const Utf8Str,
    count: usize,
) -> bool {
    use i_slint_core::platform::WindowEvent;
    with_adapter(instance, |adapter| {
        let keys = unsafe { utf8_slice(keys, count) }?;
        let window = adapter.window();
        for key in &keys {
            window.dispatch_event(WindowEvent::KeyPressed { text: (*key).into() });
        }
        for key in keys.iter().rev() {
            window.dispatch_event(WindowEvent::KeyReleased { text: (*key).into() });
        }
        Ok(())
    })
}
