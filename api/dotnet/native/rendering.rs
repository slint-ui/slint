// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Backend selection and rendering notifiers, for drawing with the graphics API that Slint renders with.

use std::ffi::{CString, c_void};

use i_slint_backend_selector::api::BackendSelector;
use slint_interpreter::{ComponentHandle, GraphicsAPI, RenderingState, SetRenderingNotifierError};

use crate::instance::DotnetInstance;
use crate::{Utf8Str, guard};

/// Selects the backend and renderer, before any other use of Slint creates the default ones.
/// `api` is 0 for any graphics API, then 1 OpenGL, 2 OpenGL ES, 3 Metal, 4 Vulkan, 5 Direct3D;
/// a negative `major` leaves the OpenGL version open, and an empty name leaves the choice to Slint.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_backend_select(
    api: i32,
    major: i32,
    minor: i32,
    backend: Utf8Str,
    renderer: Utf8Str,
) -> bool {
    crate::guard_selection(|| {
        let (backend, renderer) = unsafe { (backend.as_str()?, renderer.as_str()?) };
        let version = u8::try_from(major).ok().zip(u8::try_from(minor).ok());
        let mut selector = BackendSelector::new();
        selector = match (api, version) {
            (0, _) => selector,
            (1, Some((major, minor))) => selector.require_opengl_with_version(major, minor),
            (1, None) => selector.require_opengl(),
            (2, Some((major, minor))) => selector.require_opengl_es_with_version(major, minor),
            (2, None) => selector.require_opengl_es(),
            (3, _) => selector.require_metal(),
            (4, _) => selector.require_vulkan(),
            (5, _) => selector.require_d3d(),
            _ => return Err(format!("Unknown graphics API {api}")),
        };
        if !backend.is_empty() {
            selector = selector.backend_name(backend.into());
        }
        if !renderer.is_empty() {
            selector = selector.renderer_name(renderer.into());
        }
        selector.select().map_err(|e| e.to_string())
    })
}

/// A .NET `Action<RenderingState, GraphicsApi>`; dropping it releases `user_data` through `drop_user_data`.
struct ManagedRenderingNotifier {
    call: unsafe extern "C" fn(user_data: *mut c_void, state: i32, api: *const c_void),
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(user_data: *mut c_void),
}

impl ManagedRenderingNotifier {
    // A method, so that the closure that calls it owns the whole notifier, see `ManagedCloseHandler::call`.
    fn call(&self, state: RenderingState, api: &GraphicsAPI) {
        let state = match state {
            RenderingState::RenderingSetup => 0,
            RenderingState::BeforeRendering => 1,
            RenderingState::AfterRendering => 2,
            RenderingState::RenderingTeardown => 3,
            _ => return,
        };
        unsafe { (self.call)(self.user_data, state, api as *const GraphicsAPI as *const c_void) }
    }
}

impl Drop for ManagedRenderingNotifier {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) }
    }
}

/// `call` gets the state as the index of `Slint.RenderingState`, and a `GraphicsAPI` that's valid during the call.
/// Returns 0 when set, 1 when the renderer doesn't support notifiers, 2 when one is set already, and -1 on failure.
/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_window_set_rendering_notifier(
    instance: &DotnetInstance,
    call: unsafe extern "C" fn(*mut c_void, i32, *const c_void),
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(*mut c_void),
) -> i32 {
    let notifier = ManagedRenderingNotifier { call, user_data, drop_user_data };
    guard(-1, || {
        Ok(instance.with(|i| {
            match i.window().set_rendering_notifier(move |state, api| notifier.call(state, api)) {
                Ok(()) => 0,
                Err(SetRenderingNotifierError::Unsupported) => 1,
                Err(_) => 2,
            }
        }))
    })
}

/// The kind of the graphics API of a rendering notifier: 1 OpenGL, 2 WebGL, and 0 another one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_graphics_api_kind(api: &GraphicsAPI) -> i32 {
    match api {
        GraphicsAPI::NativeOpenGL { .. } => 1,
        GraphicsAPI::WebGL { .. } => 2,
        #[allow(unreachable_patterns)]
        _ => 0,
    }
}

/// The address of the OpenGL function `name`, or null if there's none or the API isn't OpenGL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_graphics_api_get_proc_address(
    api: &GraphicsAPI,
    name: Utf8Str,
) -> *const c_void {
    guard(std::ptr::null(), || {
        let GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
            return Ok(std::ptr::null());
        };
        let name = CString::new(unsafe { name.as_str() }?).map_err(|e| e.to_string())?;
        Ok(get_proc_address(&name))
    })
}
