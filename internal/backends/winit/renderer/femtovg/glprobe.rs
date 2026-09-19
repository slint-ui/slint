// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore clipchildren clipsiblings doublebuffer owndc
// cSpell: ignore pixelformatdescriptor wndclassw

use std::sync::OnceLock;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetDC, HDC, ReleaseDC};
use windows::Win32::Graphics::OpenGL::{
    ChoosePixelFormat, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL,
    PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR, SetPixelFormat, wglCreateContext, wglDeleteContext,
    wglGetCurrentContext, wglGetCurrentDC, wglGetProcAddress, wglMakeCurrent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_OWNDC, CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, UnregisterClassW,
    WINDOW_EX_STYLE, WNDCLASSW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_OVERLAPPED,
};
use windows::core::{s, w};

pub fn opengl_2_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    // A probe that couldn't run says nothing about the driver, and glutin may still
    // reach a working one through EGL.
    *AVAILABLE.get_or_init(|| unsafe { probe() }.unwrap_or(true))
}

/// Every Windows installation has an opengl32.dll, but in virtual machines it often provides
/// only OpenGL 1.1, without the shader entry points. Creating a context still succeeds there,
/// so probe with a throwaway window and context up-front.
///
/// Returns `None` if the probe couldn't run, which says nothing about the driver.
unsafe fn probe() -> Option<bool> {
    unsafe {
        let class_name = w!("SlintOpenGLProbe");
        let window_class = WNDCLASSW {
            style: CS_OWNDC,
            lpfnWndProc: Some(window_proc),
            lpszClassName: class_name,
            ..Default::default()
        };
        if RegisterClassW(&window_class) == 0 {
            return None;
        }

        // WGL requires a window that clips its children and siblings.
        let available = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!(""),
            WS_OVERLAPPED | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            None,
            None,
            None,
            None,
        )
        .ok()
        .and_then(|window| {
            let available = probe_window(window);
            let _ = DestroyWindow(window);
            available
        });

        let _ = UnregisterClassW(class_name, None);

        available
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

unsafe fn probe_window(window: HWND) -> Option<bool> {
    unsafe {
        let hdc = GetDC(Some(window));
        if hdc.is_invalid() {
            return None;
        }

        let available = probe_device_context(hdc);

        ReleaseDC(Some(window), hdc);

        available
    }
}

unsafe fn probe_device_context(hdc: HDC) -> Option<bool> {
    unsafe {
        let pixel_format_descriptor = PIXELFORMATDESCRIPTOR {
            nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            nVersion: 1,
            dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            iPixelType: PFD_TYPE_RGBA,
            cColorBits: 32,
            cDepthBits: 24,
            iLayerType: PFD_MAIN_PLANE.0 as u8,
            ..Default::default()
        };

        let pixel_format = ChoosePixelFormat(hdc, &pixel_format_descriptor);
        if pixel_format == 0 {
            return None;
        }
        SetPixelFormat(hdc, pixel_format, &pixel_format_descriptor).ok()?;

        let context = wglCreateContext(hdc).ok()?;

        // Restore whatever was current before, so that probing from a thread that already
        // renders through WGL is harmless.
        let previous_context = wglGetCurrentContext();
        let previous_hdc = wglGetCurrentDC();

        let available = wglMakeCurrent(hdc, context).is_ok().then(|| {
            let address = wglGetProcAddress(s!("glCreateShader")).map_or(0, |entry| entry as usize);
            let _ = wglMakeCurrent(previous_hdc, previous_context);
            // Besides null, some drivers report a missing entry point as 1, 2, 3 or -1.
            !matches!(address, 0 | 1 | 2 | 3 | usize::MAX)
        });

        let _ = wglDeleteContext(context);

        available
    }
}

#[test]
fn probe_runs() {
    // The answer depends on the machine, so this only covers the probe running to completion.
    opengl_2_available();
}
