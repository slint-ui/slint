// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-commercial

/*! Generated with Qt5 and
```sh
bindgen /usr/include/qt/QtCore/qnamespace.h --whitelist-type Qt::Key --whitelist-type Qt::KeyboardModifier --whitelist-type Qt::AlignmentFlag --whitelist-type Qt::TextFlag --whitelist-type Qt::FillRule --whitelist-type Qt::CursorShape -o internal/backends/qt/key_generated.rs -- -I /usr/include/qt -xc++
```
then add license header and this doc
*/
#![allow(unused)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

/* automatically generated by rust-bindgen 0.59.1 */

pub const Qt_KeyboardModifier_NoModifier: Qt_KeyboardModifier = 0;
pub const Qt_KeyboardModifier_ShiftModifier: Qt_KeyboardModifier = 33554432;
pub const Qt_KeyboardModifier_ControlModifier: Qt_KeyboardModifier = 67108864;
pub const Qt_KeyboardModifier_AltModifier: Qt_KeyboardModifier = 134217728;
pub const Qt_KeyboardModifier_MetaModifier: Qt_KeyboardModifier = 268435456;
pub const Qt_KeyboardModifier_KeypadModifier: Qt_KeyboardModifier = 536870912;
pub const Qt_KeyboardModifier_GroupSwitchModifier: Qt_KeyboardModifier = 1073741824;
pub const Qt_KeyboardModifier_KeyboardModifierMask: Qt_KeyboardModifier = 4261412864;
pub type Qt_KeyboardModifier = ::std::os::raw::c_uint;
pub const Qt_AlignmentFlag_AlignLeft: Qt_AlignmentFlag = 1;
pub const Qt_AlignmentFlag_AlignLeading: Qt_AlignmentFlag = 1;
pub const Qt_AlignmentFlag_AlignRight: Qt_AlignmentFlag = 2;
pub const Qt_AlignmentFlag_AlignTrailing: Qt_AlignmentFlag = 2;
pub const Qt_AlignmentFlag_AlignHCenter: Qt_AlignmentFlag = 4;
pub const Qt_AlignmentFlag_AlignJustify: Qt_AlignmentFlag = 8;
pub const Qt_AlignmentFlag_AlignAbsolute: Qt_AlignmentFlag = 16;
pub const Qt_AlignmentFlag_AlignHorizontal_Mask: Qt_AlignmentFlag = 31;
pub const Qt_AlignmentFlag_AlignTop: Qt_AlignmentFlag = 32;
pub const Qt_AlignmentFlag_AlignBottom: Qt_AlignmentFlag = 64;
pub const Qt_AlignmentFlag_AlignVCenter: Qt_AlignmentFlag = 128;
pub const Qt_AlignmentFlag_AlignBaseline: Qt_AlignmentFlag = 256;
pub const Qt_AlignmentFlag_AlignVertical_Mask: Qt_AlignmentFlag = 480;
pub const Qt_AlignmentFlag_AlignCenter: Qt_AlignmentFlag = 132;
pub type Qt_AlignmentFlag = ::std::os::raw::c_uint;
pub const Qt_TextFlag_TextSingleLine: Qt_TextFlag = 256;
pub const Qt_TextFlag_TextDontClip: Qt_TextFlag = 512;
pub const Qt_TextFlag_TextExpandTabs: Qt_TextFlag = 1024;
pub const Qt_TextFlag_TextShowMnemonic: Qt_TextFlag = 2048;
pub const Qt_TextFlag_TextWordWrap: Qt_TextFlag = 4096;
pub const Qt_TextFlag_TextWrapAnywhere: Qt_TextFlag = 8192;
pub const Qt_TextFlag_TextDontPrint: Qt_TextFlag = 16384;
pub const Qt_TextFlag_TextIncludeTrailingSpaces: Qt_TextFlag = 134217728;
pub const Qt_TextFlag_TextHideMnemonic: Qt_TextFlag = 32768;
pub const Qt_TextFlag_TextJustificationForced: Qt_TextFlag = 65536;
pub const Qt_TextFlag_TextForceLeftToRight: Qt_TextFlag = 131072;
pub const Qt_TextFlag_TextForceRightToLeft: Qt_TextFlag = 262144;
pub const Qt_TextFlag_TextLongestVariant: Qt_TextFlag = 524288;
pub const Qt_TextFlag_TextBypassShaping: Qt_TextFlag = 1048576;
pub type Qt_TextFlag = ::std::os::raw::c_uint;
pub const Qt_Key_Key_Escape: Qt_Key = 16777216;
pub const Qt_Key_Key_Tab: Qt_Key = 16777217;
pub const Qt_Key_Key_Backtab: Qt_Key = 16777218;
pub const Qt_Key_Key_Backspace: Qt_Key = 16777219;
pub const Qt_Key_Key_Return: Qt_Key = 16777220;
pub const Qt_Key_Key_Enter: Qt_Key = 16777221;
pub const Qt_Key_Key_Insert: Qt_Key = 16777222;
pub const Qt_Key_Key_Delete: Qt_Key = 16777223;
pub const Qt_Key_Key_Pause: Qt_Key = 16777224;
pub const Qt_Key_Key_Print: Qt_Key = 16777225;
pub const Qt_Key_Key_SysReq: Qt_Key = 16777226;
pub const Qt_Key_Key_Clear: Qt_Key = 16777227;
pub const Qt_Key_Key_Home: Qt_Key = 16777232;
pub const Qt_Key_Key_End: Qt_Key = 16777233;
pub const Qt_Key_Key_Left: Qt_Key = 16777234;
pub const Qt_Key_Key_Up: Qt_Key = 16777235;
pub const Qt_Key_Key_Right: Qt_Key = 16777236;
pub const Qt_Key_Key_Down: Qt_Key = 16777237;
pub const Qt_Key_Key_PageUp: Qt_Key = 16777238;
pub const Qt_Key_Key_PageDown: Qt_Key = 16777239;
pub const Qt_Key_Key_Shift: Qt_Key = 16777248;
pub const Qt_Key_Key_Control: Qt_Key = 16777249;
pub const Qt_Key_Key_Meta: Qt_Key = 16777250;
pub const Qt_Key_Key_Alt: Qt_Key = 16777251;
pub const Qt_Key_Key_CapsLock: Qt_Key = 16777252;
pub const Qt_Key_Key_NumLock: Qt_Key = 16777253;
pub const Qt_Key_Key_ScrollLock: Qt_Key = 16777254;
pub const Qt_Key_Key_F1: Qt_Key = 16777264;
pub const Qt_Key_Key_F2: Qt_Key = 16777265;
pub const Qt_Key_Key_F3: Qt_Key = 16777266;
pub const Qt_Key_Key_F4: Qt_Key = 16777267;
pub const Qt_Key_Key_F5: Qt_Key = 16777268;
pub const Qt_Key_Key_F6: Qt_Key = 16777269;
pub const Qt_Key_Key_F7: Qt_Key = 16777270;
pub const Qt_Key_Key_F8: Qt_Key = 16777271;
pub const Qt_Key_Key_F9: Qt_Key = 16777272;
pub const Qt_Key_Key_F10: Qt_Key = 16777273;
pub const Qt_Key_Key_F11: Qt_Key = 16777274;
pub const Qt_Key_Key_F12: Qt_Key = 16777275;
pub const Qt_Key_Key_F13: Qt_Key = 16777276;
pub const Qt_Key_Key_F14: Qt_Key = 16777277;
pub const Qt_Key_Key_F15: Qt_Key = 16777278;
pub const Qt_Key_Key_F16: Qt_Key = 16777279;
pub const Qt_Key_Key_F17: Qt_Key = 16777280;
pub const Qt_Key_Key_F18: Qt_Key = 16777281;
pub const Qt_Key_Key_F19: Qt_Key = 16777282;
pub const Qt_Key_Key_F20: Qt_Key = 16777283;
pub const Qt_Key_Key_F21: Qt_Key = 16777284;
pub const Qt_Key_Key_F22: Qt_Key = 16777285;
pub const Qt_Key_Key_F23: Qt_Key = 16777286;
pub const Qt_Key_Key_F24: Qt_Key = 16777287;
pub const Qt_Key_Key_F25: Qt_Key = 16777288;
pub const Qt_Key_Key_F26: Qt_Key = 16777289;
pub const Qt_Key_Key_F27: Qt_Key = 16777290;
pub const Qt_Key_Key_F28: Qt_Key = 16777291;
pub const Qt_Key_Key_F29: Qt_Key = 16777292;
pub const Qt_Key_Key_F30: Qt_Key = 16777293;
pub const Qt_Key_Key_F31: Qt_Key = 16777294;
pub const Qt_Key_Key_F32: Qt_Key = 16777295;
pub const Qt_Key_Key_F33: Qt_Key = 16777296;
pub const Qt_Key_Key_F34: Qt_Key = 16777297;
pub const Qt_Key_Key_F35: Qt_Key = 16777298;
pub const Qt_Key_Key_Super_L: Qt_Key = 16777299;
pub const Qt_Key_Key_Super_R: Qt_Key = 16777300;
pub const Qt_Key_Key_Menu: Qt_Key = 16777301;
pub const Qt_Key_Key_Hyper_L: Qt_Key = 16777302;
pub const Qt_Key_Key_Hyper_R: Qt_Key = 16777303;
pub const Qt_Key_Key_Help: Qt_Key = 16777304;
pub const Qt_Key_Key_Direction_L: Qt_Key = 16777305;
pub const Qt_Key_Key_Direction_R: Qt_Key = 16777312;
pub const Qt_Key_Key_Space: Qt_Key = 32;
pub const Qt_Key_Key_Any: Qt_Key = 32;
pub const Qt_Key_Key_Exclam: Qt_Key = 33;
pub const Qt_Key_Key_QuoteDbl: Qt_Key = 34;
pub const Qt_Key_Key_NumberSign: Qt_Key = 35;
pub const Qt_Key_Key_Dollar: Qt_Key = 36;
pub const Qt_Key_Key_Percent: Qt_Key = 37;
pub const Qt_Key_Key_Ampersand: Qt_Key = 38;
pub const Qt_Key_Key_Apostrophe: Qt_Key = 39;
pub const Qt_Key_Key_ParenLeft: Qt_Key = 40;
pub const Qt_Key_Key_ParenRight: Qt_Key = 41;
pub const Qt_Key_Key_Asterisk: Qt_Key = 42;
pub const Qt_Key_Key_Plus: Qt_Key = 43;
pub const Qt_Key_Key_Comma: Qt_Key = 44;
pub const Qt_Key_Key_Minus: Qt_Key = 45;
pub const Qt_Key_Key_Period: Qt_Key = 46;
pub const Qt_Key_Key_Slash: Qt_Key = 47;
pub const Qt_Key_Key_0: Qt_Key = 48;
pub const Qt_Key_Key_1: Qt_Key = 49;
pub const Qt_Key_Key_2: Qt_Key = 50;
pub const Qt_Key_Key_3: Qt_Key = 51;
pub const Qt_Key_Key_4: Qt_Key = 52;
pub const Qt_Key_Key_5: Qt_Key = 53;
pub const Qt_Key_Key_6: Qt_Key = 54;
pub const Qt_Key_Key_7: Qt_Key = 55;
pub const Qt_Key_Key_8: Qt_Key = 56;
pub const Qt_Key_Key_9: Qt_Key = 57;
pub const Qt_Key_Key_Colon: Qt_Key = 58;
pub const Qt_Key_Key_Semicolon: Qt_Key = 59;
pub const Qt_Key_Key_Less: Qt_Key = 60;
pub const Qt_Key_Key_Equal: Qt_Key = 61;
pub const Qt_Key_Key_Greater: Qt_Key = 62;
pub const Qt_Key_Key_Question: Qt_Key = 63;
pub const Qt_Key_Key_At: Qt_Key = 64;
pub const Qt_Key_Key_A: Qt_Key = 65;
pub const Qt_Key_Key_B: Qt_Key = 66;
pub const Qt_Key_Key_C: Qt_Key = 67;
pub const Qt_Key_Key_D: Qt_Key = 68;
pub const Qt_Key_Key_E: Qt_Key = 69;
pub const Qt_Key_Key_F: Qt_Key = 70;
pub const Qt_Key_Key_G: Qt_Key = 71;
pub const Qt_Key_Key_H: Qt_Key = 72;
pub const Qt_Key_Key_I: Qt_Key = 73;
pub const Qt_Key_Key_J: Qt_Key = 74;
pub const Qt_Key_Key_K: Qt_Key = 75;
pub const Qt_Key_Key_L: Qt_Key = 76;
pub const Qt_Key_Key_M: Qt_Key = 77;
pub const Qt_Key_Key_N: Qt_Key = 78;
pub const Qt_Key_Key_O: Qt_Key = 79;
pub const Qt_Key_Key_P: Qt_Key = 80;
pub const Qt_Key_Key_Q: Qt_Key = 81;
pub const Qt_Key_Key_R: Qt_Key = 82;
pub const Qt_Key_Key_S: Qt_Key = 83;
pub const Qt_Key_Key_T: Qt_Key = 84;
pub const Qt_Key_Key_U: Qt_Key = 85;
pub const Qt_Key_Key_V: Qt_Key = 86;
pub const Qt_Key_Key_W: Qt_Key = 87;
pub const Qt_Key_Key_X: Qt_Key = 88;
pub const Qt_Key_Key_Y: Qt_Key = 89;
pub const Qt_Key_Key_Z: Qt_Key = 90;
pub const Qt_Key_Key_BracketLeft: Qt_Key = 91;
pub const Qt_Key_Key_Backslash: Qt_Key = 92;
pub const Qt_Key_Key_BracketRight: Qt_Key = 93;
pub const Qt_Key_Key_AsciiCircum: Qt_Key = 94;
pub const Qt_Key_Key_Underscore: Qt_Key = 95;
pub const Qt_Key_Key_QuoteLeft: Qt_Key = 96;
pub const Qt_Key_Key_BraceLeft: Qt_Key = 123;
pub const Qt_Key_Key_Bar: Qt_Key = 124;
pub const Qt_Key_Key_BraceRight: Qt_Key = 125;
pub const Qt_Key_Key_AsciiTilde: Qt_Key = 126;
pub const Qt_Key_Key_nobreakspace: Qt_Key = 160;
pub const Qt_Key_Key_exclamdown: Qt_Key = 161;
pub const Qt_Key_Key_cent: Qt_Key = 162;
pub const Qt_Key_Key_sterling: Qt_Key = 163;
pub const Qt_Key_Key_currency: Qt_Key = 164;
pub const Qt_Key_Key_yen: Qt_Key = 165;
pub const Qt_Key_Key_brokenbar: Qt_Key = 166;
pub const Qt_Key_Key_section: Qt_Key = 167;
pub const Qt_Key_Key_diaeresis: Qt_Key = 168;
pub const Qt_Key_Key_copyright: Qt_Key = 169;
pub const Qt_Key_Key_ordfeminine: Qt_Key = 170;
pub const Qt_Key_Key_guillemotleft: Qt_Key = 171;
pub const Qt_Key_Key_notsign: Qt_Key = 172;
pub const Qt_Key_Key_hyphen: Qt_Key = 173;
pub const Qt_Key_Key_registered: Qt_Key = 174;
pub const Qt_Key_Key_macron: Qt_Key = 175;
pub const Qt_Key_Key_degree: Qt_Key = 176;
pub const Qt_Key_Key_plusminus: Qt_Key = 177;
pub const Qt_Key_Key_twosuperior: Qt_Key = 178;
pub const Qt_Key_Key_threesuperior: Qt_Key = 179;
pub const Qt_Key_Key_acute: Qt_Key = 180;
pub const Qt_Key_Key_mu: Qt_Key = 181;
pub const Qt_Key_Key_paragraph: Qt_Key = 182;
pub const Qt_Key_Key_periodcentered: Qt_Key = 183;
pub const Qt_Key_Key_cedilla: Qt_Key = 184;
pub const Qt_Key_Key_onesuperior: Qt_Key = 185;
pub const Qt_Key_Key_masculine: Qt_Key = 186;
pub const Qt_Key_Key_guillemotright: Qt_Key = 187;
pub const Qt_Key_Key_onequarter: Qt_Key = 188;
pub const Qt_Key_Key_onehalf: Qt_Key = 189;
pub const Qt_Key_Key_threequarters: Qt_Key = 190;
pub const Qt_Key_Key_questiondown: Qt_Key = 191;
pub const Qt_Key_Key_Agrave: Qt_Key = 192;
pub const Qt_Key_Key_Aacute: Qt_Key = 193;
pub const Qt_Key_Key_Acircumflex: Qt_Key = 194;
pub const Qt_Key_Key_Atilde: Qt_Key = 195;
pub const Qt_Key_Key_Adiaeresis: Qt_Key = 196;
pub const Qt_Key_Key_Aring: Qt_Key = 197;
pub const Qt_Key_Key_AE: Qt_Key = 198;
pub const Qt_Key_Key_Ccedilla: Qt_Key = 199;
pub const Qt_Key_Key_Egrave: Qt_Key = 200;
pub const Qt_Key_Key_Eacute: Qt_Key = 201;
pub const Qt_Key_Key_Ecircumflex: Qt_Key = 202;
pub const Qt_Key_Key_Ediaeresis: Qt_Key = 203;
pub const Qt_Key_Key_Igrave: Qt_Key = 204;
pub const Qt_Key_Key_Iacute: Qt_Key = 205;
pub const Qt_Key_Key_Icircumflex: Qt_Key = 206;
pub const Qt_Key_Key_Idiaeresis: Qt_Key = 207;
pub const Qt_Key_Key_ETH: Qt_Key = 208;
pub const Qt_Key_Key_Ntilde: Qt_Key = 209;
pub const Qt_Key_Key_Ograve: Qt_Key = 210;
pub const Qt_Key_Key_Oacute: Qt_Key = 211;
pub const Qt_Key_Key_Ocircumflex: Qt_Key = 212;
pub const Qt_Key_Key_Otilde: Qt_Key = 213;
pub const Qt_Key_Key_Odiaeresis: Qt_Key = 214;
pub const Qt_Key_Key_multiply: Qt_Key = 215;
pub const Qt_Key_Key_Ooblique: Qt_Key = 216;
pub const Qt_Key_Key_Ugrave: Qt_Key = 217;
pub const Qt_Key_Key_Uacute: Qt_Key = 218;
pub const Qt_Key_Key_Ucircumflex: Qt_Key = 219;
pub const Qt_Key_Key_Udiaeresis: Qt_Key = 220;
pub const Qt_Key_Key_Yacute: Qt_Key = 221;
pub const Qt_Key_Key_THORN: Qt_Key = 222;
pub const Qt_Key_Key_ssharp: Qt_Key = 223;
pub const Qt_Key_Key_division: Qt_Key = 247;
pub const Qt_Key_Key_ydiaeresis: Qt_Key = 255;
pub const Qt_Key_Key_AltGr: Qt_Key = 16781571;
pub const Qt_Key_Key_Multi_key: Qt_Key = 16781600;
pub const Qt_Key_Key_Codeinput: Qt_Key = 16781623;
pub const Qt_Key_Key_SingleCandidate: Qt_Key = 16781628;
pub const Qt_Key_Key_MultipleCandidate: Qt_Key = 16781629;
pub const Qt_Key_Key_PreviousCandidate: Qt_Key = 16781630;
pub const Qt_Key_Key_Mode_switch: Qt_Key = 16781694;
pub const Qt_Key_Key_Kanji: Qt_Key = 16781601;
pub const Qt_Key_Key_Muhenkan: Qt_Key = 16781602;
pub const Qt_Key_Key_Henkan: Qt_Key = 16781603;
pub const Qt_Key_Key_Romaji: Qt_Key = 16781604;
pub const Qt_Key_Key_Hiragana: Qt_Key = 16781605;
pub const Qt_Key_Key_Katakana: Qt_Key = 16781606;
pub const Qt_Key_Key_Hiragana_Katakana: Qt_Key = 16781607;
pub const Qt_Key_Key_Zenkaku: Qt_Key = 16781608;
pub const Qt_Key_Key_Hankaku: Qt_Key = 16781609;
pub const Qt_Key_Key_Zenkaku_Hankaku: Qt_Key = 16781610;
pub const Qt_Key_Key_Touroku: Qt_Key = 16781611;
pub const Qt_Key_Key_Massyo: Qt_Key = 16781612;
pub const Qt_Key_Key_Kana_Lock: Qt_Key = 16781613;
pub const Qt_Key_Key_Kana_Shift: Qt_Key = 16781614;
pub const Qt_Key_Key_Eisu_Shift: Qt_Key = 16781615;
pub const Qt_Key_Key_Eisu_toggle: Qt_Key = 16781616;
pub const Qt_Key_Key_Hangul: Qt_Key = 16781617;
pub const Qt_Key_Key_Hangul_Start: Qt_Key = 16781618;
pub const Qt_Key_Key_Hangul_End: Qt_Key = 16781619;
pub const Qt_Key_Key_Hangul_Hanja: Qt_Key = 16781620;
pub const Qt_Key_Key_Hangul_Jamo: Qt_Key = 16781621;
pub const Qt_Key_Key_Hangul_Romaja: Qt_Key = 16781622;
pub const Qt_Key_Key_Hangul_Jeonja: Qt_Key = 16781624;
pub const Qt_Key_Key_Hangul_Banja: Qt_Key = 16781625;
pub const Qt_Key_Key_Hangul_PreHanja: Qt_Key = 16781626;
pub const Qt_Key_Key_Hangul_PostHanja: Qt_Key = 16781627;
pub const Qt_Key_Key_Hangul_Special: Qt_Key = 16781631;
pub const Qt_Key_Key_Dead_Grave: Qt_Key = 16781904;
pub const Qt_Key_Key_Dead_Acute: Qt_Key = 16781905;
pub const Qt_Key_Key_Dead_Circumflex: Qt_Key = 16781906;
pub const Qt_Key_Key_Dead_Tilde: Qt_Key = 16781907;
pub const Qt_Key_Key_Dead_Macron: Qt_Key = 16781908;
pub const Qt_Key_Key_Dead_Breve: Qt_Key = 16781909;
pub const Qt_Key_Key_Dead_Abovedot: Qt_Key = 16781910;
pub const Qt_Key_Key_Dead_Diaeresis: Qt_Key = 16781911;
pub const Qt_Key_Key_Dead_Abovering: Qt_Key = 16781912;
pub const Qt_Key_Key_Dead_Doubleacute: Qt_Key = 16781913;
pub const Qt_Key_Key_Dead_Caron: Qt_Key = 16781914;
pub const Qt_Key_Key_Dead_Cedilla: Qt_Key = 16781915;
pub const Qt_Key_Key_Dead_Ogonek: Qt_Key = 16781916;
pub const Qt_Key_Key_Dead_Iota: Qt_Key = 16781917;
pub const Qt_Key_Key_Dead_Voiced_Sound: Qt_Key = 16781918;
pub const Qt_Key_Key_Dead_Semivoiced_Sound: Qt_Key = 16781919;
pub const Qt_Key_Key_Dead_Belowdot: Qt_Key = 16781920;
pub const Qt_Key_Key_Dead_Hook: Qt_Key = 16781921;
pub const Qt_Key_Key_Dead_Horn: Qt_Key = 16781922;
pub const Qt_Key_Key_Dead_Stroke: Qt_Key = 16781923;
pub const Qt_Key_Key_Dead_Abovecomma: Qt_Key = 16781924;
pub const Qt_Key_Key_Dead_Abovereversedcomma: Qt_Key = 16781925;
pub const Qt_Key_Key_Dead_Doublegrave: Qt_Key = 16781926;
pub const Qt_Key_Key_Dead_Belowring: Qt_Key = 16781927;
pub const Qt_Key_Key_Dead_Belowmacron: Qt_Key = 16781928;
pub const Qt_Key_Key_Dead_Belowcircumflex: Qt_Key = 16781929;
pub const Qt_Key_Key_Dead_Belowtilde: Qt_Key = 16781930;
pub const Qt_Key_Key_Dead_Belowbreve: Qt_Key = 16781931;
pub const Qt_Key_Key_Dead_Belowdiaeresis: Qt_Key = 16781932;
pub const Qt_Key_Key_Dead_Invertedbreve: Qt_Key = 16781933;
pub const Qt_Key_Key_Dead_Belowcomma: Qt_Key = 16781934;
pub const Qt_Key_Key_Dead_Currency: Qt_Key = 16781935;
pub const Qt_Key_Key_Dead_a: Qt_Key = 16781952;
pub const Qt_Key_Key_Dead_A: Qt_Key = 16781953;
pub const Qt_Key_Key_Dead_e: Qt_Key = 16781954;
pub const Qt_Key_Key_Dead_E: Qt_Key = 16781955;
pub const Qt_Key_Key_Dead_i: Qt_Key = 16781956;
pub const Qt_Key_Key_Dead_I: Qt_Key = 16781957;
pub const Qt_Key_Key_Dead_o: Qt_Key = 16781958;
pub const Qt_Key_Key_Dead_O: Qt_Key = 16781959;
pub const Qt_Key_Key_Dead_u: Qt_Key = 16781960;
pub const Qt_Key_Key_Dead_U: Qt_Key = 16781961;
pub const Qt_Key_Key_Dead_Small_Schwa: Qt_Key = 16781962;
pub const Qt_Key_Key_Dead_Capital_Schwa: Qt_Key = 16781963;
pub const Qt_Key_Key_Dead_Greek: Qt_Key = 16781964;
pub const Qt_Key_Key_Dead_Lowline: Qt_Key = 16781968;
pub const Qt_Key_Key_Dead_Aboveverticalline: Qt_Key = 16781969;
pub const Qt_Key_Key_Dead_Belowverticalline: Qt_Key = 16781970;
pub const Qt_Key_Key_Dead_Longsolidusoverlay: Qt_Key = 16781971;
pub const Qt_Key_Key_Back: Qt_Key = 16777313;
pub const Qt_Key_Key_Forward: Qt_Key = 16777314;
pub const Qt_Key_Key_Stop: Qt_Key = 16777315;
pub const Qt_Key_Key_Refresh: Qt_Key = 16777316;
pub const Qt_Key_Key_VolumeDown: Qt_Key = 16777328;
pub const Qt_Key_Key_VolumeMute: Qt_Key = 16777329;
pub const Qt_Key_Key_VolumeUp: Qt_Key = 16777330;
pub const Qt_Key_Key_BassBoost: Qt_Key = 16777331;
pub const Qt_Key_Key_BassUp: Qt_Key = 16777332;
pub const Qt_Key_Key_BassDown: Qt_Key = 16777333;
pub const Qt_Key_Key_TrebleUp: Qt_Key = 16777334;
pub const Qt_Key_Key_TrebleDown: Qt_Key = 16777335;
pub const Qt_Key_Key_MediaPlay: Qt_Key = 16777344;
pub const Qt_Key_Key_MediaStop: Qt_Key = 16777345;
pub const Qt_Key_Key_MediaPrevious: Qt_Key = 16777346;
pub const Qt_Key_Key_MediaNext: Qt_Key = 16777347;
pub const Qt_Key_Key_MediaRecord: Qt_Key = 16777348;
pub const Qt_Key_Key_MediaPause: Qt_Key = 16777349;
pub const Qt_Key_Key_MediaTogglePlayPause: Qt_Key = 16777350;
pub const Qt_Key_Key_HomePage: Qt_Key = 16777360;
pub const Qt_Key_Key_Favorites: Qt_Key = 16777361;
pub const Qt_Key_Key_Search: Qt_Key = 16777362;
pub const Qt_Key_Key_Standby: Qt_Key = 16777363;
pub const Qt_Key_Key_OpenUrl: Qt_Key = 16777364;
pub const Qt_Key_Key_LaunchMail: Qt_Key = 16777376;
pub const Qt_Key_Key_LaunchMedia: Qt_Key = 16777377;
pub const Qt_Key_Key_Launch0: Qt_Key = 16777378;
pub const Qt_Key_Key_Launch1: Qt_Key = 16777379;
pub const Qt_Key_Key_Launch2: Qt_Key = 16777380;
pub const Qt_Key_Key_Launch3: Qt_Key = 16777381;
pub const Qt_Key_Key_Launch4: Qt_Key = 16777382;
pub const Qt_Key_Key_Launch5: Qt_Key = 16777383;
pub const Qt_Key_Key_Launch6: Qt_Key = 16777384;
pub const Qt_Key_Key_Launch7: Qt_Key = 16777385;
pub const Qt_Key_Key_Launch8: Qt_Key = 16777386;
pub const Qt_Key_Key_Launch9: Qt_Key = 16777387;
pub const Qt_Key_Key_LaunchA: Qt_Key = 16777388;
pub const Qt_Key_Key_LaunchB: Qt_Key = 16777389;
pub const Qt_Key_Key_LaunchC: Qt_Key = 16777390;
pub const Qt_Key_Key_LaunchD: Qt_Key = 16777391;
pub const Qt_Key_Key_LaunchE: Qt_Key = 16777392;
pub const Qt_Key_Key_LaunchF: Qt_Key = 16777393;
pub const Qt_Key_Key_MonBrightnessUp: Qt_Key = 16777394;
pub const Qt_Key_Key_MonBrightnessDown: Qt_Key = 16777395;
pub const Qt_Key_Key_KeyboardLightOnOff: Qt_Key = 16777396;
pub const Qt_Key_Key_KeyboardBrightnessUp: Qt_Key = 16777397;
pub const Qt_Key_Key_KeyboardBrightnessDown: Qt_Key = 16777398;
pub const Qt_Key_Key_PowerOff: Qt_Key = 16777399;
pub const Qt_Key_Key_WakeUp: Qt_Key = 16777400;
pub const Qt_Key_Key_Eject: Qt_Key = 16777401;
pub const Qt_Key_Key_ScreenSaver: Qt_Key = 16777402;
pub const Qt_Key_Key_WWW: Qt_Key = 16777403;
pub const Qt_Key_Key_Memo: Qt_Key = 16777404;
pub const Qt_Key_Key_LightBulb: Qt_Key = 16777405;
pub const Qt_Key_Key_Shop: Qt_Key = 16777406;
pub const Qt_Key_Key_History: Qt_Key = 16777407;
pub const Qt_Key_Key_AddFavorite: Qt_Key = 16777408;
pub const Qt_Key_Key_HotLinks: Qt_Key = 16777409;
pub const Qt_Key_Key_BrightnessAdjust: Qt_Key = 16777410;
pub const Qt_Key_Key_Finance: Qt_Key = 16777411;
pub const Qt_Key_Key_Community: Qt_Key = 16777412;
pub const Qt_Key_Key_AudioRewind: Qt_Key = 16777413;
pub const Qt_Key_Key_BackForward: Qt_Key = 16777414;
pub const Qt_Key_Key_ApplicationLeft: Qt_Key = 16777415;
pub const Qt_Key_Key_ApplicationRight: Qt_Key = 16777416;
pub const Qt_Key_Key_Book: Qt_Key = 16777417;
pub const Qt_Key_Key_CD: Qt_Key = 16777418;
pub const Qt_Key_Key_Calculator: Qt_Key = 16777419;
pub const Qt_Key_Key_ToDoList: Qt_Key = 16777420;
pub const Qt_Key_Key_ClearGrab: Qt_Key = 16777421;
pub const Qt_Key_Key_Close: Qt_Key = 16777422;
pub const Qt_Key_Key_Copy: Qt_Key = 16777423;
pub const Qt_Key_Key_Cut: Qt_Key = 16777424;
pub const Qt_Key_Key_Display: Qt_Key = 16777425;
pub const Qt_Key_Key_DOS: Qt_Key = 16777426;
pub const Qt_Key_Key_Documents: Qt_Key = 16777427;
pub const Qt_Key_Key_Excel: Qt_Key = 16777428;
pub const Qt_Key_Key_Explorer: Qt_Key = 16777429;
pub const Qt_Key_Key_Game: Qt_Key = 16777430;
pub const Qt_Key_Key_Go: Qt_Key = 16777431;
pub const Qt_Key_Key_iTouch: Qt_Key = 16777432;
pub const Qt_Key_Key_LogOff: Qt_Key = 16777433;
pub const Qt_Key_Key_Market: Qt_Key = 16777434;
pub const Qt_Key_Key_Meeting: Qt_Key = 16777435;
pub const Qt_Key_Key_MenuKB: Qt_Key = 16777436;
pub const Qt_Key_Key_MenuPB: Qt_Key = 16777437;
pub const Qt_Key_Key_MySites: Qt_Key = 16777438;
pub const Qt_Key_Key_News: Qt_Key = 16777439;
pub const Qt_Key_Key_OfficeHome: Qt_Key = 16777440;
pub const Qt_Key_Key_Option: Qt_Key = 16777441;
pub const Qt_Key_Key_Paste: Qt_Key = 16777442;
pub const Qt_Key_Key_Phone: Qt_Key = 16777443;
pub const Qt_Key_Key_Calendar: Qt_Key = 16777444;
pub const Qt_Key_Key_Reply: Qt_Key = 16777445;
pub const Qt_Key_Key_Reload: Qt_Key = 16777446;
pub const Qt_Key_Key_RotateWindows: Qt_Key = 16777447;
pub const Qt_Key_Key_RotationPB: Qt_Key = 16777448;
pub const Qt_Key_Key_RotationKB: Qt_Key = 16777449;
pub const Qt_Key_Key_Save: Qt_Key = 16777450;
pub const Qt_Key_Key_Send: Qt_Key = 16777451;
pub const Qt_Key_Key_Spell: Qt_Key = 16777452;
pub const Qt_Key_Key_SplitScreen: Qt_Key = 16777453;
pub const Qt_Key_Key_Support: Qt_Key = 16777454;
pub const Qt_Key_Key_TaskPane: Qt_Key = 16777455;
pub const Qt_Key_Key_Terminal: Qt_Key = 16777456;
pub const Qt_Key_Key_Tools: Qt_Key = 16777457;
pub const Qt_Key_Key_Travel: Qt_Key = 16777458;
pub const Qt_Key_Key_Video: Qt_Key = 16777459;
pub const Qt_Key_Key_Word: Qt_Key = 16777460;
pub const Qt_Key_Key_Xfer: Qt_Key = 16777461;
pub const Qt_Key_Key_ZoomIn: Qt_Key = 16777462;
pub const Qt_Key_Key_ZoomOut: Qt_Key = 16777463;
pub const Qt_Key_Key_Away: Qt_Key = 16777464;
pub const Qt_Key_Key_Messenger: Qt_Key = 16777465;
pub const Qt_Key_Key_WebCam: Qt_Key = 16777466;
pub const Qt_Key_Key_MailForward: Qt_Key = 16777467;
pub const Qt_Key_Key_Pictures: Qt_Key = 16777468;
pub const Qt_Key_Key_Music: Qt_Key = 16777469;
pub const Qt_Key_Key_Battery: Qt_Key = 16777470;
pub const Qt_Key_Key_Bluetooth: Qt_Key = 16777471;
pub const Qt_Key_Key_WLAN: Qt_Key = 16777472;
pub const Qt_Key_Key_UWB: Qt_Key = 16777473;
pub const Qt_Key_Key_AudioForward: Qt_Key = 16777474;
pub const Qt_Key_Key_AudioRepeat: Qt_Key = 16777475;
pub const Qt_Key_Key_AudioRandomPlay: Qt_Key = 16777476;
pub const Qt_Key_Key_Subtitle: Qt_Key = 16777477;
pub const Qt_Key_Key_AudioCycleTrack: Qt_Key = 16777478;
pub const Qt_Key_Key_Time: Qt_Key = 16777479;
pub const Qt_Key_Key_Hibernate: Qt_Key = 16777480;
pub const Qt_Key_Key_View: Qt_Key = 16777481;
pub const Qt_Key_Key_TopMenu: Qt_Key = 16777482;
pub const Qt_Key_Key_PowerDown: Qt_Key = 16777483;
pub const Qt_Key_Key_Suspend: Qt_Key = 16777484;
pub const Qt_Key_Key_ContrastAdjust: Qt_Key = 16777485;
pub const Qt_Key_Key_LaunchG: Qt_Key = 16777486;
pub const Qt_Key_Key_LaunchH: Qt_Key = 16777487;
pub const Qt_Key_Key_TouchpadToggle: Qt_Key = 16777488;
pub const Qt_Key_Key_TouchpadOn: Qt_Key = 16777489;
pub const Qt_Key_Key_TouchpadOff: Qt_Key = 16777490;
pub const Qt_Key_Key_MicMute: Qt_Key = 16777491;
pub const Qt_Key_Key_Red: Qt_Key = 16777492;
pub const Qt_Key_Key_Green: Qt_Key = 16777493;
pub const Qt_Key_Key_Yellow: Qt_Key = 16777494;
pub const Qt_Key_Key_Blue: Qt_Key = 16777495;
pub const Qt_Key_Key_ChannelUp: Qt_Key = 16777496;
pub const Qt_Key_Key_ChannelDown: Qt_Key = 16777497;
pub const Qt_Key_Key_Guide: Qt_Key = 16777498;
pub const Qt_Key_Key_Info: Qt_Key = 16777499;
pub const Qt_Key_Key_Settings: Qt_Key = 16777500;
pub const Qt_Key_Key_MicVolumeUp: Qt_Key = 16777501;
pub const Qt_Key_Key_MicVolumeDown: Qt_Key = 16777502;
pub const Qt_Key_Key_New: Qt_Key = 16777504;
pub const Qt_Key_Key_Open: Qt_Key = 16777505;
pub const Qt_Key_Key_Find: Qt_Key = 16777506;
pub const Qt_Key_Key_Undo: Qt_Key = 16777507;
pub const Qt_Key_Key_Redo: Qt_Key = 16777508;
pub const Qt_Key_Key_MediaLast: Qt_Key = 16842751;
pub const Qt_Key_Key_Select: Qt_Key = 16842752;
pub const Qt_Key_Key_Yes: Qt_Key = 16842753;
pub const Qt_Key_Key_No: Qt_Key = 16842754;
pub const Qt_Key_Key_Cancel: Qt_Key = 16908289;
pub const Qt_Key_Key_Printer: Qt_Key = 16908290;
pub const Qt_Key_Key_Execute: Qt_Key = 16908291;
pub const Qt_Key_Key_Sleep: Qt_Key = 16908292;
pub const Qt_Key_Key_Play: Qt_Key = 16908293;
pub const Qt_Key_Key_Zoom: Qt_Key = 16908294;
pub const Qt_Key_Key_Exit: Qt_Key = 16908298;
pub const Qt_Key_Key_Context1: Qt_Key = 17825792;
pub const Qt_Key_Key_Context2: Qt_Key = 17825793;
pub const Qt_Key_Key_Context3: Qt_Key = 17825794;
pub const Qt_Key_Key_Context4: Qt_Key = 17825795;
pub const Qt_Key_Key_Call: Qt_Key = 17825796;
pub const Qt_Key_Key_Hangup: Qt_Key = 17825797;
pub const Qt_Key_Key_Flip: Qt_Key = 17825798;
pub const Qt_Key_Key_ToggleCallHangup: Qt_Key = 17825799;
pub const Qt_Key_Key_VoiceDial: Qt_Key = 17825800;
pub const Qt_Key_Key_LastNumberRedial: Qt_Key = 17825801;
pub const Qt_Key_Key_Camera: Qt_Key = 17825824;
pub const Qt_Key_Key_CameraFocus: Qt_Key = 17825825;
pub const Qt_Key_Key_unknown: Qt_Key = 33554431;
pub type Qt_Key = ::std::os::raw::c_uint;
pub const Qt_CursorShape_ArrowCursor: Qt_CursorShape = 0;
pub const Qt_CursorShape_UpArrowCursor: Qt_CursorShape = 1;
pub const Qt_CursorShape_CrossCursor: Qt_CursorShape = 2;
pub const Qt_CursorShape_WaitCursor: Qt_CursorShape = 3;
pub const Qt_CursorShape_IBeamCursor: Qt_CursorShape = 4;
pub const Qt_CursorShape_SizeVerCursor: Qt_CursorShape = 5;
pub const Qt_CursorShape_SizeHorCursor: Qt_CursorShape = 6;
pub const Qt_CursorShape_SizeBDiagCursor: Qt_CursorShape = 7;
pub const Qt_CursorShape_SizeFDiagCursor: Qt_CursorShape = 8;
pub const Qt_CursorShape_SizeAllCursor: Qt_CursorShape = 9;
pub const Qt_CursorShape_BlankCursor: Qt_CursorShape = 10;
pub const Qt_CursorShape_SplitVCursor: Qt_CursorShape = 11;
pub const Qt_CursorShape_SplitHCursor: Qt_CursorShape = 12;
pub const Qt_CursorShape_PointingHandCursor: Qt_CursorShape = 13;
pub const Qt_CursorShape_ForbiddenCursor: Qt_CursorShape = 14;
pub const Qt_CursorShape_WhatsThisCursor: Qt_CursorShape = 15;
pub const Qt_CursorShape_BusyCursor: Qt_CursorShape = 16;
pub const Qt_CursorShape_OpenHandCursor: Qt_CursorShape = 17;
pub const Qt_CursorShape_ClosedHandCursor: Qt_CursorShape = 18;
pub const Qt_CursorShape_DragCopyCursor: Qt_CursorShape = 19;
pub const Qt_CursorShape_DragMoveCursor: Qt_CursorShape = 20;
pub const Qt_CursorShape_DragLinkCursor: Qt_CursorShape = 21;
pub const Qt_CursorShape_LastCursor: Qt_CursorShape = 21;
pub const Qt_CursorShape_BitmapCursor: Qt_CursorShape = 24;
pub const Qt_CursorShape_CustomCursor: Qt_CursorShape = 25;
pub type Qt_CursorShape = ::std::os::raw::c_uint;
pub const Qt_FillRule_OddEvenFill: Qt_FillRule = 0;
pub const Qt_FillRule_WindingFill: Qt_FillRule = 1;
pub type Qt_FillRule = ::std::os::raw::c_uint;
