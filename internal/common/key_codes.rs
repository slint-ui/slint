// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

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
//!     ($($char:literal # $name:ident # $($qt:ident)|* # $($winit:ident)|* # $($xkb:ident)|* ;)*) => {
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
'\u{0008}'  # Backspace   # Qt_Key_Key_Backspace    # Back         # KEY_BackSpace  ;
'\u{0009}'  # Tab         # Qt_Key_Key_Tab          # Tab          # KEY_Tab        ;
'\u{000a}'  # Return      # Qt_Key_Key_Enter|Qt_Key_Key_Return # NumpadEnter|Return # KEY_Return;
'\u{001b}'  # Escape      # Qt_Key_Key_Escape       # Escape       # KEY_Escape     ;
'\u{0019}'  # Backtab     # Qt_Key_Key_Backtab      #              # KEY_BackTab    ;
'\u{007f}'  # Delete      # Qt_Key_Key_Delete       # Delete       # KEY_Delete     ;

// The modifier key codes comes from https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/keyCode.
'\u{0010}'  # Shift       # Qt_Key_Key_Shift        # LShift       # KEY_Shift_L    ;
'\u{0011}'  # Control     # Qt_Key_Key_Control      # LControl     # KEY_Control_L  ;
'\u{0012}'  # Alt         # Qt_Key_Key_Alt          # LAlt         # KEY_Alt_L      ;
'\u{0013}'  # AltGr       # Qt_Key_Key_AltGr        # RAlt         # KEY_Mode_switch;
'\u{0014}'  # CapsLock    # Qt_Key_Key_CapsLock     #              # KEY_Caps_Lock  ;

'\u{0015}'  # ShiftR      #                         # RShift       # KEY_Shift_R    ;
'\u{0016}'  # ControlR    #                         # RControl     # KEY_Control_R  ;

// Use custom codes instead of DOM_VK_META for meta, because the Mozilla defined code is a regular character (E0; LATIN SMALL LETTER A WITH GRAVE)
// which makes those keys appear as text.
'\u{0017}'  # Meta        # Qt_Key_Key_Meta         # LWin         # KEY_Meta_L     ;
'\u{0018}'  # MetaR       #                         # RWin         # KEY_Meta_R     ;

