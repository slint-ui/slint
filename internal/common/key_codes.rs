// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module is meant to be included by different crate and each crate must define the macro for_each_keys
//!
//! The key code comes from <https://www.unicode.org/Public/MAPPINGS/VENDORS/APPLE/CORPCHAR.TXT>
//! the names comes should match with <https://www.w3.org/TR/uievents-key/#named-key-attribute-values>,
//!
//! The format is a semicolon separated list of keys
//! `<char code>  # Slint name # Qt code # Winit code # xkb code`
//!
//! ## Example
//!
//! ```
//! macro_rules! do_something_with_keys {
//!     ($($char:literal # $name:ident # $($qt:ident)|* # $($winit:ident $(($_pos:ident))?)|* # $($xkb:ident)|* ;)*) => {
//!         //...
//!     };
//! }
//! i_slint_common::for_each_special_keys!(do_something_with_keys);
//! ```
//!
// NOTE: Update namespaces.md when changing/adding/removing keys, to keep the docs in sync!
#[macro_export]
macro_rules! for_each_special_keys {
    ($macro:ident) => {
        $macro![
'\u{0008}'  # Backspace   # Qt_Key_Key_Backspace    # Backspace    # BackSpace  ;
'\u{0009}'  # Tab         # Qt_Key_Key_Tab          # Tab          # Tab        ;
'\u{000a}'  # Return      # Qt_Key_Key_Enter|Qt_Key_Key_Return # Enter # Return;
'\u{001b}'  # Escape      # Qt_Key_Key_Escape       # Escape       # Escape     ;
'\u{0019}'  # Backtab     # Qt_Key_Key_Backtab      #              # BackTab    ;
'\u{007f}'  # Delete      # Qt_Key_Key_Delete       # Delete       # Delete     ;

// The modifier key codes comes from https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/keyCode.
'\u{0010}'  # Shift       # Qt_Key_Key_Shift        # Shift(Left)  # Shift_L    ;
'\u{0011}'  # Control     # Qt_Key_Key_Control      # Control(Left)# Control_L  ;
'\u{0012}'  # Alt         # Qt_Key_Key_Alt          # Alt          # Alt_L      ;
'\u{0013}'  # AltGr       # Qt_Key_Key_AltGr        # AltGraph     # Mode_switch;
'\u{0014}'  # CapsLock    # Qt_Key_Key_CapsLock     #              # Caps_Lock  ;

'\u{0015}'  # ShiftR      #                         # Shift(Right) # Shift_R    ;
'\u{0016}'  # ControlR    #                         # Control(Right)# Control_R  ;

// Use custom codes instead of DOM_VK_META for meta, because the Mozilla defined code is a regular character (E0; LATIN SMALL LETTER A WITH GRAVE)
// which makes those keys appear as text.
'\u{0017}'  # Meta        # Qt_Key_Key_Meta         # Super(Left)  # Meta_L     ;
'\u{0018}'  # MetaR       #                         # Super(Right) # Meta_R     ;

'\u{0020}'  # Space       # Qt_Key_Key_Space        # Space # space     ;

'\u{F700}'	# UpArrow     # Qt_Key_Key_Up           # ArrowUp           # Up         ;
'\u{F701}'	# DownArrow   # Qt_Key_Key_Down         # ArrowDown         # Down       ;
'\u{F702}'	# LeftArrow   # Qt_Key_Key_Left         # ArrowLeft         # Left       ;
'\u{F703}'	# RightArrow  # Qt_Key_Key_Right        # ArrowRight        # Right      ;
'\u{F704}'	# F1          # Qt_Key_Key_F1           # F1           # F1         ;
'\u{F705}'	# F2          # Qt_Key_Key_F2           # F2           # F2         ;
'\u{F706}'	# F3          # Qt_Key_Key_F3           # F3           # F3         ;
'\u{F707}'	# F4          # Qt_Key_Key_F4           # F4           # F4         ;
'\u{F708}'	# F5          # Qt_Key_Key_F5           # F5           # F5         ;
'\u{F709}'	# F6          # Qt_Key_Key_F6           # F6           # F6         ;
'\u{F70A}'	# F7          # Qt_Key_Key_F7           # F7           # F7         ;
'\u{F70B}'	# F8          # Qt_Key_Key_F8           # F8           # F8         ;
'\u{F70C}'	# F9          # Qt_Key_Key_F9           # F9           # F9         ;
'\u{F70D}'	# F10         # Qt_Key_Key_F10          # F10          # F10        ;
'\u{F70E}'	# F11         # Qt_Key_Key_F11          # F11          # F11        ;
'\u{F70F}'	# F12         # Qt_Key_Key_F12          # F12          # F12        ;
'\u{F710}'	# F13         # Qt_Key_Key_F13          # F13          # F13        ;
'\u{F711}'	# F14         # Qt_Key_Key_F14          # F14          # F14        ;
'\u{F712}'	# F15         # Qt_Key_Key_F15          # F15          # F15        ;
'\u{F713}'	# F16         # Qt_Key_Key_F16          # F16          # F16        ;
'\u{F714}'	# F17         # Qt_Key_Key_F17          # F17          # F17        ;
'\u{F715}'	# F18         # Qt_Key_Key_F18          # F18          # F18        ;
'\u{F716}'	# F19         # Qt_Key_Key_F19          # F19          # F19        ;
'\u{F717}'	# F20         # Qt_Key_Key_F20          # F20          # F20        ;
'\u{F718}'	# F21         # Qt_Key_Key_F21          # F21          # F21        ;
'\u{F719}'	# F22         # Qt_Key_Key_F22          # F22          # F22        ;
'\u{F71A}'	# F23         # Qt_Key_Key_F23          # F23          # F23        ;
'\u{F71B}'	# F24         # Qt_Key_Key_F24          # F24          # F24        ;
//'\u{F71C}'# F25         # Qt_Key_Key_F25          # F25          # F25        ;
//'\u{F71D}'# F26         # Qt_Key_Key_F26          # F26          # F26        ;
//'\u{F71E}'# F27         # Qt_Key_Key_F27          # F27          # F27        ;
//'\u{F71F}'# F28         # Qt_Key_Key_F28          # F28          # F28        ;
//'\u{F720}'# F29         # Qt_Key_Key_F29          # F29          # F29        ;
//'\u{F721}'# F30         # Qt_Key_Key_F30          # F30          # F30        ;
//'\u{F722}'# F31         # Qt_Key_Key_F31          # F31          # F31        ;
//'\u{F723}'# F32         # Qt_Key_Key_F32          # F32          # F32        ;
//'\u{F724}'# F33         # Qt_Key_Key_F33          # F33          # F33        ;
//'\u{F725}'# F34         # Qt_Key_Key_F34          # F34          # F34        ;
//'\u{F726}'# F35         # Qt_Key_Key_F35          # F35          # F35        ;
'\u{F727}'	# Insert      # Qt_Key_Key_Insert       # Insert       # Insert     ;
//'\u{F728}'	# Delete     ;  // already as a control code
'\u{F729}'	# Home        # Qt_Key_Key_Home         # Home         # Home       ;
//'\u{F72A}'	# Begin       #                         #              ;
'\u{F72B}'	# End         # Qt_Key_Key_End          # End          # End        ;
'\u{F72C}'	# PageUp      # Qt_Key_Key_PageUp       # PageUp       # Page_Up    ;
'\u{F72D}'	# PageDown    # Qt_Key_Key_PageDown     # PageDown     # Page_Down  ;
//'\u{F72E}'	# PrintScreen #                         # Snapshot     ;
'\u{F72F}'	# ScrollLock  # Qt_Key_Key_ScrollLock   # ScrollLock   # Scroll_Lock;
'\u{F730}'	# Pause       # Qt_Key_Key_Pause        # Pause        # Pause      ;
'\u{F731}'	# SysReq      # Qt_Key_Key_SysReq       # PrintScreen  # Sys_Req    ;
//'\u{F732}'	# Break       #                         #              ;
//'\u{F733}'	# Reset       #                         #              ;
'\u{F734}'	# Stop        # Qt_Key_Key_Stop         #              # XF86_Stop       ;
'\u{F735}'	# Menu        # Qt_Key_Key_Menu         # ContextMenu  # Menu       ;
//'\u{F736}'	# User        #                         #              ;
//'\u{F737}'	# System      #                         #              ;
//'\u{F738}'	# Print       # Qt_Key_Key_Print        #              ;
//'\u{F739}'	# ClearLine   #                         #              ;
//'\u{F73A}'	# ClearDisplay#                         #              ;
//'\u{F73B}'	# InsertLine  #                         #              ;
//'\u{F73C}'	# DeleteLine  #                         #              ;
//'\u{F73D}'	# InsertChar  #                         #              ;
//'\u{F73E}'	# DeleteChar  #                         #              ;
//'\u{F73F}'	# Prev        #                         #              ;
//'\u{F740}'	# Next        #                         #              ;
//'\u{F741}'	# Select      # Qt_Key_Key_Select       #              ;
//'\u{F742}'	# Execute     # Qt_Key_Key_Execute      #              ;
//'\u{F743}'	# Undo        # Qt_Key_Key_Undo         #              ;
//'\u{F744}'	# Redo        # Qt_Key_Key_Redo         #              ;
//'\u{F745}'	# Find        # Qt_Key_Key_Find         #              ;
//'\u{F746}'	# Help        # Qt_Key_Key_Help         #              ;
//'\u{F747}'	# ModeSwitch  # Qt_Key_Key_Mode_switch  #            ;
];
    };
}
