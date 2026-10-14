// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The builtin functions of the .slint language.

/// Calls `$macro!` with the declaration of every builtin function.
///
/// An entry declares one builtin with:
///
/// - The `BuiltinFunction` variant, and its payload if any: `Name:` or `Name(Orientation):`.
///   Doc comments and attributes before it go to the variant.
/// - `pure fn` or `impure fn`, depending on whether the call has side effects.
/// - The named parameters and the return type.
///   The LSP shows these names.
/// - Types that are Rust types, or the name of a Slint type without a Rust equivalent,
///   such as `ElementReference`.
///   `[T]` is an array, and `()` is no value.
/// - `RustType => SlintType` when the Slint type isn't the one of the Rust type,
///   such as `f32 => Angle`.
/// - Optionally, a name and a body that implement the builtin.
///   The body is compiled in `i_slint_core::builtin_functions`.
///   A first parameter `&context` gives it the component's `SlintContext`.
///   A parameter `name: ItemArg<Item> => ElementReference`, where `Item` is the type of the
///   element's native item, gives it the item and its `ItemRc`.
///   `name: ItemRc => ElementReference` gives it the `ItemRc` of an element of any type.
///   Attributes after the colon, such as `#[cfg(feature = "path")]`, only apply to that function.
///
/// ```text
/// Round: pure fn round(x: f32) -> i32 { x.round() as i32 }
/// ToFixed: pure fn to_fixed(&context, n: f64, digits: i32) -> SharedString { … }
/// ShowPopupWindow: impure fn(popup: ElementReference) -> ()
/// HasSelection: pure fn has_selection(text_input: ItemArg<TextInput> => ElementReference) -> bool { … }
/// ```
// The `runtime` functions refer to `i_slint_core` with `crate`
#[allow(clippy::crate_in_macro_def)]
#[macro_export]
macro_rules! for_each_builtin_function {
    ($macro:ident) => {
        $macro! {
            GetWindowScaleFactor: pure fn() -> f32 => ScaleFactor
            GetWindowDefaultFontSize: pure fn() -> Coord
            AnimationTick: pure fn() -> i64 => Duration
            // Has a side effect, but is still considered pure
            Debug: pure fn(message: SharedString) -> ()
            Mod: pure fn modulo(a: f32, b: f32) -> f32 { Euclid::rem_euclid(&a, &b) }
            Round: pure fn round(x: f32) -> i32 { x.round() as i32 }
            Ceil: pure fn ceil(x: f32) -> i32 { x.ceil() as i32 }
            Floor: pure fn floor(x: f32) -> i32 { x.floor() as i32 }
            Abs: pure fn abs(x: f32) -> f32 { x.abs() }
            Sqrt: pure fn sqrt(x: f32) -> f32 { x.sqrt() }
            Cos: pure fn cos(degrees: f32 => Angle) -> f32 { degrees.to_radians().cos() }
            Sin: pure fn sin(degrees: f32 => Angle) -> f32 { degrees.to_radians().sin() }
            Tan: pure fn tan(degrees: f32 => Angle) -> f32 { degrees.to_radians().tan() }
            ACos: pure fn acos(x: f32) -> f32 => Angle { x.acos().to_degrees() }
            ASin: pure fn asin(x: f32) -> f32 => Angle { x.asin().to_degrees() }
            ATan: pure fn atan(x: f32) -> f32 => Angle { x.atan().to_degrees() }
            ATan2: pure fn atan2(y: f32, x: f32) -> f32 => Angle { y.atan2(x).to_degrees() }
            Log: pure fn log(x: f32, base: f32) -> f32 { x.log(base) }
            Ln: pure fn ln(x: f32) -> f32 { x.ln() }
            Pow: pure fn pow(x: f32, y: f32) -> f32 { x.powf(y) }
            Exp: pure fn exp(x: f32) -> f32 { x.exp() }
            ToFixed: pure fn to_fixed(&context, n: f64, digits: i32) -> SharedString {
                context.format_number_fixed(n, digits.max(0) as usize)
            }
            ToPrecision: pure fn to_precision(&context, n: f64, digits: i32) -> SharedString {
                context.format_number_precision(n, digits.max(0) as usize)
            }
            ToStringUnlocalized: pure fn to_string_unlocalized(n: f64) -> SharedString { crate::string::shared_string_from_number_unlocalized(n) }
            SetFocusItem: impure fn(element: ElementReference) -> ()
            ClearFocusItem: impure fn(element: ElementReference) -> ()
            ScrollTo: impure fn scroll_to(flickable: ItemArg<Flickable> => ElementReference, pos: LogicalPosition, behavior: ScrollBehavior) -> () {
                flickable.item.scroll_to(&flickable.rc, pos, behavior)
            }
            EnsureVisible: impure fn ensure_visible(element: ItemRc => ElementReference, behavior: ScrollBehavior) -> () {
                element.try_scroll_into_visible(behavior)
            }
            ShowPopupWindow: impure fn(popup: ElementReference) -> ()
            ClosePopupWindow: impure fn(popup: ElementReference) -> ()
            /// Show a context popup menu.
            /// Arguments are `(parent, menu_tree, position)`
            ///
            /// The first argument (parent) is a reference to the `ContextMenu` native item
            /// The second argument (menu_tree) is a ElementReference to the root of the tree,
            /// and in the LLR, a NumberLiteral to an index in  [`crate::llr::SubComponent::menu_item_trees`]
            ShowPopupMenu: impure fn(parent: ElementReference, menu: ElementReference, position: LogicalPosition) -> ()
            /// Show a context popup menu from a list of entries.
            /// Arguments are `(parent, entries, position)`
            /// The entries argument is an array of MenuEntry
            ShowPopupMenuInternal: impure fn(parent: ElementReference, entries: Model, position: LogicalPosition) -> ()
            SetSelectionOffsets: impure fn set_selection_offsets(text_input: ItemArg<TextInput> => ElementReference, anchor: i32, focus: i32) -> () {
                with_window_adapter(&text_input.rc, |w| text_input.item.set_selection_offsets(w, &text_input.rc, anchor, focus))
            }
            TextInputSelectAll: impure fn text_input_select_all(text_input: ItemArg<TextInput> => ElementReference) -> () {
                with_window_adapter(&text_input.rc, |w| text_input.item.select_all(w, &text_input.rc))
            }
            TextInputClearSelection: impure fn text_input_clear_selection(text_input: ItemArg<TextInput> => ElementReference) -> () {
                text_input.item.clear_selection()
            }
            TextInputCut: impure fn text_input_cut(text_input: ItemArg<TextInput> => ElementReference) -> () {
                with_window_adapter(&text_input.rc, |w| text_input.item.cut(w, &text_input.rc))
            }
            TextInputCopy: impure fn text_input_copy(text_input: ItemArg<TextInput> => ElementReference) -> () {
                with_window_adapter(&text_input.rc, |w| text_input.item.copy(w))
            }
            TextInputPaste: impure fn text_input_paste(text_input: ItemArg<TextInput> => ElementReference) -> () {
                with_window_adapter(&text_input.rc, |w| text_input.item.paste(w, &text_input.rc))
            }
            TextInputUndo: impure fn text_input_undo(text_input: ItemArg<TextInput> => ElementReference) -> () {
                with_window_adapter(&text_input.rc, |w| text_input.item.undo(w, &text_input.rc))
            }
            TextInputRedo: impure fn text_input_redo(text_input: ItemArg<TextInput> => ElementReference) -> () {
                with_window_adapter(&text_input.rc, |w| text_input.item.redo(w, &text_input.rc))
            }
            HasSelection: pure fn has_selection(text_input: ItemArg<TextInput> => ElementReference) -> bool {
                text_input.item.has_selection()
            }
            StyledTextSetSelectionOffsets: impure fn styled_text_set_selection_offsets(styled_text: ItemArg<StyledTextItem> => ElementReference, anchor: i32, focus: i32) -> () {
                with_window_adapter(&styled_text.rc, |w| styled_text.item.set_selection_offsets(w, &styled_text.rc, anchor, focus))
            }
            StyledTextSelectAll: impure fn styled_text_select_all(styled_text: ItemArg<StyledTextItem> => ElementReference) -> () {
                with_window_adapter(&styled_text.rc, |w| styled_text.item.select_all(w, &styled_text.rc))
            }
            StyledTextClearSelection: impure fn styled_text_clear_selection(styled_text: ItemArg<StyledTextItem> => ElementReference) -> () {
                with_window_adapter(&styled_text.rc, |w| styled_text.item.clear_selection(w, &styled_text.rc))
            }
            StyledTextCopy: impure fn styled_text_copy(styled_text: ItemArg<StyledTextItem> => ElementReference) -> () {
                with_window_adapter(&styled_text.rc, |w| styled_text.item.copy(w, &styled_text.rc))
            }
            StyledTextHasSelection: pure fn styled_text_has_selection(styled_text: ItemArg<StyledTextItem> => ElementReference) -> bool {
                styled_text.item.has_selection()
            }
            ItemFontMetrics: pure fn item_font_metrics(element: ItemRc => ElementReference) -> FontMetrics {
                with_window_adapter(&element, |w| crate::items::text_item_font_metrics(w, &element))
            }
            /// the "42".to_float()
            StringToFloat: pure fn string_to_float(&context, s: SharedString) -> f32 {
                context.parse_number(&s).unwrap_or_default()
            }
            /// the "42".is_float()
            StringIsFloat: pure fn string_is_float(&context, s: SharedString) -> bool { context.parse_number(&s).is_some() }
            /// the "42".is_empty
            StringIsEmpty: pure fn string_is_empty(s: SharedString) -> bool { s.is_empty() }
            /// the "42".length
            StringCharacterCount: pure fn string_character_count(s: SharedString) -> i32 {
                unicode_segmentation::UnicodeSegmentation::graphemes(s.as_str(), true).count() as i32
            }
            StringToLowercase: pure fn string_to_lowercase(s: SharedString) -> SharedString { s.to_lowercase().into() }
            StringToUppercase: pure fn string_to_uppercase(s: SharedString) -> SharedString { s.to_uppercase().into() }
            StringStartsWith: pure fn string_starts_with(s: SharedString, prefix: SharedString) -> bool { s.starts_with(prefix.as_str()) }
            StringEndsWith: pure fn string_ends_with(s: SharedString, suffix: SharedString) -> bool { s.ends_with(suffix.as_str()) }
            StringReplaceAll: pure fn string_replace_all(s: SharedString, from: SharedString, to: SharedString) -> SharedString {
                crate::string::shared_string_replace_all(&s, &from, &to)
            }
            KeysToString: pure fn keys_to_string(keys: Keys) -> SharedString { keys.to_shared_string() }
            ColorRgbaStruct: pure fn color_rgba(color: Color) -> RgbaColorU8 { color.to_argb_u8() }
            ColorHsvaStruct: pure fn color_hsva(color: Color) -> HsvaColor { color.to_hsva() }
            ColorOklchStruct: pure fn color_oklch(color: Color) -> OklchColor { color.to_oklch() }
            ColorBrighter: pure fn color_brighter(brush: Brush, factor: f32) -> Brush { brush.brighter(factor) }
            ColorDarker: pure fn color_darker(brush: Brush, factor: f32) -> Brush { brush.darker(factor) }
            ColorTransparentize: pure fn color_transparentize(brush: Brush, factor: f32) -> Brush { brush.transparentize(factor) }
            ColorMix: pure fn color_mix(color: Color, other: Color, factor: f32) -> Color { color.mix(&other, factor) }
            ColorWithAlpha: pure fn color_with_alpha(brush: Brush, alpha: f32) -> Brush { brush.with_alpha(alpha) }
            ImageSize: pure fn(image: Image) -> Size
            ArrayLength: pure fn(array: Model) -> i32
            // Using Type::InferredProperty as there is currently no valid type for the data argument.
            ArrayPush: impure fn(array: Model, value: InferredProperty) -> ()
            ArrayRemove: impure fn(array: Model, index: i32) -> ()
            ArrayInsert: impure fn(array: Model, index: i32, value: InferredProperty) -> ()
            ArrayAny: pure fn(array: Model, predicate: Closure) -> bool
            ArrayAll: pure fn(array: Model, predicate: Closure) -> bool
            ArrayFindIndex: pure fn(array: Model, predicate: Closure) -> i32
            Rgb: pure fn rgb(r: i32, g: i32, b: i32, a: f32) -> Color {
                let channel = |c: i32| c.clamp(0, 255) as u8;
                // `as u8` saturates the alpha
                Color::from_argb_u8((255. * a) as u8, channel(r), channel(g), channel(b))
            }
            Hsv: pure fn hsv(h: f32, s: f32, v: f32, a: f32) -> Color {
                Color::from_hsva(h, s.clamp(0., 1.), v.clamp(0., 1.), a.clamp(0., 1.))
            }
            Oklch: pure fn oklch(l: f32, c: f32, h: f32, a: f32) -> Color {
                Color::from_oklch(l.clamp(0., 1.), c.max(0.), h, a.clamp(0., 1.))
            }
            ColorScheme: pure fn() -> ColorScheme
            AccentColor: pure fn accent_color(&context) -> Color { context.accent_color() }
            SupportsNativeMenuBar: pure fn() -> bool
            /// Setup the menu bar
            ///
            /// arguments are: `(ref entries, ref sub-menu, ref activated, item_tree_root, no_native_menu_bar, <condition>)`
            /// `item_tree_root` is a reference to the MenuItem tree root (just like in the [`Self::ShowPopupMenu`] call).
            /// `no_native_menu_bar` is a boolean literal that is true when we shouldn't try to setup the native menu bar.
            /// `condition` is an optional expression that is the expression to `if condition : MenuBar { ... }` for optional menu
            // The types are not accurate.
            SetupMenuBar: impure fn(entries: Model, sub_menu: NoArgCallback, activated: NoArgCallback) -> ()
            /// Setup the menu of a `SystemTrayIcon`.
            ///
            /// Arguments are `(system_tray_ref, menu_tree_root)` where `menu_tree_root` is an ElementReference to the root of
            /// the MenuItem tree, lowered to a NumberLiteral index into [`crate::llr::SubComponent::menu_item_trees`] in the LLR.
            SetupSystemTrayIcon: impure fn(system_tray: ElementReference, menu: ElementReference) -> ()
            Use24HourFormat: pure fn use_24_hour_format() -> bool { date_time::use_24_hour_format() }
            MonthDayCount: pure fn month_day_count(month: i32, year: i32) -> i32 {
                date_time::month_day_count(month as u32, year).unwrap_or(0)
            }
            MonthOffset: pure fn month_offset(month: i32, year: i32) -> i32 { date_time::month_offset(month as u32, year) }
            FormatDate: pure fn format_date(format: SharedString, day: i32, month: i32, year: i32) -> SharedString {
                date_time::format_date(&format, day as u32, month as u32, year)
            }
            DateNow: pure fn() -> [i32]
            ValidDate: pure fn valid_date(date: SharedString, format: SharedString) -> bool { date_time::parse_date(&date, &format).is_some() }
            ParseDate: pure fn(date: SharedString, format: SharedString) -> [i32]
            TextInputFocused: pure fn() -> bool
            SetTextInputFocused: impure fn(focused: bool) -> ()
            #[strum(disabled)]
            ImplicitLayoutInfo(Orientation): pure fn(element: ElementReference, constraint: f32) -> LayoutInfo
            SwipeGestureHandlerCancel: impure fn swipe_gesture_handler_cancel(handler: ItemArg<SwipeGestureHandler> => ElementReference) -> () {
                handler.item.cancel()
            }
            ContextMenuClose: impure fn context_menu_close(menu: ItemArg<ContextMenu> => ElementReference) -> () {
                with_window_adapter(&menu.rc, |w| menu.item.close(w, &menu.rc))
            }
            ContextMenuIsOpen: pure fn context_menu_is_open(menu: ItemArg<ContextMenu> => ElementReference) -> bool {
                with_window_adapter(&menu.rc, |w| menu.item.is_open(w))
            }
            WindowClose: impure fn window_close(window: ItemArg<WindowItem> => ElementReference) -> bool {
                with_window_adapter(&window.rc, |w| window.item.close(w, &window.rc))
            }
            WindowHide: impure fn window_hide(window: ItemArg<WindowItem> => ElementReference) -> () {
                with_window_adapter(&window.rc, |w| window.item.hide(w, &window.rc))
            }
            ItemAbsolutePosition: pure fn item_absolute_position(element: ItemRc => ElementReference) -> LogicalPosition {
                logical_position_to_api(element.map_to_window(element.geometry().origin))
            }
            RegisterCustomFontByPath: impure fn(path: SharedString) -> ()
            RegisterCustomFontByMemory: impure fn(resource: i32) -> ()
            RegisterBitmapFont: impure fn(resource: i32) -> ()
            Translate: pure fn(original: SharedString, context: SharedString, domain: SharedString, arguments: [SharedString]) -> SharedString
            UpdateTimers: impure fn() -> ()
            DetectOperatingSystem: pure fn detect_operating_system() -> OperatingSystemType { crate::detect_operating_system() }
            StartTimer: impure fn(timer: ElementReference) -> ()
            StopTimer: impure fn(timer: ElementReference) -> ()
            RestartTimer: impure fn(timer: ElementReference) -> ()
            OpenUrl: impure fn open_url(&context, url: SharedString) -> bool { crate::open_url(&url, context).is_ok() }
            MacosBringAllWindowsToFront: impure fn() -> ()
            ParseMarkdown: pure fn(format: SharedString, arguments: [StyledText]) -> StyledText
            StringToStyledText: pure fn string_to_styled_text(s: SharedString) -> StyledText { crate::styled_text::string_to_styled_text(s.into()) }
            /// Converts a color to a hex string wrapped in StyledText.
            /// Used for `<font color='\{expr}'>` interpolation in `@markdown`,
            /// because `parse_interpolated` takes `StyledText` arguments.
            ColorToStyledText: pure fn color_to_styled_text(color: Color) -> StyledText { crate::styled_text::color_to_styled_text(color) }
            DecimalSeparator: pure fn decimal_separator(&context) -> SharedString { context.locale_decimal_separator().into() }
            /// The window title when the application doesn't set one, see
            /// `i_slint_core::window::default_window_title`
            DefaultWindowTitle: pure fn default_window_title() -> SharedString { crate::window::default_window_title() }
            PathPointAt: #[cfg(feature = "path")] pure fn path_point_at(path: ItemArg<Path> => ElementReference, t: f32) -> LogicalPosition {
                let point = path.item.point_at(&path.rc, t);
                crate::api::LogicalPosition::new(point.x, point.y)
            }
            PathAngleAt: #[cfg(feature = "path")] pure fn path_angle_at(path: ItemArg<Path> => ElementReference, t: f32) -> f32 => Angle {
                path.item.angle_at(&path.rc, t)
            }
        }
    };
}
