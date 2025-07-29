// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains all enums exposed in the .slint language.

// cSpell: ignore evenodd grabbable horizontalbox horizontallayout nesw standardbutton standardtableview verticalbox verticallayout

/// Call a macro with every enum exposed in the .slint language
///
/// Each enum is declared with `pub enum` if it should be re-exported in a public
/// language-binding module (e.g. `slint::language` in the Rust crate), or plain `enum`
/// to stay private. Consumers can dispatch on `$vis:vis`.
///
/// ## Example
/// ```rust
/// macro_rules! print_enums {
///     ($( $(#[$enum_doc:meta])* $vis:vis enum $Name:ident { $( $(#[$value_doc:meta])* $Value:ident,)* })*) => {
///         $(println!("{} ({}) => [{}]", stringify!($Name), stringify!($vis), stringify!($($Value),*));)*
///     }
/// }
/// i_slint_common::for_each_enums!(print_enums);
/// ```
#[macro_export]
macro_rules! for_each_enums {
    ($macro:ident) => {
        $macro! {
            /// This enum describes the different types of alignment of text along the horizontal axis of a `Text` or `StyledText` element.
            #[non_exhaustive]
            enum TextHorizontalAlignment {
                /// The text will be aligned with the start edge of the containing box.
                /// This could be left or right depending on the direction of the text.
                Start,
                /// The text will be aligned with the end edge of the containing box.
                /// This could be left or right depending on the direction of the text.
                End,
                /// The text will be aligned with the left edge of the containing box.
                Left,
                /// The text will be horizontally centered within the containing box.
                Center,
                /// The text will be aligned to the right of the containing box.
                Right,
            }

            /// This enum describes the different types of alignment of text along the vertical axis of a `Text` or `StyledText` element.
            #[non_exhaustive]
            enum TextVerticalAlignment {
                /// The text will be aligned to the top of the containing box.
                Top,
                /// The text will be vertically centered within the containing box.
                Center,
                /// The text will be aligned to the bottom of the containing box.
                Bottom,
            }

            /// This enum describes the how the text wraps if it is too wide to fit in the width of a `Text` or `StyledText` element.
            #[non_exhaustive]
            enum TextWrap {
                /// The text won't wrap, but instead will overflow.
                NoWrap,
                /// The text will be wrapped at word boundaries if possible, or at any location for very long words.
                WordWrap,
                /// The text will be wrapped at any character. Currently only supported by the Qt and Software renderers.
                CharWrap,
            }

            /// This enum describes the how the text appears if it is too wide to fit in the width of a `Text` or `StyledText` element.
            #[non_exhaustive]
            enum TextOverflow {
                /// The text will simply be clipped.
                Clip,
                /// The text will be elided with `…`.
                Elide,
            }

            /// This enum describes the positioning of a text stroke relative to the border of the glyphs in a `Text` or `StyledText` element.
            #[non_exhaustive]
            enum TextStrokeStyle {
                /// The inside edge of the stroke is at the outer edge of the text.
                Outside,
                /// The center line of the stroke is at the outer edge of the text, like in Adobe Illustrator.
                Center,
            }

            /// This enum describes the auto-capitalization behavior that the input method
            /// (e.g. a soft keyboard) should apply while text is entered in a `TextInput`.
            #[non_exhaustive]
            pub enum CapitalizationMode {
                /// No auto-capitalization.
                None,
                /// Capitalize the first character of each sentence.
                Sentences,
                /// Capitalize the first character of each word.
                Words,
                /// Capitalize all characters.
                Characters,
            }

            /// This enum describes whether an event was rejected or accepted by an event handler.
            #[non_exhaustive]
            enum EventResult {
                /// The event is rejected by this event handler and may then be handled by the parent item
                Reject,
                /// The event is accepted and won't be processed further
                Accept,
            }

            /// This enum describes the different ways of deciding what the inside of a shape described by a path shall be.
            #[non_exhaustive]
            enum FillRule {
                /// The ["nonzero" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#nonzero).
                Nonzero,
                /// The ["evenodd" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#evenodd)
                Evenodd,
            }

            /// Use this enum to add standard buttons to a `Dialog`. The look and positioning
            /// of these `StandardButton`s depends on the environment
            /// (OS, UI environment, etc.) the application runs in.
            #[non_exhaustive]
            enum StandardButtonKind {
                /// A "OK" button that accepts a `Dialog`, closing it when clicked.
                Ok,
                /// A "Cancel" button that rejects a `Dialog`, closing it when clicked.
                Cancel,
                /// A "Apply" button that should accept values from a
                /// `Dialog` without closing it.
                Apply,
                /// A "Close" button, which should close a `Dialog` without looking at values.
                Close,
                /// A "Reset" button, which should reset the `Dialog` to its initial state.
                Reset,
                /// A "Help" button, which should bring up context related documentation when clicked.
                Help,
                /// A "Yes" button, used to confirm an action.
                Yes,
                /// A "No" button, used to deny an action.
                No,
                /// A "Abort" button, used to abort an action.
                Abort,
                /// A "Retry" button, used to retry a failed action.
                Retry,
                /// A "Ignore" button, used to ignore a failed action.
                Ignore,
            }

            /// This enum represents the value of the `dialog-button-role` property which can be added to
            /// any element within a `Dialog` to put that item in the button row, and its exact position
            /// depends on the role and the platform.
            #[non_exhaustive]
            enum DialogButtonRole {
                /// This isn't a button meant to go into the bottom row
                None,
                /// This is the role of the main button to click to accept the dialog. e.g. "Ok" or "Yes"
                Accept,
                /// This is the role of the main button to click to reject the dialog. e.g. "Cancel" or "No"
                Reject,
                /// This is the role of the "Apply" button
                Apply,
                /// This is the role of the "Reset" button
                Reset,
                /// This is the role of the  "Help" button
                Help,
                /// This is the role of any other button that performs another action.
                Action,
            }

            /// This enum describes the different reasons for a FocusEvent
            #[non_exhaustive]
            enum FocusReason {
                /// A built-in function invocation caused the event (`.focus()`, `.clear-focus()`)
                Programmatic,
                /// Keyboard navigation caused the event (tabbing)
                TabNavigation,
                /// A mouse click caused the event
                PointerClick,
                /// A popup caused the event
                PopupActivation,
                /// The window manager changed the active window and caused the event
                WindowActivation,
            }

            /// The enum reports what happened to the `PointerEventButton` in the event
            #[non_exhaustive]
            pub enum PointerEventKind {
                /// The action was cancelled.
                Cancel,
                /// The button was pressed.
                Down,
                /// The button was released.
                Up,
                /// The pointer has moved,
                Move,
            }

            /// This enum describes the different types of buttons for a pointer event,
            /// typically on a mouse or a pencil.
            #[non_exhaustive]
            pub enum PointerEventButton {
                /// A button that is none of left, right, middle, back or forward. For example,
                /// this is used for the task button on a mouse with many buttons.
                Other,
                /// The left button.
                Left,
                /// The right button.
                Right,
                /// The center button.
                Middle,
                /// The back button.
                Back,
                /// The forward button.
                Forward,
            }

            /// Represents different types of mouse cursors. It's a subset of the mouse cursors available in CSS.
            /// For details and pictograms see the [MDN Documentation for cursor](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor#values).
            /// Depending on the backend and used OS unidirectional resize cursors may be replaced with bidirectional ones.
            #[non_exhaustive]
            enum BuiltInMouseCursor {
                /// The systems default cursor.
                Default,
                /// No cursor is displayed.
                None,
                //context_menu,
                /// A cursor indicating help information.
                Help,
                /// A pointing hand indicating a link.
                Pointer,
                /// The program is busy but can still be interacted with.
                Progress,
                /// The program is busy.
                Wait,
                //cell,
                /// A crosshair.
                Crosshair,
                /// A cursor indicating selectable text.
                Text,
                //vertical_text,
                /// An alias or shortcut is being created.
                Alias,
                /// A copy is being created.
                Copy,
                /// Something is to be moved.
                Move,
                /// Something can't be dropped here.
                NoDrop,
                /// An action isn't allowed
                NotAllowed,
                /// Something is grabbable.
                Grab,
                /// Something is being grabbed.
                Grabbing,
                //all_scroll,
                /// Indicating that a column is resizable horizontally.
                ColResize,
                /// Indicating that a row is resizable vertically.
                RowResize,
                /// Unidirectional resize north.
                NResize,
                /// Unidirectional resize east.
                EResize,
                /// Unidirectional resize south.
                SResize,
                /// Unidirectional resize west.
                WResize,
                /// Unidirectional resize north-east.
                NeResize,
                /// Unidirectional resize north-west.
                NwResize,
                /// Unidirectional resize south-east.
                SeResize,
                /// Unidirectional resize south-west.
                SwResize,
                /// Bidirectional resize east-west.
                EwResize,
                /// Bidirectional resize north-south.
                NsResize,
                /// Bidirectional resize north-east-south-west.
                NeswResize,
                /// Bidirectional resize north-west-south-east.
                NwseResize,
                //zoom_in,
                //zoom_out,
            }

            /// This enum defines how the source image or path shall fit into an `Image` or `Path` element.
            #[non_exhaustive]
            enum ImageFit {
                /// Scales and stretches the source to fit the width and height of the element.
                Fill,
                /// The source is scaled to fit into the element's dimensions while preserving the aspect ratio.
                Contain,
                /// The source is scaled to cover the element's dimensions while preserving the aspect ratio.
                /// If the aspect ratios don't match, the source will be clipped to fit.
                Cover,
                /// Preserves the size of the source in logical pixels.
                /// The source will still be scaled by the scale factor that applies to all elements in the window.
                /// Any extra space will be left blank.
                Preserve,
            }

            /// This enum specifies the horizontal alignment of the source image.
            #[non_exhaustive]
            enum ImageHorizontalAlignment {
                /// Aligns the source image at the center of the `Image` element.
                Center,
                /// Aligns the source image at the left of the `Image` element.
                Left,
                /// Aligns the source image at the right of the `Image` element.
                Right,
            }

            /// This enum specifies the vertical alignment of the source image.
            #[non_exhaustive]
            enum ImageVerticalAlignment {
                /// Aligns the source image at the center of the `Image` element.
                Center,
                /// Aligns the source image at the top of the `Image` element.
                Top,
                /// Aligns the source image at the bottom of the `Image` element.
                Bottom,
            }

            /// This enum specifies how the source image will be scaled.
            #[non_exhaustive]
            enum ImageRendering {
                /// The image is scaled with a linear interpolation algorithm.
                Smooth,
                /// The image is scaled with the nearest neighbor algorithm.
                Pixelated,
            }

            /// This enum specifies how the source image will be tiled.
            #[non_exhaustive]
            enum ImageTiling {
                /// The source image will not be tiled.
                None,
                /// The source image will be repeated to fill the `Image` element.
                Repeat,
                /// The source image will be repeated and scaled to fill the `Image` element, ensuring an integer number of repetitions.
                Round,
            }

            /// This enum is used to define the type of the input field.
            #[non_exhaustive]
            enum InputType {
                /// The default value. This will render all characters normally
                Text,
                /// This will render all characters with a character that defaults to "*"
                Password,
                /// This will only accept and render number characters (0-9)
                Number,
                /// This will accept and render characters if it's valid part of a decimal,
                /// using the decimal separator of the current locale
                Decimal,
                /// This identifies the input field as a search box. Characters are rendered normally,
                /// but assistive technologies are informed that the field is used for searching or
                /// filtering content.
                Search,
            }

            /// Enum representing the `alignment` property of a
            /// `HorizontalBox`, a `VerticalBox`,
            /// a `HorizontalLayout`, or `VerticalLayout`.
            #[non_exhaustive]
            enum LayoutAlignment {
                /// Use the minimum size of all elements in a layout, distribute remaining space
                /// based on `*-stretch` among all elements.
                Stretch,
                /// Use the preferred size for all elements, distribute remaining space evenly before the
                /// first and after the last element.
                Center,
                /// Use the preferred size for all elements, put remaining space after the last element.
                Start,
                /// Use the preferred size for all elements, put remaining space before the first
                /// element.
                End,
                /// Use the preferred size for all elements, distribute remaining space evenly between
                /// elements.
                SpaceBetween,
                /// Use the preferred size for all elements, distribute remaining space evenly
                /// between the elements, and use half spaces at the start and end.
                SpaceAround,
                /// Use the preferred size for all elements, distribute remaining space evenly before the
                /// first element, after the last element and between elements.
                SpaceEvenly,
            }

            /// The direction in which flex items are placed in a flex container.
            #[non_exhaustive]
            enum FlexboxLayoutDirection {
                /// Items are placed in a row, from left to right.
                Row,
                /// Items are placed in a row in reverse order, from right to left.
                RowReverse,
                /// Items are placed in a column, from top to bottom.
                Column,
                /// Items are placed in a column in reverse order, from bottom to top.
                ColumnReverse,
            }

            /// Controls the distribution of flex lines along the cross axis in a flex container.
            #[non_exhaustive]
            enum FlexboxLayoutAlignContent {
                /// Lines are stretched to fill the container along the cross axis.
                Stretch,
                /// Lines are placed at the start of the cross axis.
                Start,
                /// Lines are placed at the end of the cross axis.
                End,
                /// Lines are centered along the cross axis.
                Center,
                /// Equal gaps between lines, no gap at the edges.
                SpaceBetween,
                /// Equal gaps around each line (half-size at edges).
                SpaceAround,
                /// Equal gaps between lines and at the edges.
                SpaceEvenly,
            }

            /// Controls the alignment of individual items along the cross axis of a layout.
            /// Used as the `cross-axis-alignment` property of `HorizontalLayout`, `VerticalLayout`,
            /// and `FlexboxLayout`.
            #[non_exhaustive]
            enum CrossAxisAlignment {
                /// Items are stretched to fill the cross axis.
                Stretch,
                /// Items are placed at the start of the cross axis.
                Start,
                /// Items are placed at the end of the cross axis.
                End,
                /// Items are centered along the cross axis.
                Center,
            }

            /// Overrides the container's `cross-axis-alignment` for a specific flex item.
            #[non_exhaustive]
            enum FlexboxLayoutAlignSelf {
                /// Use the container's `cross-axis-alignment` value (default).
                Auto,
                /// The item is stretched to fill the line along the cross axis.
                Stretch,
                /// The item is placed at the start of the cross axis.
                Start,
                /// The item is placed at the end of the cross axis.
                End,
                /// The item is centered along the cross axis.
                Center,
            }

            /// Controls whether flex items wrap onto multiple lines.
            #[non_exhaustive]
            enum FlexboxLayoutWrap {
                /// Flex items wrap onto multiple lines, from top to bottom (for row direction) or left to right (for column direction).
                Wrap,
                /// All flex items are laid out on a single line (default for CSS, but Slint defaults to `wrap`).
                NoWrap,
                /// Flex items wrap onto multiple lines in the reverse direction.
                WrapReverse,
            }

            /// PathEvent is a low-level data structure describing the composition of a path. Typically it is
            /// generated at compile time from a higher-level description, such as SVG commands.
            #[non_exhaustive]
            enum PathEvent {
                /// The beginning of the path.
                Begin,
                /// A straight line on the path.
                Line,
                /// A quadratic bezier curve on the path.
                Quadratic,
                /// A cubic bezier curve on the path.
                Cubic,
                /// The end of the path that remains open.
                EndOpen,
                /// The end of a path that is closed.
                EndClosed,
            }

            /// This enum represents the different values for the `accessible-role` property, used to describe the
            /// role of an element in the context of assistive technology such as screen readers.
            ///
            /// In addition to widget roles, this enum includes *landmark* roles (`banner`, `complementary`,
            /// `content-info`, `form`, `main`, `navigation`, `region`, `search`).
            /// Landmarks identify large content areas that screen reader users can jump between,
            /// giving the application a navigable structure similar to headings in a document.
            /// See [WAI-ARIA Landmark Regions](https://www.w3.org/WAI/ARIA/apg/practices/landmark-regions/)
            /// for guidance on when and how to use them.
            #[non_exhaustive]
            pub enum AccessibleRole {
                /// The element isn't accessible.
                None,
                /// The element is a `Button` or behaves like one.
                Button,
                /// The element is a `CheckBox` or behaves like one.
                Checkbox,
                /// The element is a `ComboBox` or behaves like one.
                Combobox,
                /// The element is a `GroupBox` or behaves like one.
                Groupbox,
                /// The element is an `Image` or behaves like one. This is automatically applied to `Image` elements.
                Image,
                /// The element is a `ListView` or behaves like one.
                List,
                /// The element is a `Slider` or behaves like one.
                Slider,
                /// The element is a `SpinBox` or behaves like one.
                Spinbox,
                /// The element is a `Tab` or behaves like one.
                Tab,
                /// The element is similar to the tab bar in a `TabWidget`.
                TabList,
                /// The element is a container for tab content.
                TabPanel,
                /// The role for a `Text` element. This is automatically applied to `Text` elements.
                Text,
                /// The role for a `TableView` or behaves like one.
                Table,
                /// The role for a TreeView or behaves like one. (Not provided yet)
                Tree,
                /// The element is a `ProgressIndicator` or behaves like one.
                ProgressIndicator,
                /// The role for widget with editable text such as a `LineEdit` or a `TextEdit`.
                /// This is automatically applied to `TextInput` elements.
                TextInput,
                /// The element is a `Switch` or behaves like one.
                Switch,
                /// The element is an item in a `ListView`.
                ListItem,
                /// The element is a `RadioButton` or behaves like one.
                RadioButton,
                /// The element is a container grouping related `RadioButton`s.
                RadioGroup,
                // Landmark roles
                /// Landmark: the header area of the application, typically containing a logo, title, or global navigation.
                Banner,
                /// Landmark: a supporting section that complements the main content, such as a sidebar.
                Complementary,
                /// Landmark: information about the application or its content, typically at the bottom (e.g. status bar, copyright).
                ContentInfo,
                /// Landmark: a region containing input fields and controls for submitting information.
                Form,
                /// Landmark: the primary content of the application. Each view should have exactly one `main` landmark.
                Main,
                /// Landmark: a group of links or controls used for navigating the application.
                Navigation,
                /// Landmark: a generic section significant enough to be listed in a summary.
                /// Use a more specific landmark if one applies.
                Region,
                /// Landmark: a region containing controls for searching or filtering content.
                Search,
            }

            /// This enum represents the different values of the `accessible-live-region` property.
            /// It indicates that an element is a live region whose content changes should be
            /// announced by assistive technologies.
            #[non_exhaustive]
            pub enum AccessibleLiveness {
                /// Use in regions that present information that is of low-importance to the user.
                /// Assistive technologies are expected to not announce changes unless the user explicitly asks for it.
                Off,
                /// Use in regions that present new information to users.
                /// Assistive technologies are expected to not interrupt the user to inform of changes to the live region.
                Polite,
                /// Use in regions that present information that a user should know about right away.
                /// Assistive technologies are expected to announce to the user as soon as possible.
                Assertive,
            }

            /// This enum represents the different values of the `sort-order` property.
            /// It's used to sort a `StandardTableView` by a column.
            #[non_exhaustive]
            pub enum SortOrder {
                /// The column is unsorted.
                Unsorted,

                /// The column is sorted in ascending order.
                Ascending,

                /// The column is sorted in descending order.
                Descending,
            }

            /// Represents the orientation of an element or widget such as the `Slider`.
            // (on purpose not #[non_exhaustive])
            pub enum Orientation {
                /// Element is oriented horizontally.
                Horizontal,
                /// Element is oriented vertically.
                Vertical,
            }

            /// This enum indicates the color scheme used by the widget style. Use this to explicitly switch
            /// between dark and light schemes, or choose Unknown to fall back to the system default.
            #[non_exhaustive]
            pub enum ColorScheme {
                /// The scheme is not known and a system wide setting configures this. This could mean that
                /// the widgets are shown in a dark or light scheme, but it could also be a custom color scheme.
                Unknown,
                /// The style chooses light colors for the background and dark for the foreground.
                Dark,
                /// The style chooses dark colors for the background and light for the foreground.
                Light,
            }

            /// This enum describes the direction of an animation.
            #[non_exhaustive]
            enum AnimationDirection {
                /// The ["normal" direction as defined in CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/animation-direction#normal).
                Normal,
                /// The ["reverse" direction as defined in CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/animation-direction#reverse).
                Reverse,
                /// The ["alternate" direction as defined in CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/animation-direction#alternate).
                Alternate,
                /// The ["alternate reverse" direction as defined in CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/animation-direction#alternate-reverse).
                AlternateReverse,
            }

            /// This enum describes the scrollbar visibility
            #[non_exhaustive]
            enum ScrollBarPolicy {
                /// Scrollbar will be visible only when needed
                AsNeeded,
                /// Scrollbar never shown
                AlwaysOff,
                /// Scrollbar always visible
                AlwaysOn,
            }

            // This enum describes the close behavior of `PopupWindow`
            #[non_exhaustive]
            enum PopupClosePolicy {
                /// Closes the `PopupWindow` when user clicks or presses the escape key.
                CloseOnClick,

                /// Closes the `PopupWindow` when user clicks outside of the popup or presses the escape key.
                CloseOnClickOutside,

                /// Does not close the `PopupWindow` automatically when user clicks.
                NoAutoClose,
            }

            /// This enum describes the appearance of the ends of stroked paths.
            #[non_exhaustive]
            enum LineCap {
                /// The stroke ends with a flat edge that is perpendicular to the path.
                Butt,
                /// The stroke ends with a rounded edge.
                Round,
                /// The stroke ends with a square projection beyond the path.
                Square,
            }

            /// This enum describes the appearance of the joins between segments of stroked paths.
            #[non_exhaustive]
            enum LineJoin {
                /// The stroke joins with a sharp corner or a clipped corner, depending on the miter limit.
                Miter,
                /// The stroke joins with a smooth, rounded corner.
                Round,
                /// The stroke joins with a beveled (flattened) corner.
                Bevel,
            }

            /// This enum describes the action negotiated between the source of a drag (`DragArea`)
            /// and its target (`DropArea`) during a drag-and-drop operation. The source declares
            /// which actions it permits, the target picks one in its `can-drop` callback, and the
            /// chosen action is reported back to the source via `drag-finished` so that, for
            /// example, a `move` source can remove the original data. The same enum is used for
            /// drags that come from another application or window once native drag-and-drop is
            /// in play.
            #[non_exhaustive]
            pub enum DragAction {
                /// No action: the drag is rejected, no drop will be delivered.
                None,
                /// The data is copied to the target; the source retains it.
                Copy,
                /// The data is moved to the target; the source should remove it once the
                /// operation completes.
                Move,
                /// A link to the source data is created at the target; neither side gives
                /// up ownership.
                Link,
            }

            /// This enum describes the detected operating system types.
            #[non_exhaustive]
            enum OperatingSystemType {
                /// This variant includes any version of Android running mobile phones, tablets, as well as embedded Android devices.
                Android,
                /// This variant covers iOS running on iPhones and iPads.
                Ios,
                /// This variant covers macOS running on Apple's Mac computers.
                Macos,
                /// This variant covers any version of Linux, except Android.
                Linux,
                /// This variant covers Microsoft Windows.
                Windows,
                /// This variant is reported when the operating system is none of the above.
                Other,
            }
        }
    };
}
