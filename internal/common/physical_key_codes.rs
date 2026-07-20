// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared physical key names used by `@physical-keys(...)`.
//!
//! Currently these names are based on the US keyboard key names.
//!
//! This is currently in the shape:
//!
//!     Slint # Winit # Xkb
//!
//! The `Xkb` column is the XKB keycode. Qt reports it as the native scan code on
//! X11 and Wayland, so the Qt backend uses it to derive the physical key.
//!
//! See the comment in key_codes.rs for the usage of this macro.

#[macro_export]
macro_rules! for_each_physical_keys {
    ($macro:ident) => {
        $macro![
            A # KeyA # 38;
            B # KeyB # 56;
            C # KeyC # 54;
            D # KeyD # 40;
            E # KeyE # 26;
            F # KeyF # 41;
            G # KeyG # 42;
            H # KeyH # 43;
            I # KeyI # 31;
            J # KeyJ # 44;
            K # KeyK # 45;
            L # KeyL # 46;
            M # KeyM # 58;
            N # KeyN # 57;
            O # KeyO # 32;
            P # KeyP # 33;
            Q # KeyQ # 24;
            R # KeyR # 27;
            S # KeyS # 39;
            T # KeyT # 28;
            U # KeyU # 30;
            V # KeyV # 55;
            W # KeyW # 25;
            X # KeyX # 53;
            Y # KeyY # 29;
            Z # KeyZ # 52;

            Digit0 # Digit0 # 19;
            Digit1 # Digit1 # 10;
            Digit2 # Digit2 # 11;
            Digit3 # Digit3 # 12;
            Digit4 # Digit4 # 13;
            Digit5 # Digit5 # 14;
            Digit6 # Digit6 # 15;
            Digit7 # Digit7 # 16;
            Digit8 # Digit8 # 17;
            Digit9 # Digit9 # 18;

            BackQuote # Backquote # 49;
            HyphenMinus # Minus # 20;
            Equals # Equal # 21;
            OpenBracket # BracketLeft # 34;
            CloseBracket # BracketRight # 35;
            BackSlash # Backslash # 51;
            Semicolon # Semicolon # 47;
            Quote # Quote # 48;
            Comma # Comma # 59;
            Period # Period # 60;
            Slash # Slash # 61;
            Space # Space # 65;

            Escape # Escape # 9;
            Tab # Tab # 23;
            Return # Enter # 36;
            Backspace # Backspace # 22;
            Delete # Delete # 119;
            Insert # Insert # 118;
            Home # Home # 110;
            End # End # 115;
            PageUp # PageUp # 112;
            PageDown # PageDown # 117;
            UpArrow # ArrowUp # 111;
            DownArrow # ArrowDown # 116;
            LeftArrow # ArrowLeft # 113;
            RightArrow # ArrowRight # 114;
            Menu # ContextMenu # 135;
            CapsLock # CapsLock # 66;
            ScrollLock # ScrollLock # 78;
            Pause # Pause # 127;

            F1 # F1 # 67;
            F2 # F2 # 68;
            F3 # F3 # 69;
            F4 # F4 # 70;
            F5 # F5 # 71;
            F6 # F6 # 72;
            F7 # F7 # 73;
            F8 # F8 # 74;
            F9 # F9 # 75;
            F10 # F10 # 76;
            F11 # F11 # 95;
            F12 # F12 # 96;
            F13 # F13 # 191;
            F14 # F14 # 192;
            F15 # F15 # 193;
            F16 # F16 # 194;
            F17 # F17 # 195;
            F18 # F18 # 196;
            F19 # F19 # 197;
            F20 # F20 # 198;
            F21 # F21 # 199;
            F22 # F22 # 200;
            F23 # F23 # 201;
            F24 # F24 # 202;
        ];
    };
}
