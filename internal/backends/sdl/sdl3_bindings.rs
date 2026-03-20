// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Minimal raw FFI bindings for SDL3 and SDL3_ttf.
//!
//! Only the subset of the SDL3 API required by this backend is declared here.
//! The build script links against the system-installed SDL3 and SDL3_ttf libraries.

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::os::raw::{c_char, c_float, c_int, c_void};

// ---------------------------------------------------------------------------
// SDL3 core types
// ---------------------------------------------------------------------------

pub type SDL_Window = c_void;
pub type SDL_Renderer = c_void;
pub type SDL_Texture = c_void;

pub type SDL_WindowID = u32;
pub type SDL_PropertiesID = u32;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct SDL_FRect {
    pub x: c_float,
    pub y: c_float,
    pub w: c_float,
    pub h: c_float,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct SDL_FPoint {
    pub x: c_float,
    pub y: c_float,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct SDL_Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SDL_Rect {
    pub x: c_int,
    pub y: c_int,
    pub w: c_int,
    pub h: c_int,
}

/// SDL_Surface (only the fields we need)
#[repr(C)]
pub struct SDL_Surface {
    pub flags: u32,
    pub format: u32,
    pub w: c_int,
    pub h: c_int,
    pub pitch: c_int,
    pub pixels: *mut c_void,
    pub refcount: c_int,
    pub reserved: *mut c_void,
}

// ---------------------------------------------------------------------------
// SDL3 init flags
// ---------------------------------------------------------------------------

pub const SDL_INIT_VIDEO: u64 = 0x0000_0020;
pub const SDL_INIT_EVENTS: u64 = 0x0000_4000;

// ---------------------------------------------------------------------------
// SDL3 window flags
// ---------------------------------------------------------------------------

pub const SDL_WINDOW_RESIZABLE: u64 = 0x0000_0020;
pub const SDL_WINDOW_HIGH_PIXEL_DENSITY: u64 = 0x0000_2000;

// ---------------------------------------------------------------------------
// SDL3 blend modes
// ---------------------------------------------------------------------------

pub const SDL_BLENDMODE_NONE: u32 = 0x0000_0000;
pub const SDL_BLENDMODE_BLEND: u32 = 0x0000_0001;

// ---------------------------------------------------------------------------
// SDL3 pixel formats (for SDL_CreateTexture)
// ---------------------------------------------------------------------------

pub const SDL_PIXELFORMAT_RGBA32: u32 = 0x1646_2004; // SDL_PIXELFORMAT_ABGR8888 on LE
pub const SDL_PIXELFORMAT_ARGB8888: u32 = 0x1636_2004;

// ---------------------------------------------------------------------------
// SDL3 texture access
// ---------------------------------------------------------------------------

pub const SDL_TEXTUREACCESS_STATIC: c_int = 0;
pub const SDL_TEXTUREACCESS_STREAMING: c_int = 1;
pub const SDL_TEXTUREACCESS_TARGET: c_int = 2;

// ---------------------------------------------------------------------------
// SDL3 ScaleMode
// ---------------------------------------------------------------------------

pub const SDL_SCALEMODE_NEAREST: c_int = 0;
pub const SDL_SCALEMODE_LINEAR: c_int = 1;

// ---------------------------------------------------------------------------
// SDL3 event types (SDL3 uses SDL_EVENT_* prefix)
// ---------------------------------------------------------------------------

pub const SDL_EVENT_QUIT: u32 = 0x100;
pub const SDL_EVENT_WINDOW_SHOWN: u32 = 0x202;
pub const SDL_EVENT_WINDOW_HIDDEN: u32 = 0x203;
pub const SDL_EVENT_WINDOW_EXPOSED: u32 = 0x204;
pub const SDL_EVENT_WINDOW_MOVED: u32 = 0x205;
pub const SDL_EVENT_WINDOW_RESIZED: u32 = 0x206;
pub const SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED: u32 = 0x207;
pub const SDL_EVENT_WINDOW_FOCUS_GAINED: u32 = 0x20e;
pub const SDL_EVENT_WINDOW_FOCUS_LOST: u32 = 0x20f;
pub const SDL_EVENT_WINDOW_CLOSE_REQUESTED: u32 = 0x210;
pub const SDL_EVENT_WINDOW_DISPLAY_CHANGED: u32 = 0x213;
pub const SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED: u32 = 0x214;

pub const SDL_EVENT_KEY_DOWN: u32 = 0x300;
pub const SDL_EVENT_KEY_UP: u32 = 0x301;
pub const SDL_EVENT_TEXT_INPUT: u32 = 0x302;

pub const SDL_EVENT_MOUSE_MOTION: u32 = 0x400;
pub const SDL_EVENT_MOUSE_BUTTON_DOWN: u32 = 0x401;
pub const SDL_EVENT_MOUSE_BUTTON_UP: u32 = 0x402;
pub const SDL_EVENT_MOUSE_WHEEL: u32 = 0x403;

pub const SDL_EVENT_USER: u32 = 0x8000;

// Mouse button constants
pub const SDL_BUTTON_LEFT: u8 = 1;
pub const SDL_BUTTON_MIDDLE: u8 = 2;
pub const SDL_BUTTON_RIGHT: u8 = 3;

// ---------------------------------------------------------------------------
// SDL3 event structures
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_CommonEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_WindowEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: SDL_WindowID,
    pub data1: i32,
    pub data2: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_KeyboardEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: SDL_WindowID,
    pub which: u32,
    pub scancode: u32,
    pub key: u32,
    pub r#mod: u16,
    pub raw: u16,
    pub down: bool,
    pub repeat: bool,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_TextInputEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: SDL_WindowID,
    pub text: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_MouseMotionEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: SDL_WindowID,
    pub which: u32,
    pub state: u32,
    pub x: c_float,
    pub y: c_float,
    pub xrel: c_float,
    pub yrel: c_float,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_MouseButtonEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: SDL_WindowID,
    pub which: u32,
    pub button: u8,
    pub down: bool,
    pub clicks: u8,
    pub padding: u8,
    pub x: c_float,
    pub y: c_float,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_MouseWheelEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: SDL_WindowID,
    pub which: u32,
    pub x: c_float,
    pub y: c_float,
    pub direction: u32,
    pub mouse_x: c_float,
    pub mouse_y: c_float,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_UserEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: SDL_WindowID,
    pub code: i32,
    pub data1: *mut c_void,
    pub data2: *mut c_void,
}

/// SDL_Event union — we keep this as a large byte array and provide typed accessors.
#[repr(C)]
#[derive(Copy, Clone)]
pub union SDL_Event {
    pub r#type: u32,
    pub common: SDL_CommonEvent,
    pub window: SDL_WindowEvent,
    pub key: SDL_KeyboardEvent,
    pub text: SDL_TextInputEvent,
    pub motion: SDL_MouseMotionEvent,
    pub button: SDL_MouseButtonEvent,
    pub wheel: SDL_MouseWheelEvent,
    pub user: SDL_UserEvent,
    // padding to cover the full union size (128 bytes in SDL3)
    _padding: [u8; 128],
}

impl Default for SDL_Event {
    fn default() -> Self {
        // Safety: zero-initialized event is valid
        unsafe { std::mem::zeroed() }
    }
}

// ---------------------------------------------------------------------------
// SDL3 keycode constants (subset)
// ---------------------------------------------------------------------------

pub const SDLK_RETURN: u32 = '\r' as u32;
pub const SDLK_ESCAPE: u32 = 0x1B;
pub const SDLK_BACKSPACE: u32 = '\x08' as u32;
pub const SDLK_TAB: u32 = '\t' as u32;
pub const SDLK_DELETE: u32 = 0x7F;
pub const SDLK_LEFT: u32 = 0x4000_0050;
pub const SDLK_RIGHT: u32 = 0x4000_004F;
pub const SDLK_UP: u32 = 0x4000_0052;
pub const SDLK_DOWN: u32 = 0x4000_0051;
pub const SDLK_HOME: u32 = 0x4000_004A;
pub const SDLK_END: u32 = 0x4000_004D;
pub const SDLK_PAGEUP: u32 = 0x4000_004B;
pub const SDLK_PAGEDOWN: u32 = 0x4000_004E;
pub const SDLK_F1: u32 = 0x4000_003A;
pub const SDLK_F2: u32 = 0x4000_003B;
pub const SDLK_F3: u32 = 0x4000_003C;
pub const SDLK_F4: u32 = 0x4000_003D;
pub const SDLK_F5: u32 = 0x4000_003E;
pub const SDLK_F6: u32 = 0x4000_003F;
pub const SDLK_F7: u32 = 0x4000_0040;
pub const SDLK_F8: u32 = 0x4000_0041;
pub const SDLK_F9: u32 = 0x4000_0042;
pub const SDLK_F10: u32 = 0x4000_0043;
pub const SDLK_F11: u32 = 0x4000_0044;
pub const SDLK_F12: u32 = 0x4000_0045;

pub const SDL_KMOD_SHIFT: u16 = 0x0003;
pub const SDL_KMOD_CTRL: u16 = 0x00C0;
pub const SDL_KMOD_ALT: u16 = 0x0300;

// ---------------------------------------------------------------------------
// SDL3 core functions
// ---------------------------------------------------------------------------

unsafe extern "C" {
    pub fn SDL_Init(flags: u64) -> bool;
    pub fn SDL_Quit();
    pub fn SDL_GetError() -> *const c_char;

    pub fn SDL_CreateWindow(
        title: *const c_char,
        w: c_int,
        h: c_int,
        flags: u64,
    ) -> *mut SDL_Window;
    pub fn SDL_DestroyWindow(window: *mut SDL_Window);
    pub fn SDL_SetWindowTitle(window: *mut SDL_Window, title: *const c_char) -> bool;
    pub fn SDL_GetWindowSize(window: *mut SDL_Window, w: *mut c_int, h: *mut c_int) -> bool;
    pub fn SDL_GetWindowSizeInPixels(
        window: *mut SDL_Window,
        w: *mut c_int,
        h: *mut c_int,
    ) -> bool;
    pub fn SDL_SetWindowSize(window: *mut SDL_Window, w: c_int, h: c_int) -> bool;
    pub fn SDL_GetWindowPosition(
        window: *mut SDL_Window,
        x: *mut c_int,
        y: *mut c_int,
    ) -> bool;
    pub fn SDL_SetWindowPosition(window: *mut SDL_Window, x: c_int, y: c_int) -> bool;
    pub fn SDL_ShowWindow(window: *mut SDL_Window) -> bool;
    pub fn SDL_HideWindow(window: *mut SDL_Window) -> bool;
    pub fn SDL_GetWindowID(window: *mut SDL_Window) -> SDL_WindowID;
    pub fn SDL_GetWindowDisplayScale(window: *mut SDL_Window) -> c_float;

    pub fn SDL_CreateRenderer(
        window: *mut SDL_Window,
        name: *const c_char,
    ) -> *mut SDL_Renderer;
    pub fn SDL_DestroyRenderer(renderer: *mut SDL_Renderer);
    pub fn SDL_SetRenderDrawColor(
        renderer: *mut SDL_Renderer,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> bool;
    pub fn SDL_SetRenderDrawBlendMode(renderer: *mut SDL_Renderer, blend_mode: u32) -> bool;
    pub fn SDL_RenderClear(renderer: *mut SDL_Renderer) -> bool;
    pub fn SDL_RenderPresent(renderer: *mut SDL_Renderer) -> bool;
    pub fn SDL_RenderFillRect(renderer: *mut SDL_Renderer, rect: *const SDL_FRect) -> bool;
    pub fn SDL_RenderRect(renderer: *mut SDL_Renderer, rect: *const SDL_FRect) -> bool;
    pub fn SDL_SetRenderClipRect(renderer: *mut SDL_Renderer, rect: *const SDL_Rect) -> bool;
    pub fn SDL_RenderTexture(
        renderer: *mut SDL_Renderer,
        texture: *mut SDL_Texture,
        srcrect: *const SDL_FRect,
        dstrect: *const SDL_FRect,
    ) -> bool;
    pub fn SDL_RenderTextureRotated(
        renderer: *mut SDL_Renderer,
        texture: *mut SDL_Texture,
        srcrect: *const SDL_FRect,
        dstrect: *const SDL_FRect,
        angle: f64,
        center: *const SDL_FPoint,
        flip: c_int,
    ) -> bool;
    pub fn SDL_SetRenderTarget(
        renderer: *mut SDL_Renderer,
        texture: *mut SDL_Texture,
    ) -> bool;

    pub fn SDL_CreateTexture(
        renderer: *mut SDL_Renderer,
        format: u32,
        access: c_int,
        w: c_int,
        h: c_int,
    ) -> *mut SDL_Texture;
    pub fn SDL_CreateTextureFromSurface(
        renderer: *mut SDL_Renderer,
        surface: *mut SDL_Surface,
    ) -> *mut SDL_Texture;
    pub fn SDL_DestroyTexture(texture: *mut SDL_Texture);
    pub fn SDL_SetTextureAlphaMod(texture: *mut SDL_Texture, alpha: u8) -> bool;
    pub fn SDL_SetTextureColorMod(texture: *mut SDL_Texture, r: u8, g: u8, b: u8) -> bool;
    pub fn SDL_SetTextureBlendMode(texture: *mut SDL_Texture, blend_mode: u32) -> bool;
    pub fn SDL_UpdateTexture(
        texture: *mut SDL_Texture,
        rect: *const SDL_Rect,
        pixels: *const c_void,
        pitch: c_int,
    ) -> bool;
    pub fn SDL_SetTextureScaleMode(texture: *mut SDL_Texture, scale_mode: c_int) -> bool;

    pub fn SDL_DestroySurface(surface: *mut SDL_Surface);

    // Events
    pub fn SDL_PollEvent(event: *mut SDL_Event) -> bool;
    pub fn SDL_WaitEventTimeout(event: *mut SDL_Event, timeout_ms: i32) -> bool;
    pub fn SDL_PushEvent(event: *mut SDL_Event) -> bool;

    // Clipboard
    pub fn SDL_SetClipboardText(text: *const c_char) -> bool;
    pub fn SDL_GetClipboardText() -> *mut c_char;
    pub fn SDL_free(mem: *mut c_void);

    // Timer
    pub fn SDL_GetTicks() -> u64;

    // Hints
    pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> bool;
}

// ---------------------------------------------------------------------------
// SDL3_ttf types and functions
// ---------------------------------------------------------------------------

pub type TTF_Font = c_void;
pub type TTF_TextEngine = c_void;
pub type TTF_Text = c_void;

unsafe extern "C" {
    pub fn TTF_Init() -> bool;
    pub fn TTF_Quit();

    pub fn TTF_OpenFont(file: *const c_char, ptsize: c_float) -> *mut TTF_Font;
    pub fn TTF_OpenFontIO(
        src: *mut c_void,
        closeio: bool,
        ptsize: c_float,
    ) -> *mut TTF_Font;
    pub fn TTF_CloseFont(font: *mut TTF_Font);
    pub fn TTF_SetFontSize(font: *mut TTF_Font, ptsize: c_float) -> bool;

    pub fn TTF_GetFontAscent(font: *mut TTF_Font) -> c_int;
    pub fn TTF_GetFontDescent(font: *mut TTF_Font) -> c_int;
    pub fn TTF_GetFontHeight(font: *mut TTF_Font) -> c_int;
    pub fn TTF_GetFontLineSkip(font: *mut TTF_Font) -> c_int;

    pub fn TTF_SetFontStyle(font: *mut TTF_Font, style: c_int);
    pub fn TTF_GetFontStyle(font: *mut TTF_Font) -> c_int;

    /// Render text to an SDL_Surface using blended (anti-aliased) rendering.
    pub fn TTF_RenderText_Blended(
        font: *mut TTF_Font,
        text: *const c_char,
        length: usize,
        fg: SDL_Color,
    ) -> *mut SDL_Surface;

    /// Render wrapped text to an SDL_Surface.
    pub fn TTF_RenderText_Blended_Wrapped(
        font: *mut TTF_Font,
        text: *const c_char,
        length: usize,
        fg: SDL_Color,
        wrap_length: c_int,
    ) -> *mut SDL_Surface;

    /// Get the size of a string when rendered with the given font.
    pub fn TTF_GetStringSize(
        font: *mut TTF_Font,
        text: *const c_char,
        length: usize,
        w: *mut c_int,
        h: *mut c_int,
    ) -> bool;

    /// Measure how much of a string will fit in a given width.
    pub fn TTF_MeasureString(
        font: *mut TTF_Font,
        text: *const c_char,
        length: usize,
        max_width: c_int,
        extent: *mut c_int,
        count: *mut usize,
    ) -> bool;

    // --- New SDL_Renderer-based text engine API (SDL_ttf 3.x) ---

    /// Create a text engine for use with an SDL_Renderer.
    pub fn TTF_CreateRendererTextEngine(renderer: *mut SDL_Renderer) -> *mut TTF_TextEngine;
    /// Destroy a text engine.
    pub fn TTF_DestroyRendererTextEngine(engine: *mut TTF_TextEngine);
    /// Create a text object for rendering.
    pub fn TTF_CreateText(
        engine: *mut TTF_TextEngine,
        font: *mut TTF_Font,
        text: *const c_char,
        length: usize,
    ) -> *mut TTF_Text;
    /// Destroy a text object.
    pub fn TTF_DestroyText(text: *mut TTF_Text);
    /// Get the size of a text object.
    pub fn TTF_GetTextSize(text: *mut TTF_Text, w: *mut c_int, h: *mut c_int) -> bool;
    /// Set the color of a text object.
    pub fn TTF_SetTextColor(text: *mut TTF_Text, r: u8, g: u8, b: u8, a: u8) -> bool;
    /// Draw a text object to the renderer.
    pub fn TTF_DrawRendererText(text: *mut TTF_Text, x: c_float, y: c_float) -> bool;
    /// Set wrapping on a text object.
    pub fn TTF_SetTextWrapWidth(text: *mut TTF_Text, wrap_width: c_int) -> bool;

    // SDL_IOStream for loading fonts from memory
    pub fn SDL_IOFromConstMem(mem: *const c_void, size: usize) -> *mut c_void;
}

// TTF font style flags
pub const TTF_STYLE_NORMAL: c_int = 0x00;
pub const TTF_STYLE_BOLD: c_int = 0x01;
pub const TTF_STYLE_ITALIC: c_int = 0x02;

// ---------------------------------------------------------------------------
// Helper to get SDL error as a Rust string
// ---------------------------------------------------------------------------

pub fn sdl_error() -> String {
    unsafe {
        let ptr = SDL_GetError();
        if ptr.is_null() {
            "Unknown SDL error".to_string()
        } else {
            std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}
