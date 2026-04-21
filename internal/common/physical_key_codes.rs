// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared physical key names used by `@physical-keys(...)`.

#[macro_export]
macro_rules! for_each_physical_keys {
    ($macro:ident) => {
        $macro![
            A # KeyA;
            B # KeyB;
            C # KeyC;
            D # KeyD;
            E # KeyE;
            F # KeyF;
            G # KeyG;
            H # KeyH;
            I # KeyI;
            J # KeyJ;
            K # KeyK;
            L # KeyL;
            M # KeyM;
            N # KeyN;
            O # KeyO;
            P # KeyP;
            Q # KeyQ;
            R # KeyR;
            S # KeyS;
            T # KeyT;
            U # KeyU;
            V # KeyV;
            W # KeyW;
            X # KeyX;
            Y # KeyY;
            Z # KeyZ;

            Digit0 # Digit0;
            Digit1 # Digit1;
            Digit2 # Digit2;
            Digit3 # Digit3;
            Digit4 # Digit4;
            Digit5 # Digit5;
            Digit6 # Digit6;
            Digit7 # Digit7;
            Digit8 # Digit8;
            Digit9 # Digit9;

            BackQuote # Backquote;
            HyphenMinus # Minus;
            Equals # Equal;
            OpenBracket # BracketLeft;
            CloseBracket # BracketRight;
            BackSlash # Backslash;
            Semicolon # Semicolon;
            Quote # Quote;
            Comma # Comma;
            Period # Period;
            Slash # Slash;
            Space # Space;

            Escape # Escape;
            Tab # Tab;
            Return # Enter;
            Backspace # Backspace;
            Delete # Delete;
            Insert # Insert;
            Home # Home;
            End # End;
            PageUp # PageUp;
            PageDown # PageDown;
            UpArrow # ArrowUp;
            DownArrow # ArrowDown;
            LeftArrow # ArrowLeft;
            RightArrow # ArrowRight;
            Menu # ContextMenu;
            CapsLock # CapsLock;
            ScrollLock # ScrollLock;
            Pause # Pause;

            F1 # F1;
            F2 # F2;
            F3 # F3;
            F4 # F4;
            F5 # F5;
            F6 # F6;
            F7 # F7;
            F8 # F8;
            F9 # F9;
            F10 # F10;
            F11 # F11;
            F12 # F12;
            F13 # F13;
            F14 # F14;
            F15 # F15;
            F16 # F16;
            F17 # F17;
            F18 # F18;
            F19 # F19;
            F20 # F20;
            F21 # F21;
            F22 # F22;
            F23 # F23;
            F24 # F24;
        ];
    };
}
