// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The window of a component. A live component keeps its window across reloads,
//! so these work on the window of the latest instance.

use std::ffi::c_void;

use slint_interpreter::{
    CloseRequestResponse, ComponentHandle, LogicalPosition, LogicalSize, PhysicalPosition,
    PhysicalSize, Window,
};

use crate::guard;
use crate::instance::DotnetInstance;

fn with_window<R>(instance: &DotnetInstance, on_error: R, f: impl FnOnce(&Window) -> R) -> R {
    guard(on_error, || Ok(instance.with(|i| f(i.window()))))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_is_visible(instance: &DotnetInstance) -> bool {
    with_window(instance, false, |w| w.is_visible())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_scale_factor(instance: &DotnetInstance) -> f32 {
    with_window(instance, 0., |w| w.scale_factor())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_request_redraw(instance: &DotnetInstance) {
    with_window(instance, (), |w| w.request_redraw())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_position(
    instance: &DotnetInstance,
    x: &mut i32,
    y: &mut i32,
) {
    with_window(instance, (), |w| {
        let position = w.position();
        (*x, *y) = (position.x, position.y);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_physical_position(
    instance: &DotnetInstance,
    x: i32,
    y: i32,
) {
    with_window(instance, (), |w| w.set_position(PhysicalPosition::new(x, y)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_logical_position(
    instance: &DotnetInstance,
    x: f32,
    y: f32,
) {
    with_window(instance, (), |w| w.set_position(LogicalPosition::new(x, y)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_size(
    instance: &DotnetInstance,
    width: &mut u32,
    height: &mut u32,
) {
    with_window(instance, (), |w| {
        let size = w.size();
        (*width, *height) = (size.width, size.height);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_physical_size(
    instance: &DotnetInstance,
    width: u32,
    height: u32,
) {
    with_window(instance, (), |w| w.set_size(PhysicalSize::new(width, height)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_logical_size(
    instance: &DotnetInstance,
    width: f32,
    height: f32,
) {
    with_window(instance, (), |w| w.set_size(LogicalSize::new(width, height)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_is_fullscreen(instance: &DotnetInstance) -> bool {
    with_window(instance, false, |w| w.is_fullscreen())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_fullscreen(
    instance: &DotnetInstance,
    value: bool,
) {
    with_window(instance, (), |w| w.set_fullscreen(value))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_is_maximized(instance: &DotnetInstance) -> bool {
    with_window(instance, false, |w| w.is_maximized())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_maximized(instance: &DotnetInstance, value: bool) {
    with_window(instance, (), |w| w.set_maximized(value))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_is_minimized(instance: &DotnetInstance) -> bool {
    with_window(instance, false, |w| w.is_minimized())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_minimized(instance: &DotnetInstance, value: bool) {
    with_window(instance, (), |w| w.set_minimized(value))
}

/// A .NET `Func<CloseRequestResponse>`; dropping it releases `user_data` through `drop_user_data`.
struct ManagedCloseHandler {
    call: unsafe extern "C" fn(user_data: *mut c_void) -> i32,
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(user_data: *mut c_void),
}

impl ManagedCloseHandler {
    // A method, so that a closure that calls it captures the whole handler, not just its fields,
    // and drops it, which releases `user_data`, only with the closure.
    fn call(&self) -> CloseRequestResponse {
        match unsafe { (self.call)(self.user_data) } {
            1 => CloseRequestResponse::KeepWindowShown,
            _ => CloseRequestResponse::HideWindow,
        }
    }
}

impl Drop for ManagedCloseHandler {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) }
    }
}

/// `call` returns 1 to keep the window shown, 0 to hide it.
/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_on_close_requested(
    instance: &DotnetInstance,
    call: unsafe extern "C" fn(*mut c_void) -> i32,
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(*mut c_void),
) {
    let handler = ManagedCloseHandler { call, user_data, drop_user_data };
    with_window(instance, (), move |w| w.on_close_requested(move || handler.call()))
}

/// Sends the window a close request, like the window manager does when the user closes it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_dispatch_close_requested(instance: &DotnetInstance) {
    with_window(instance, (), |w| {
        w.dispatch_event(i_slint_core::platform::WindowEvent::CloseRequested)
    })
}
