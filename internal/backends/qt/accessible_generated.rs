// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*! Generated with Qt5 and
```sh
bindgen /usr/include/qt/QtGui/qaccessible.h --allowlist-type QAccessible_Role --allowlist-type QAccessible_Event --allowlist-type QAccessible_Text -o internal/backends/qt/accessible_generated.rs -- -I /usr/include/qt -xc++
```
then add license header and this doc incl. the following comment and allow lines:
*/

// cSpell: ignore qaccessible

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(unused)]

/* automatically generated by rust-bindgen 0.59.2 */

pub const QAccessible_Event_SoundPlayed: QAccessible_Event = 1;
pub const QAccessible_Event_Alert: QAccessible_Event = 2;
pub const QAccessible_Event_ForegroundChanged: QAccessible_Event = 3;
pub const QAccessible_Event_MenuStart: QAccessible_Event = 4;
pub const QAccessible_Event_MenuEnd: QAccessible_Event = 5;
pub const QAccessible_Event_PopupMenuStart: QAccessible_Event = 6;
pub const QAccessible_Event_PopupMenuEnd: QAccessible_Event = 7;
pub const QAccessible_Event_ContextHelpStart: QAccessible_Event = 12;
pub const QAccessible_Event_ContextHelpEnd: QAccessible_Event = 13;
pub const QAccessible_Event_DragDropStart: QAccessible_Event = 14;
pub const QAccessible_Event_DragDropEnd: QAccessible_Event = 15;
pub const QAccessible_Event_DialogStart: QAccessible_Event = 16;
pub const QAccessible_Event_DialogEnd: QAccessible_Event = 17;
pub const QAccessible_Event_ScrollingStart: QAccessible_Event = 18;
pub const QAccessible_Event_ScrollingEnd: QAccessible_Event = 19;
pub const QAccessible_Event_MenuCommand: QAccessible_Event = 24;
pub const QAccessible_Event_ActionChanged: QAccessible_Event = 257;
pub const QAccessible_Event_ActiveDescendantChanged: QAccessible_Event = 258;
pub const QAccessible_Event_AttributeChanged: QAccessible_Event = 259;
pub const QAccessible_Event_DocumentContentChanged: QAccessible_Event = 260;
pub const QAccessible_Event_DocumentLoadComplete: QAccessible_Event = 261;
pub const QAccessible_Event_DocumentLoadStopped: QAccessible_Event = 262;
pub const QAccessible_Event_DocumentReload: QAccessible_Event = 263;
pub const QAccessible_Event_HyperlinkEndIndexChanged: QAccessible_Event = 264;
pub const QAccessible_Event_HyperlinkNumberOfAnchorsChanged: QAccessible_Event = 265;
pub const QAccessible_Event_HyperlinkSelectedLinkChanged: QAccessible_Event = 266;
pub const QAccessible_Event_HypertextLinkActivated: QAccessible_Event = 267;
pub const QAccessible_Event_HypertextLinkSelected: QAccessible_Event = 268;
pub const QAccessible_Event_HyperlinkStartIndexChanged: QAccessible_Event = 269;
pub const QAccessible_Event_HypertextChanged: QAccessible_Event = 270;
pub const QAccessible_Event_HypertextNLinksChanged: QAccessible_Event = 271;
pub const QAccessible_Event_ObjectAttributeChanged: QAccessible_Event = 272;
pub const QAccessible_Event_PageChanged: QAccessible_Event = 273;
pub const QAccessible_Event_SectionChanged: QAccessible_Event = 274;
pub const QAccessible_Event_TableCaptionChanged: QAccessible_Event = 275;
pub const QAccessible_Event_TableColumnDescriptionChanged: QAccessible_Event = 276;
pub const QAccessible_Event_TableColumnHeaderChanged: QAccessible_Event = 277;
pub const QAccessible_Event_TableModelChanged: QAccessible_Event = 278;
pub const QAccessible_Event_TableRowDescriptionChanged: QAccessible_Event = 279;
pub const QAccessible_Event_TableRowHeaderChanged: QAccessible_Event = 280;
pub const QAccessible_Event_TableSummaryChanged: QAccessible_Event = 281;
pub const QAccessible_Event_TextAttributeChanged: QAccessible_Event = 282;
pub const QAccessible_Event_TextCaretMoved: QAccessible_Event = 283;
pub const QAccessible_Event_TextColumnChanged: QAccessible_Event = 285;
pub const QAccessible_Event_TextInserted: QAccessible_Event = 286;
pub const QAccessible_Event_TextRemoved: QAccessible_Event = 287;
pub const QAccessible_Event_TextUpdated: QAccessible_Event = 288;
pub const QAccessible_Event_TextSelectionChanged: QAccessible_Event = 289;
pub const QAccessible_Event_VisibleDataChanged: QAccessible_Event = 290;
pub const QAccessible_Event_ObjectCreated: QAccessible_Event = 32768;
pub const QAccessible_Event_ObjectDestroyed: QAccessible_Event = 32769;
pub const QAccessible_Event_ObjectShow: QAccessible_Event = 32770;
pub const QAccessible_Event_ObjectHide: QAccessible_Event = 32771;
pub const QAccessible_Event_ObjectReorder: QAccessible_Event = 32772;
pub const QAccessible_Event_Focus: QAccessible_Event = 32773;
pub const QAccessible_Event_Selection: QAccessible_Event = 32774;
pub const QAccessible_Event_SelectionAdd: QAccessible_Event = 32775;
pub const QAccessible_Event_SelectionRemove: QAccessible_Event = 32776;
pub const QAccessible_Event_SelectionWithin: QAccessible_Event = 32777;
pub const QAccessible_Event_StateChanged: QAccessible_Event = 32778;
pub const QAccessible_Event_LocationChanged: QAccessible_Event = 32779;
pub const QAccessible_Event_NameChanged: QAccessible_Event = 32780;
pub const QAccessible_Event_DescriptionChanged: QAccessible_Event = 32781;
pub const QAccessible_Event_ValueChanged: QAccessible_Event = 32782;
pub const QAccessible_Event_ParentChanged: QAccessible_Event = 32783;
pub const QAccessible_Event_HelpChanged: QAccessible_Event = 32928;
pub const QAccessible_Event_DefaultActionChanged: QAccessible_Event = 32944;
pub const QAccessible_Event_AcceleratorChanged: QAccessible_Event = 32960;
pub const QAccessible_Event_InvalidEvent: QAccessible_Event = 32961;
pub type QAccessible_Event = ::std::os::raw::c_uint;
pub const QAccessible_Role_NoRole: QAccessible_Role = 0;
pub const QAccessible_Role_TitleBar: QAccessible_Role = 1;
pub const QAccessible_Role_MenuBar: QAccessible_Role = 2;
pub const QAccessible_Role_ScrollBar: QAccessible_Role = 3;
pub const QAccessible_Role_Grip: QAccessible_Role = 4;
pub const QAccessible_Role_Sound: QAccessible_Role = 5;
pub const QAccessible_Role_Cursor: QAccessible_Role = 6;
pub const QAccessible_Role_Caret: QAccessible_Role = 7;
pub const QAccessible_Role_AlertMessage: QAccessible_Role = 8;
pub const QAccessible_Role_Window: QAccessible_Role = 9;
pub const QAccessible_Role_Client: QAccessible_Role = 10;
pub const QAccessible_Role_PopupMenu: QAccessible_Role = 11;
pub const QAccessible_Role_MenuItem: QAccessible_Role = 12;
pub const QAccessible_Role_ToolTip: QAccessible_Role = 13;
pub const QAccessible_Role_Application: QAccessible_Role = 14;
pub const QAccessible_Role_Document: QAccessible_Role = 15;
pub const QAccessible_Role_Pane: QAccessible_Role = 16;
pub const QAccessible_Role_Chart: QAccessible_Role = 17;
pub const QAccessible_Role_Dialog: QAccessible_Role = 18;
pub const QAccessible_Role_Border: QAccessible_Role = 19;
pub const QAccessible_Role_Grouping: QAccessible_Role = 20;
pub const QAccessible_Role_Separator: QAccessible_Role = 21;
pub const QAccessible_Role_ToolBar: QAccessible_Role = 22;
pub const QAccessible_Role_StatusBar: QAccessible_Role = 23;
pub const QAccessible_Role_Table: QAccessible_Role = 24;
pub const QAccessible_Role_ColumnHeader: QAccessible_Role = 25;
pub const QAccessible_Role_RowHeader: QAccessible_Role = 26;
pub const QAccessible_Role_Column: QAccessible_Role = 27;
pub const QAccessible_Role_Row: QAccessible_Role = 28;
pub const QAccessible_Role_Cell: QAccessible_Role = 29;
pub const QAccessible_Role_Link: QAccessible_Role = 30;
pub const QAccessible_Role_HelpBalloon: QAccessible_Role = 31;
pub const QAccessible_Role_Assistant: QAccessible_Role = 32;
pub const QAccessible_Role_List: QAccessible_Role = 33;
pub const QAccessible_Role_ListItem: QAccessible_Role = 34;
pub const QAccessible_Role_Tree: QAccessible_Role = 35;
pub const QAccessible_Role_TreeItem: QAccessible_Role = 36;
pub const QAccessible_Role_PageTab: QAccessible_Role = 37;
pub const QAccessible_Role_PropertyPage: QAccessible_Role = 38;
pub const QAccessible_Role_Indicator: QAccessible_Role = 39;
pub const QAccessible_Role_Graphic: QAccessible_Role = 40;
pub const QAccessible_Role_StaticText: QAccessible_Role = 41;
pub const QAccessible_Role_EditableText: QAccessible_Role = 42;
pub const QAccessible_Role_Button: QAccessible_Role = 43;
pub const QAccessible_Role_PushButton: QAccessible_Role = 43;
pub const QAccessible_Role_CheckBox: QAccessible_Role = 44;
pub const QAccessible_Role_RadioButton: QAccessible_Role = 45;
pub const QAccessible_Role_ComboBox: QAccessible_Role = 46;
pub const QAccessible_Role_ProgressBar: QAccessible_Role = 48;
pub const QAccessible_Role_Dial: QAccessible_Role = 49;
pub const QAccessible_Role_HotkeyField: QAccessible_Role = 50;
pub const QAccessible_Role_Slider: QAccessible_Role = 51;
pub const QAccessible_Role_SpinBox: QAccessible_Role = 52;
pub const QAccessible_Role_Canvas: QAccessible_Role = 53;
pub const QAccessible_Role_Animation: QAccessible_Role = 54;
pub const QAccessible_Role_Equation: QAccessible_Role = 55;
pub const QAccessible_Role_ButtonDropDown: QAccessible_Role = 56;
pub const QAccessible_Role_ButtonMenu: QAccessible_Role = 57;
pub const QAccessible_Role_ButtonDropGrid: QAccessible_Role = 58;
pub const QAccessible_Role_Whitespace: QAccessible_Role = 59;
pub const QAccessible_Role_PageTabList: QAccessible_Role = 60;
pub const QAccessible_Role_Clock: QAccessible_Role = 61;
pub const QAccessible_Role_Splitter: QAccessible_Role = 62;
pub const QAccessible_Role_LayeredPane: QAccessible_Role = 128;
pub const QAccessible_Role_Terminal: QAccessible_Role = 129;
pub const QAccessible_Role_Desktop: QAccessible_Role = 130;
pub const QAccessible_Role_Paragraph: QAccessible_Role = 131;
pub const QAccessible_Role_WebDocument: QAccessible_Role = 132;
pub const QAccessible_Role_Section: QAccessible_Role = 133;
pub const QAccessible_Role_Notification: QAccessible_Role = 134;
pub const QAccessible_Role_ColorChooser: QAccessible_Role = 1028;
pub const QAccessible_Role_Footer: QAccessible_Role = 1038;
pub const QAccessible_Role_Form: QAccessible_Role = 1040;
pub const QAccessible_Role_Heading: QAccessible_Role = 1044;
pub const QAccessible_Role_Note: QAccessible_Role = 1051;
pub const QAccessible_Role_ComplementaryContent: QAccessible_Role = 1068;
pub const QAccessible_Role_UserRole: QAccessible_Role = 65535;
pub type QAccessible_Role = ::std::os::raw::c_uint;
pub const QAccessible_Text_Name: QAccessible_Text = 0;
pub const QAccessible_Text_Description: QAccessible_Text = 1;
pub const QAccessible_Text_Value: QAccessible_Text = 2;
pub const QAccessible_Text_Help: QAccessible_Text = 3;
pub const QAccessible_Text_Accelerator: QAccessible_Text = 4;
pub const QAccessible_Text_DebugDescription: QAccessible_Text = 5;
pub const QAccessible_Text_UserText: QAccessible_Text = 65535;
pub type QAccessible_Text = ::std::os::raw::c_uint;
