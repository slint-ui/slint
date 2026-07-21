// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared physical key names used by `@physical-keys(...)`.
//!
//! Currently these names are based on the US keyboard key names.
//!
//! This is currently in the shape:
//!
//!     Slint # Winit # Xkb # Win # Mac
//!
//! `Xkb`/`Win`/`Mac` are the native key codes for:
//! Xkb: Linux X11/Wayland
//! Win: PS/2 set-1 scan codes for Windows with 0xe0 prefix for extended keys
//! Mac: The virtual_key code in the Qt backend
//!
//! See the comment in key_codes.rs for the usage of this macro.

#[macro_export]
macro_rules! for_each_physical_keys {
    ($macro:ident) => {
        $macro! {
            A # KeyA # 38 # 0x001e # 0x0000;
            B # KeyB # 56 # 0x0030 # 0x000b;
            C # KeyC # 54 # 0x002e # 0x0008;
            D # KeyD # 40 # 0x0020 # 0x0002;
            E # KeyE # 26 # 0x0012 # 0x000e;
            F # KeyF # 41 # 0x0021 # 0x0003;
            G # KeyG # 42 # 0x0022 # 0x0005;
            H # KeyH # 43 # 0x0023 # 0x0004;
            I # KeyI # 31 # 0x0017 # 0x0022;
            J # KeyJ # 44 # 0x0024 # 0x0026;
            K # KeyK # 45 # 0x0025 # 0x0028;
            L # KeyL # 46 # 0x0026 # 0x0025;
            M # KeyM # 58 # 0x0032 # 0x002e;
            N # KeyN # 57 # 0x0031 # 0x002d;
            O # KeyO # 32 # 0x0018 # 0x001f;
            P # KeyP # 33 # 0x0019 # 0x0023;
            Q # KeyQ # 24 # 0x0010 # 0x000c;
            R # KeyR # 27 # 0x0013 # 0x000f;
            S # KeyS # 39 # 0x001f # 0x0001;
            T # KeyT # 28 # 0x0014 # 0x0011;
            U # KeyU # 30 # 0x0016 # 0x0020;
            V # KeyV # 55 # 0x002f # 0x0009;
            W # KeyW # 25 # 0x0011 # 0x000d;
            X # KeyX # 53 # 0x002d # 0x0007;
            Y # KeyY # 29 # 0x0015 # 0x0010;
            Z # KeyZ # 52 # 0x002c # 0x0006;

            Digit0 # Digit0 # 19 # 0x000b # 0x001d;
            Digit1 # Digit1 # 10 # 0x0002 # 0x0012;
            Digit2 # Digit2 # 11 # 0x0003 # 0x0013;
            Digit3 # Digit3 # 12 # 0x0004 # 0x0014;
            Digit4 # Digit4 # 13 # 0x0005 # 0x0015;
            Digit5 # Digit5 # 14 # 0x0006 # 0x0017;
            Digit6 # Digit6 # 15 # 0x0007 # 0x0016;
            Digit7 # Digit7 # 16 # 0x0008 # 0x001a;
            Digit8 # Digit8 # 17 # 0x0009 # 0x001c;
            Digit9 # Digit9 # 18 # 0x000a # 0x0019;

            BackQuote    # Backquote    # 49 # 0x0029 # 0x0032;
            HyphenMinus  # Minus        # 20 # 0x000c # 0x001b;
            Equals       # Equal        # 21 # 0x000d # 0x0018;
            OpenBracket  # BracketLeft  # 34 # 0x001a # 0x0021;
            CloseBracket # BracketRight # 35 # 0x001b # 0x001e;
            BackSlash    # Backslash    # 51 # 0x002b # 0x002a;
            Semicolon    # Semicolon    # 47 # 0x0027 # 0x0029;
            Quote        # Quote        # 48 # 0x0028 # 0x0027;
            Comma        # Comma        # 59 # 0x0033 # 0x002b;
            Period       # Period       # 60 # 0x0034 # 0x002f;
            Slash        # Slash        # 61 # 0x0035 # 0x002c;
            Space        # Space        # 65 # 0x0039 # 0x0031;

            Escape     # Escape      # 9   # 0x0001 # 0x0035;
            Tab        # Tab         # 23  # 0x000f # 0x0030;
            Return     # Enter       # 36  # 0x001c # 0x0024;
            Backspace  # Backspace   # 22  # 0x000e # 0x0033;
            Delete     # Delete      # 119 # 0xe053 # 0x0075;
            Insert     # Insert      # 118 # 0xe052 # 0x0072;
            Home       # Home        # 110 # 0xe047 # 0x0073;
            End        # End         # 115 # 0xe04f # 0x0077;
            PageUp     # PageUp      # 112 # 0xe049 # 0x0074;
            PageDown   # PageDown    # 117 # 0xe051 # 0x0079;
            UpArrow    # ArrowUp     # 111 # 0xe048 # 0x007e;
            DownArrow  # ArrowDown   # 116 # 0xe050 # 0x007d;
            LeftArrow  # ArrowLeft   # 113 # 0xe04b # 0x007b;
            RightArrow # ArrowRight  # 114 # 0xe04d # 0x007c;
            Menu       # ContextMenu # 135 # 0xe05d # 0x006e;
            CapsLock   # CapsLock    # 66  # 0x003a # 0x0039;
            ScrollLock # ScrollLock  # 78  # 0x0046 # ;
            Pause      # Pause       # 127 # 0x0045 # ;

            F1  # F1  # 67  # 0x003b # 0x007a;
            F2  # F2  # 68  # 0x003c # 0x0078;
            F3  # F3  # 69  # 0x003d # 0x0063;
            F4  # F4  # 70  # 0x003e # 0x0076;
            F5  # F5  # 71  # 0x003f # 0x0060;
            F6  # F6  # 72  # 0x0040 # 0x0061;
            F7  # F7  # 73  # 0x0041 # 0x0062;
            F8  # F8  # 74  # 0x0042 # 0x0064;
            F9  # F9  # 75  # 0x0043 # 0x0065;
            F10 # F10 # 76  # 0x0044 # 0x006d;
            F11 # F11 # 95  # 0x0057 # 0x0067;
            F12 # F12 # 96  # 0x0058 # 0x006f;
            F13 # F13 # 191 # 0x0064 # 0x0069;
            F14 # F14 # 192 # 0x0065 # 0x006b;
            F15 # F15 # 193 # 0x0066 # 0x0071;
            F16 # F16 # 194 # 0x0067 # 0x006a;
            F17 # F17 # 195 # 0x0068 # 0x0040;
            F18 # F18 # 196 # 0x0069 # 0x004f;
            F19 # F19 # 197 # 0x006a # 0x0050;
            F20 # F20 # 198 # 0x006b # 0x005a;
            F21 # F21 # 199 # 0x006c # ;
            F22 # F22 # 200 # 0x006d # ;
            F23 # F23 # 201 # 0x006e # ;
            F24 # F24 # 202 # 0x0076 # ;
        }
    };
}

/// Maps an XKB keycode (the evdev keycode plus 8) to a Slint physical key name.
///
/// This is what X11, Wayland, and libinput-based backends report as the native key code.
#[cfg(target_os = "linux")]
pub fn physical_key_name_from_xkb(keycode: u32) -> Option<&'static str> {
    macro_rules! xkb_to_name {
        ($($name:ident # $_winit:ident # $xkb:literal # $_win:literal # $($_mac:literal)?;)*) => {
            match keycode {
                $($xkb => Some(stringify!($name)),)*
                _ => None,
            }
        };
    }
    for_each_physical_keys!(xkb_to_name)
}
