// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Re-exports from `sdl3-sys` and `sdl3-ttf-sys` used by this backend, plus
//! a small helper function.
//!
//! All SDL3 and SDL_ttf types, constants, and functions are provided by the
//! upstream `-sys` crates. This module gathers the subset we use into a
//! single import for convenience.

// ---------------------------------------------------------------------------
// SDL3 core
// ---------------------------------------------------------------------------

pub use sdl3_sys::blendmode::SDL_BLENDMODE_BLEND;

pub use sdl3_sys::clipboard::{SDL_GetClipboardText, SDL_SetClipboardText};

pub use sdl3_sys::error::SDL_GetError;

pub use sdl3_sys::events::{
    SDL_Event, SDL_UserEvent,
    // Event type constants
    SDL_EVENT_KEY_DOWN, SDL_EVENT_KEY_UP, SDL_EVENT_MOUSE_BUTTON_DOWN, SDL_EVENT_MOUSE_BUTTON_UP,
    SDL_EVENT_MOUSE_MOTION, SDL_EVENT_MOUSE_WHEEL, SDL_EVENT_QUIT, SDL_EVENT_TEXT_INPUT,
    SDL_EVENT_WINDOW_CLOSE_REQUESTED, SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED,
    SDL_EVENT_WINDOW_EXPOSED, SDL_EVENT_WINDOW_RESIZED,
    SDL_EVENT_USER,
    // Functions
    SDL_PollEvent, SDL_PushEvent, SDL_WaitEventTimeout,
};

pub use sdl3_sys::init::{SDL_Init, SDL_Quit, SDL_INIT_EVENTS, SDL_INIT_VIDEO};

pub use sdl3_sys::iostream::SDL_IOFromConstMem;

pub use sdl3_sys::keycode::{
    SDL_Keycode, SDLK_BACKSPACE, SDLK_DELETE, SDLK_DOWN, SDLK_END, SDLK_ESCAPE, SDLK_F1,
    SDLK_F10, SDLK_F11, SDLK_F12, SDLK_F2, SDLK_F3, SDLK_F4, SDLK_F5, SDLK_F6, SDLK_F7,
    SDLK_F8, SDLK_F9, SDLK_HOME, SDLK_LEFT, SDLK_PAGEDOWN, SDLK_PAGEUP, SDLK_RETURN,
    SDLK_RIGHT, SDLK_TAB, SDLK_UP,
};

pub use sdl3_sys::mouse::{SDL_BUTTON_LEFT, SDL_BUTTON_MIDDLE, SDL_BUTTON_RIGHT};

pub use sdl3_sys::pixels::SDL_PIXELFORMAT_RGBA32;

pub use sdl3_sys::rect::{SDL_FRect, SDL_Rect};

pub use sdl3_sys::render::{
    SDL_CreateRenderer, SDL_CreateTexture, SDL_DestroyRenderer,
    SDL_DestroyTexture, SDL_RenderClear, SDL_RenderFillRect, SDL_RenderPresent,
    SDL_RenderTexture, SDL_Renderer, SDL_SetRenderClipRect, SDL_SetRenderDrawBlendMode,
    SDL_SetRenderDrawColor, SDL_SetTextureAlphaMod,
    SDL_SetTextureBlendMode, SDL_SetTextureColorMod, SDL_Texture, SDL_UpdateTexture,
    SDL_TEXTUREACCESS_STATIC,
};

pub use sdl3_sys::stdinc::SDL_free;


pub use sdl3_sys::video::{
    SDL_CreateWindow, SDL_DestroyWindow, SDL_GetWindowDisplayScale,
    SDL_GetWindowPosition, SDL_GetWindowSize, SDL_GetWindowSizeInPixels, SDL_HideWindow,
    SDL_SetWindowPosition, SDL_SetWindowSize, SDL_SetWindowTitle, SDL_ShowWindow, SDL_Window, SDL_WINDOW_HIGH_PIXEL_DENSITY, SDL_WINDOW_RESIZABLE,
};

// ---------------------------------------------------------------------------
// SDL3_ttf
// ---------------------------------------------------------------------------

pub use sdl3_ttf_sys::ttf::{
    TTF_CloseFont, TTF_CreateRendererTextEngine, TTF_CreateText,
    TTF_DestroyRendererTextEngine, TTF_DestroyText, TTF_DrawRendererText, TTF_Font,
    TTF_GetFontAscent, TTF_GetFontDescent, TTF_GetFontHeight,
    TTF_GetFontLineSkip, TTF_GetStringSize, TTF_Init, TTF_MeasureString,
    TTF_OpenFont, TTF_OpenFontIO, TTF_Quit, TTF_SetFontStyle, TTF_SetTextColor, TTF_SetTextWrapWidth, TTF_STYLE_BOLD, TTF_STYLE_ITALIC, TTF_STYLE_NORMAL,
};

// ---------------------------------------------------------------------------
// Helper
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
