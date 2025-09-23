## Technical Preview -> 1.0 migration guide

This document summarizes all changes made to the Material Components between the Technical Preview and the 1.0 release.

## Major System-Wide Changes

### Styling System Renaming
- **`Animation` → `MaterialAnimation`**
  - All animation imports now use `MaterialAnimation` instead of `Animation`
  - Affects: bottom-sheet, check-box, chip, dialog, drawer, extended-touch-area, modal, navigation-drawer, radio-button, search-bar, segmented-button, snack-bar, state-layer, switch, tab-bar, text-field

- **`typography` → `MaterialTypography`**
  - Typography system renamed for consistency
  - Affects: app-bar, badge, base-button, chip, date-picker, dialog, drawer, elevated-button, filled-button, filled-icon-button, floating-action-button, icon-button, list, material-text, menu, navigation-bar, navigation-drawer, outline-button, outline-icon-button, search-bar, snack-bar, tab-bar, text-button, text-field, time-picker, tonal-button, tonal-icon-button, tooltip

## Component Changes

### Navigation Components

#### Navigation Drawer
- **New**: ModalNavigationDrawer documentation added
- **API Changes**:
  - `leading_clicked` → `leading_button_clicked`
  - `trailing_clicked` → `trailing_button_clicked`
  - `leading_icon` → `leading_button`
  - `trailing_icon` → `trailing_button`
  - `current_item` → `current_index`
- **New Features**: Added `index_changed` callback

#### Navigation Bar
- **API Changes**: Same navigation-related changes as Navigation Drawer

### Form Components

#### Check-Box
- **Component Rename**: `CheckBoxListTile` → `CheckBoxTile`
- **Various fixes** implemented

#### Radio-Button
- **New Components**: Added RadioButton and RadioButtonTile

#### Progress-Indicator
- **API Change**: `value` property → `progress` property

#### Slider
- **New Feature**: Added `released` callback
- **Improvements**: Various slider enhancements

#### Segmented-Button
- **API Changes**:
  - `model` → `items`
  - `icon-selected` → `selected-icon`
- **New Feature**: Added `index-changed` callback

### Input Components

#### Text-Field
- **API Changes**:
  - `placeholder` → `placeholder-text`

#### Time-Picker
- **Component Rename**: `TimePicker` → `TimePickerPopup`
- **API Changes**:
  - `twelf_hour_model` → `twelfth_hour_model` (typo fix)
  - `current_item` → `current_index`
  - `hour_labl` → `hour_label` (typo fix)
- **New Properties**: Added various time picker properties (radius, picker_diameter, center, outer_padding, inner_padding, etc.)
- **New Properties**: Added `use_24_hour_format`, `title`, `cancel_text` properties

#### Date-Picker
- **New Component**: Added DatePickerPopup

### Button Components

#### All Button Types
- **API Changes**: Same typography and animation system updates as listed above

### Layout Components

#### List
- **API Changes**:
  - `action-icon` → `action-button-icon`

#### Badge
- **API Changes**:
  - `empty-badge` → `show-badge`

### Menu Components

#### Menu
- **New Component**: Added Menu component
- **API Changes**:
  - `item_clicked` → `activated` callback
  - Component rename: `Menu` → `PopupMenu`

#### Drop-Down-Menu
- **New Component**: Added DropDownMenu
- **API Changes**:
  - Added `selected` callback

### Dialog Components

#### Dialog & SnackBar
- **Multi-rename changes**

#### Action-Chip & Search-Bar
- **API Changes**:
  - `avatar` → `avatar-icon`

### Visual Components

#### Check-Box (Icon Properties)
- **API Changes**:
  - `icon-checked` → `checked-icon`

#### Various Components
- **API Changes**:
  - `container-background` → `show-background`

## New Components Added

1. **Radio-Button** and **RadioButtonTile**
2. **Menu** component
3. **Drop-Down-Menu** component
4. **Date-Picker-Popup** component
5. **Modal-Navigation-Drawer** documentation