'\u{F700}'	# UpArrow     # Qt_Key_Key_Up           # Up           # KEY_Up         ;
'\u{F701}'	# DownArrow   # Qt_Key_Key_Down         # Down         # KEY_Down       ;
'\u{F702}'	# LeftArrow   # Qt_Key_Key_Left         # Left         # KEY_Left       ;
'\u{F703}'	# RightArrow  # Qt_Key_Key_Right        # Right        # KEY_Right      ;
'\u{F704}'	# F1          # Qt_Key_Key_F1           # F1           # KEY_F1         ;
'\u{F705}'	# F2          # Qt_Key_Key_F2           # F2           # KEY_F2         ;
'\u{F706}'	# F3          # Qt_Key_Key_F3           # F3           # KEY_F3         ;
'\u{F707}'	# F4          # Qt_Key_Key_F4           # F4           # KEY_F4         ;
'\u{F708}'	# F5          # Qt_Key_Key_F5           # F5           # KEY_F5         ;
'\u{F709}'	# F6          # Qt_Key_Key_F6           # F6           # KEY_F6         ;
'\u{F70A}'	# F7          # Qt_Key_Key_F7           # F7           # KEY_F7         ;
'\u{F70B}'	# F8          # Qt_Key_Key_F8           # F8           # KEY_F8         ;
'\u{F70C}'	# F9          # Qt_Key_Key_F9           # F9           # KEY_F9         ;
'\u{F70D}'	# F10         # Qt_Key_Key_F10          # F10          # KEY_F10        ;
'\u{F70E}'	# F11         # Qt_Key_Key_F11          # F11          # KEY_F11        ;
'\u{F70F}'	# F12         # Qt_Key_Key_F12          # F12          # KEY_F12        ;
'\u{F710}'	# F13         # Qt_Key_Key_F13          # F13          # KEY_F13        ;
'\u{F711}'	# F14         # Qt_Key_Key_F14          # F14          # KEY_F14        ;
'\u{F712}'	# F15         # Qt_Key_Key_F15          # F15          # KEY_F15        ;
'\u{F713}'	# F16         # Qt_Key_Key_F16          # F16          # KEY_F16        ;
'\u{F714}'	# F17         # Qt_Key_Key_F17          # F17          # KEY_F17        ;
'\u{F715}'	# F18         # Qt_Key_Key_F18          # F18          # KEY_F18        ;
'\u{F716}'	# F19         # Qt_Key_Key_F19          # F19          # KEY_F19        ;
'\u{F717}'	# F20         # Qt_Key_Key_F20          # F20          # KEY_F20        ;
'\u{F718}'	# F21         # Qt_Key_Key_F21          # F21          # KEY_F21        ;
'\u{F719}'	# F22         # Qt_Key_Key_F22          # F22          # KEY_F22        ;
'\u{F71A}'	# F23         # Qt_Key_Key_F23          # F23          # KEY_F23        ;
'\u{F71B}'	# F24         # Qt_Key_Key_F24          # F24          # KEY_F24        ;
//'\u{F71C}'# F25         # Qt_Key_Key_F25          #              # KEY_F25        ;
//'\u{F71D}'# F26         # Qt_Key_Key_F26          #              # KEY_F26        ;
//'\u{F71E}'# F27         # Qt_Key_Key_F27          #              # KEY_F27        ;
//'\u{F71F}'# F28         # Qt_Key_Key_F28          #              # KEY_F28        ;
//'\u{F720}'# F29         # Qt_Key_Key_F29          #              # KEY_F29        ;
//'\u{F721}'# F30         # Qt_Key_Key_F30          #              # KEY_F30        ;
//'\u{F722}'# F31         # Qt_Key_Key_F31          #              # KEY_F31        ;
//'\u{F723}'# F32         # Qt_Key_Key_F32          #              # KEY_F32        ;
//'\u{F724}'# F33         # Qt_Key_Key_F33          #              # KEY_F33        ;
//'\u{F725}'# F34         # Qt_Key_Key_F34          #              # KEY_F34        ;
//'\u{F726}'# F35         # Qt_Key_Key_F35          #              # KEY_F35        ;
'\u{F727}'	# Insert      # Qt_Key_Key_Insert       # Insert       # KEY_Insert     ;
//'\u{F728}'	# Delete     ;  // already as a control code
'\u{F729}'	# Home        # Qt_Key_Key_Home         # Home         # KEY_Home       ;
//'\u{F72A}'	# Begin       #                         #              ;
'\u{F72B}'	# End         # Qt_Key_Key_End          # End          # KEY_End        ;
'\u{F72C}'	# PageUp      # Qt_Key_Key_PageUp       # PageUp       # KEY_Page_Up    ;
'\u{F72D}'	# PageDown    # Qt_Key_Key_PageDown     # PageDown     # KEY_Page_Down  ;
//'\u{F72E}'	# PrintScreen #                         # Snapshot     ;
'\u{F72F}'	# ScrollLock  # Qt_Key_Key_ScrollLock   # Scroll       # KEY_Scroll_Lock;
'\u{F730}'	# Pause       # Qt_Key_Key_Pause        # Pause        # KEY_Pause      ;
'\u{F731}'	# SysReq      # Qt_Key_Key_SysReq       # Sysrq        # KEY_Sys_Req    ;
//'\u{F732}'	# Break       #                         #              ;
//'\u{F733}'	# Reset       #                         #              ;
'\u{F734}'	# Stop        # Qt_Key_Key_Stop         # Stop         # KEY_XF86Stop       ;
'\u{F735}'	# Menu        # Qt_Key_Key_Menu         #              # KEY_Menu       ;
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
