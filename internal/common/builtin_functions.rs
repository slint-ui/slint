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
/// - The parameter types and the return type.
/// - Types that are Rust types, or the name of a Slint type without a Rust equivalent,
///   such as `ElementReference`.
///   `[T]` is an array, and `()` is no value.
/// - `RustType => SlintType` when the Slint type isn't the one of the Rust type,
///   such as `f64 => Angle`.
/// - Optionally, a name, parameter names, and a body that implement the builtin,
///   when its result only depends on its arguments.
///   The body is compiled in `i_slint_core::builtin_functions`.
///   A first parameter `&context` gives it the component's `SlintContext`.
///
/// ```text
/// Round: pure fn round(x: f64) -> f64 => Int32 { x.round() }
/// ToFixed: pure fn to_fixed(&context, n: f64, digits: f64 => Int32) -> SharedString { … }
/// ShowPopupWindow: impure fn(ElementReference) -> ()
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
            Debug: pure fn(SharedString) -> ()
            Mod: pure fn modulo(a: f64 => Int32, b: f64 => Int32) -> f64 => Int32 { Euclid::rem_euclid(&a, &b) }
            Round: pure fn round(x: f64) -> f64 => Int32 { x.round() }
            Ceil: pure fn ceil(x: f64) -> f64 => Int32 { x.ceil() }
            Floor: pure fn floor(x: f64) -> f64 => Int32 { x.floor() }
            Abs: pure fn abs(x: f64) -> f64 { x.abs() }
            Sqrt: pure fn sqrt(x: f64) -> f64 { x.sqrt() }
            Cos: pure fn cos(degrees: f64 => Angle) -> f64 { degrees.to_radians().cos() }
            Sin: pure fn sin(degrees: f64 => Angle) -> f64 { degrees.to_radians().sin() }
            Tan: pure fn tan(degrees: f64 => Angle) -> f64 { degrees.to_radians().tan() }
            ACos: pure fn acos(x: f64) -> f64 => Angle { x.acos().to_degrees() }
            ASin: pure fn asin(x: f64) -> f64 => Angle { x.asin().to_degrees() }
            ATan: pure fn atan(x: f64) -> f64 => Angle { x.atan().to_degrees() }
            ATan2: pure fn atan2(y: f64, x: f64) -> f64 => Angle { y.atan2(x).to_degrees() }
            Log: pure fn log(x: f64, base: f64) -> f64 { x.log(base) }
            Ln: pure fn ln(x: f64) -> f64 { x.ln() }
            Pow: pure fn pow(x: f64, y: f64) -> f64 { x.powf(y) }
            Exp: pure fn exp(x: f64) -> f64 { x.exp() }
            ToFixed: pure fn to_fixed(&context, n: f64, digits: f64 => Int32) -> SharedString {
                context.format_number_fixed(n, digits.max(0.) as usize)
            }
            ToPrecision: pure fn to_precision(&context, n: f64, precision: f64 => Int32) -> SharedString {
                context.format_number_precision(n, precision.max(0.) as usize)
            }
            ToStringUnlocalized: pure fn to_string_unlocalized(n: f64) -> SharedString { crate::string::shared_string_from_number_unlocalized(n) }
            SetFocusItem: impure fn(ElementReference) -> ()
            ClearFocusItem: impure fn(ElementReference) -> ()
            ShowPopupWindow: impure fn(ElementReference) -> ()
            ClosePopupWindow: impure fn(ElementReference) -> ()
            /// Show a context popup menu.
            /// Arguments are `(parent, menu_tree, position)`
            ///
            /// The first argument (parent) is a reference to the `ContextMenu` native item
            /// The second argument (menu_tree) is a ElementReference to the root of the tree,
            /// and in the LLR, a NumberLiteral to an index in  [`crate::llr::SubComponent::menu_item_trees`]
            ShowPopupMenu: impure fn(ElementReference, ElementReference, LogicalPosition) -> ()
            /// Show a context popup menu from a list of entries.
            /// Arguments are `(parent, entries, position)`
            /// The entries argument is an array of MenuEntry
            ShowPopupMenuInternal: impure fn(ElementReference, Model, LogicalPosition) -> ()
            SetSelectionOffsets: impure fn(ElementReference, i32, i32) -> ()
            HasSelection: pure fn(ElementReference) -> bool
            ItemFontMetrics: pure fn(ElementReference) -> FontMetrics
            /// the "42".to_float()
            StringToFloat: pure fn string_to_float(&context, s: SharedString) -> f64 {
                context.parse_number(&s).unwrap_or_default() as f64
            }
            /// the "42".is_float()
            StringIsFloat: pure fn string_is_float(&context, s: SharedString) -> bool { context.parse_number(&s).is_some() }
            /// the "42".is_empty
            StringIsEmpty: pure fn string_is_empty(s: SharedString) -> bool { s.is_empty() }
            /// the "42".length
            StringCharacterCount: pure fn string_character_count(s: SharedString) -> f64 => Int32 {
                unicode_segmentation::UnicodeSegmentation::graphemes(s.as_str(), true).count() as f64
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
            ColorBrighter: pure fn color_brighter(brush: Brush, factor: f64) -> Brush { brush.brighter(factor as f32) }
            ColorDarker: pure fn color_darker(brush: Brush, factor: f64) -> Brush { brush.darker(factor as f32) }
            ColorTransparentize: pure fn color_transparentize(brush: Brush, factor: f64) -> Brush { brush.transparentize(factor as f32) }
            ColorMix: pure fn color_mix(a: Color, b: Color, factor: f64) -> Color { a.mix(&b, factor as f32) }
            ColorWithAlpha: pure fn color_with_alpha(brush: Brush, alpha: f64) -> Brush { brush.with_alpha(alpha as f32) }
            ImageSize: pure fn(Image) -> Size
            ArrayLength: pure fn(Model) -> i32
            // Using Type::InferredProperty as there is currently no valid type for the data argument.
            ArrayPush: impure fn(Model, InferredProperty) -> ()
            ArrayRemove: impure fn(Model, i32) -> ()
            ArrayInsert: impure fn(Model, i32, InferredProperty) -> ()
            ArrayAny: pure fn(Model, Closure) -> bool
            ArrayAll: pure fn(Model, Closure) -> bool
            ArrayFindIndex: pure fn(Model, Closure) -> i32
            Rgb: pure fn rgb(r: f64 => Int32, g: f64 => Int32, b: f64 => Int32, a: f64) -> Color {
                // `as u8` saturates
                Color::from_argb_u8((255. * a) as u8, r as u8, g as u8, b as u8)
            }
            Hsv: pure fn hsv(h: f64, s: f64, v: f64, a: f64) -> Color {
                Color::from_hsva(h as f32, s.clamp(0., 1.) as f32, v.clamp(0., 1.) as f32, a.clamp(0., 1.) as f32)
            }
            Oklch: pure fn oklch(l: f64, c: f64, h: f64, a: f64) -> Color {
                Color::from_oklch(l.clamp(0., 1.) as f32, c.max(0.) as f32, h as f32, a.clamp(0., 1.) as f32)
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
            SetupMenuBar: impure fn(Model, NoArgCallback, NoArgCallback) -> ()
            /// Setup the menu of a `SystemTrayIcon`.
            ///
            /// Arguments are `(system_tray_ref, menu_tree_root)` where `menu_tree_root` is an ElementReference to the root of
            /// the MenuItem tree, lowered to a NumberLiteral index into [`crate::llr::SubComponent::menu_item_trees`] in the LLR.
            SetupSystemTrayIcon: impure fn(ElementReference, ElementReference) -> ()
            Use24HourFormat: pure fn use_24_hour_format() -> bool { date_time::use_24_hour_format() }
            MonthDayCount: pure fn month_day_count(month: f64 => Int32, year: f64 => Int32) -> f64 => Int32 {
                date_time::month_day_count(month as u32, year as i32).unwrap_or(0) as f64
            }
            MonthOffset: pure fn month_offset(month: f64 => Int32, year: f64 => Int32) -> f64 => Int32 { date_time::month_offset(month as u32, year as i32) as f64 }
            FormatDate: pure fn format_date(format: SharedString, day: f64 => Int32, month: f64 => Int32, year: f64 => Int32) -> SharedString {
                date_time::format_date(&format, day as u32, month as u32, year as i32)
            }
            DateNow: pure fn() -> [i32]
            ValidDate: pure fn valid_date(date: SharedString, format: SharedString) -> bool { date_time::parse_date(&date, &format).is_some() }
            ParseDate: pure fn(SharedString, SharedString) -> [i32]
            TextInputFocused: pure fn() -> bool
            SetTextInputFocused: impure fn(bool) -> ()
            #[strum(disabled)]
            ImplicitLayoutInfo(Orientation): pure fn(ElementReference, f32) -> LayoutInfo
            ItemAbsolutePosition: pure fn(ElementReference) -> LogicalPosition
            RegisterCustomFontByPath: impure fn(SharedString) -> ()
            RegisterCustomFontByMemory: impure fn(i32) -> ()
            RegisterBitmapFont: impure fn(i32) -> ()
            // original, context, domain, args
            Translate: pure fn(SharedString, SharedString, SharedString, [SharedString]) -> SharedString
            UpdateTimers: impure fn() -> ()
            DetectOperatingSystem: pure fn detect_operating_system() -> OperatingSystemType { crate::detect_operating_system() }
            StartTimer: impure fn(ElementReference) -> ()
            StopTimer: impure fn(ElementReference) -> ()
            RestartTimer: impure fn(ElementReference) -> ()
            OpenUrl: impure fn open_url(&context, url: SharedString) -> bool { crate::open_url(&url, context).is_ok() }
            MacosBringAllWindowsToFront: impure fn() -> ()
            ParseMarkdown: pure fn(SharedString, [StyledText]) -> StyledText
            StringToStyledText: pure fn string_to_styled_text(s: SharedString) -> StyledText { crate::styled_text::string_to_styled_text(s.into()) }
            /// Converts a color to a hex string wrapped in StyledText.
            /// Used for `<font color='\{expr}'>` interpolation in `@markdown`,
            /// because `parse_interpolated` takes `StyledText` arguments.
            ColorToStyledText: pure fn color_to_styled_text(color: Color) -> StyledText { crate::styled_text::color_to_styled_text(color) }
            DecimalSeparator: pure fn decimal_separator(&context) -> SharedString { context.locale_decimal_separator().into() }
            /// The window title when the application doesn't set one, see
            /// `i_slint_core::window::default_window_title`
            DefaultWindowTitle: pure fn default_window_title() -> SharedString { crate::window::default_window_title() }
            PathPointAt: pure fn(ElementReference, f32) -> LogicalPosition
            PathAngleAt: pure fn(ElementReference, f32) -> f32 => Angle
        }
    };
}
