# Widgets

Slint provides a series of built-in widgets that can be imported from `"std-widgets.slint"`.

The widget appearance depends on the selected style. The following styles are available:

-   `fluent`: The **Fluent** style implements the [Fluent Design System](https://www.microsoft.com/design/fluent/).
-   `material`: The **Material** style implements the [Material Design](https://m3.material.io).
-   `native`: The **Native** style resembles the appearance of the controls that are native to the platform they
    are used on. This specifically includes support for the look and feel of controls on macOS and Windows. This
    style is only available if you have Qt installed on your system.

See [Selecting a Widget Style](#selecting-a-widget-style) for details how to select the style. If no style is selected, `native` is the default. If `native` is not available, `fluent` is the default.

## `Button`

### Properties

-   **`checkable`** (_bool_): Shows whether the button can be checked or not. This enables the `checked` property to possibly become `true`.
-   **`checked`** (_bool_): Shows whether the button is checked or not. Needs `checkable` to be `true` to work.
-   **`enabled`**: (_bool_): Defaults to true. When false, the button cannot be pressed
-   **`icon`** (_image_): The image to show in the button. Note that not all styles support drawing icons.
-   **`pressed`**: (_bool_): Set to true when the button is pressed.
-   **`text`** (_string_): The text written in the button.

### Callbacks

-   **`clicked`**

### Example

```slint
import { Button, VerticalBox } from "std-widgets.slint";
export component Example inherits Window {
    VerticalBox {
        Button {
            text: "Click Me";
            clicked => { self.text = "Clicked"; }
        }
    }
}
```

## `StandardButton`

The StandardButton looks like a button, but instead of customizing with `text` and `icon`,
it can used one of the pre-defined `kind` and the text and icon will depend on the style.

### Properties

-   **`kind`** (_enum_): The kind of button, one of
    `ok` `cancel`, `apply`, `close`, `reset`, `help`, `yes`, `no,` `abort`, `retry` or `ignore`
-   **`enabled`**: (_bool_): Defaults to true. When false, the button cannot be pressed

### Callbacks

-   **`clicked`**

### Example

```slint
import { StandardButton, VerticalBox } from "std-widgets.slint";
export component Example inherits Window {
  VerticalBox {
    StandardButton { kind: ok; }
    StandardButton { kind: apply; }
    StandardButton { kind: cancel; }
  }
}
```

## `CheckBox`

### Properties

-   **`text`** (_string_): The text written next to the checkbox.
-   **`checked`**: (_bool_): Whether the checkbox is checked or not.

### Callbacks

-   **`toggled`**: The checkbox value changed

### Example

```slint
import { CheckBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    CheckBox {
        width: parent.width;
        height: parent.height;
        text: "Hello World";
    }
}
```

## `SpinBox`

### Properties

-   **`value`** (_int_): The value.
-   **`minimum`** (_int_): The minimum value (default: 0).
-   **`maximum`** (_int_): The maximum value (default: 100).

### Example

```slint
import { SpinBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    SpinBox {
        width: parent.width;
        height: parent.height;
        value: 42;
    }
}
```

## `Slider`

### Properties

-   **`value`** (_float_): The value.
-   **`minimum`** (_float_): The minimum value (default: 0)
-   **`maximum`** (_float_): The maximum value (default: 100)

### Callbacks

-   **`changed(float)`**: The value was changed

### Example

```slint
import { Slider } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    Slider {
        width: parent.width;
        height: parent.height;
        value: 42;
    }
}
```

## `GroupBox`

### Properties

-   **`title`** (_string_): A text written as the title of the group box.

### Example

```slint
import { GroupBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 100px;
    GroupBox {
        title: "A Nice Title";
        Text {
            text: "Hello World";
            color: blue;
        }
    }
}
```

## `LineEdit`

A widget used to enter a single line of text

### Properties

-   **`text`** (_string_): The text being edited
-   **`font-size`** (_length_): the size of the font of the input text
-   **`has-focus`**: (_bool_): Set to true when the line edit currently has the focus
-   **`placeholder-text`**: (_string_): A placeholder text being shown when there is no text in the edit field
-   **`enabled`**: (_bool_): Defaults to true. When false, nothing can be entered
-   **`read-only`** (_bool_): When set to `true`, text editing via keyboard and mouse is disabled but
    selecting text is still enabled as well as editing text programatically (default value: `false`)
-   **`input-type`** (_enum [`InputType`](builtin_enums.md#inputtype)_): The way to allow special input viewing properties such as password fields (default value: `text`).
-   **`horizontal-alignment`** (_enum [`TextHorizontalAlignment`](builtin_enums.md#texthorizontalalignment)_): The horizontal alignment of the text.

### Callbacks

-   **`accepted`**: Enter was pressed
-   **`edited`**: Emitted when the text has changed because the user modified it

### Example

```slint
import { LineEdit } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 25px;
    LineEdit {
        font-size: 14px;
        width: parent.width;
        height: parent.height;
        placeholder-text: "Enter text here";
    }
}
```

## `TextEdit`

Similar to LineEdit, but can be used to enter several lines of text

_Note:_ The current implementation only implement very few basic shortcut. More
shortcut will be implemented in a future version: <https://github.com/slint-ui/slint/issues/474>

### Properties

-   **`text`** (_string_): The text being edited
-   **`font-size`** (_length_): the size of the font of the input text
-   **`has-focus`**: (_bool_): Set to true when the widget currently has the focus
-   **`enabled`**: (_bool_): Defaults to true. When false, nothing can be entered
-   **`read-only`** (_bool_): When set to `true`, text editing via keyboard and mouse is disabled but
    selecting text is still enabled as well as editing text programatically (default value: `false`)
-   **`wrap`** (_enum [`TextWrap`](builtin_enums.md#textwrap)_): The way the text wraps (default: word-wrap).
-   **`horizontal-alignment`** (_enum [`TextHorizontalAlignment`](builtin_enums.md#texthorizontalalignment)_): The horizontal alignment of the text.

### Callbacks

-   **`edited`**: Emitted when the text has changed because the user modified it

### Example

```slint
import { TextEdit } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 200px;
    TextEdit {
        font-size: 14px;
        width: parent.width;
        height: parent.height;
        text: "Lorem ipsum dolor sit amet,\n consectetur adipisici elit";
    }
}
```

## `ScrollView`

A Scrollview contains a viewport that is bigger than the view and can be
scrolled. It has scrollbar to interact with. The viewport-width and
viewport-height are calculated automatically to create a scollable view
except for when using a for loop to populate the elements. In that case
the viewport-width and viewport-height are not calculated automatically
and must be set manually for scrolling to work. The ability to
automatically calculate the viewport-width and viewport-height when
using for loops may be added in the future and is tracked in issue #407.

### Properties

-   **`viewport-width`** and **`viewport-height`** (_length_): The `width` and `length` properties of the viewport
-   **`viewport-x`** and **`viewport-y`** (_length_): The `x` and `y` properties of the viewport. Usually these are negative
-   **`visible-width`** and **`visible-height`** (_length_): The size of the visible area of the ScrollView (not including the scrollbar)
-   **`enabled`** and **`has-focus`** (_bool_): property that are only used to render the frame as disabled or focused, but do not
    change the behavior of the widget.

### Example

```slint
import { ScrollView } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 200px;
    ScrollView {
        width: 200px;
        height: 200px;
        viewport-width: 300px;
        viewport-height: 300px;
        Rectangle { width: 30px; height: 30px; x: 275px; y: 50px; background: blue; }
        Rectangle { width: 30px; height: 30px; x: 175px; y: 130px; background: red; }
        Rectangle { width: 30px; height: 30px; x: 25px; y: 210px; background: yellow; }
        Rectangle { width: 30px; height: 30px; x: 98px; y: 55px; background: orange; }
    }
}
```

## `ListView`

A ListView is like a Scrollview but it should have a `for` element, and the content are
automatically layed out in a list.
Elements are only instantiated if they are visible

### Properties

Same as ScrollView

### Example

```slint
import { ListView } from "std-widgets.slint";
export component Example inherits Window {
    width: 150px;
    height: 150px;
    ListView {
        width: 150px;
        height: 150px;
        for data in [
            { text: "Blue", color: #0000ff, bg: #eeeeee},
            { text: "Red", color: #ff0000, bg: #eeeeee},
            { text: "Green", color: #00ff00, bg: #eeeeee},
            { text: "Yellow", color: #ffff00, bg: #222222 },
            { text: "Black", color: #000000, bg: #eeeeee },
            { text: "White", color: #ffffff, bg: #222222 },
            { text: "Magenta", color: #ff00ff, bg: #eeeeee },
            { text: "Cyan", color: #00ffff, bg: #222222 },
        ] : Rectangle {
            height: 30px;
            background: data.bg;
            width: parent.width;
            Text {
                x: 0;
                text: data.text;
                color: data.color;
            }
        }
    }
}
```

## `StandardListView`

Like ListView, but with a default delegate, and a `model` property which is a model of type
`StandardListViewItem`

The `StandardListViewItem` is equivalent to `{ text: string }` but will be improved in the future with `icon`, `checked` and so on (TODO)

### Properties

Same as ListView, and in addition:

-   **`model`** (_`[StandardListViewItem]`_): The model
-   **`current-item`** (_int_): The index of the currently active item. -1 mean none is selected, which is the default

### Functions

-   **`set-current-item(index: int)`**: Sets the current item and brings it into view

### Example

```slint
import { StandardListView } from "std-widgets.slint";
export component Example inherits Window {
    width: 150px;
    height: 150px;
    StandardListView {
        width: 150px;
        height: 150px;
        model: [ { text: "Blue"}, { text: "Red" }, { text: "Green" },
            { text: "Yellow" }, { text: "Black"}, { text: "White"},
            { text: "Magenta" }, { text: "Cyan" },
        ];
    }
}
```

## `StandardTableView`

The `StandardTableView` represents a table of data with columns and rows. Cells are organised in a model where each row is a model of `StandardListViewItem`.

### Properties

Same as ListView, and in addition:

-   **`current-sort-column`** (_int_): Indicates the sorted column. -1 mean no column is sorted.
-   **`columns`** (_`[TableColumn]`_): Defines the model of the table columns.
-   **`rows`** (_`[StandardListViewItem]`_): Defines the model of table rows.

### Callbacks

-   **`sort-ascending(int)`**: Emitted if the model should be sorted by the given column in ascending order.
-   **`sort-descending(int)`**: Emitted if the model should be sorted by the given column in descending order.

### Example

```slint
import { StandardTableView } from "std-widgets.slint";
export component Example inherits Window {
    width: 230px;
    height: 200px;
    StandardTableView {
        width: 230px;
        height: 200px;
        columns: [
            { title: "Header 1" },
            { title: "Header 2" },
        ];
        rows: [
            [
                { text: "Item 1" }, { text: "Item 2" },
            ],
            [
                { text: "Item 1" }, { text: "Item 2" },
            ],
            [
                { text: "Item 1" }, { text: "Item 2" },
            ]
        ];
    }
}
```

## `ComboBox`

A button that, when clicked, opens a popup to select a value.

### Properties

-   **`model`** (_\[string\]_): The list of possible values
-   **`current-index`**: (_int_): The index of the selected value (-1 if no value is selected)
-   **`current-value`**: (_string_): The currently selected text
-   **`enabled`**: (_bool_): When false, the combobox cannot be opened (default: true)

### Callbacks

-   **`selected(string)`**: A value was selected from the combo box. The argument is the currently selected value.

### Example

```slint
import { ComboBox } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 130px;
    ComboBox {
        y: 0px;
        width: self.preferred-width;
        height: self.preferred-height;
        model: ["first", "second", "third"];
        current-value: "first";
    }
}
```

## `TabWidget`

TabWidget is a container for a set of tabs. It can only have `Tab` elements as children and only one tab will be visible at
a time.

### Properties

-   **`current-index`** (_int_): The index of the currently visible tab

### Properties of the `Tab` element

-   **`title`** (_string_): The text written in the tab bar.

### Example

```slint
import { TabWidget } from "std-widgets.slint";
export component Example inherits Window {
    width: 200px;
    height: 200px;
    TabWidget {
        Tab {
            title: "First";
            Rectangle { background: orange; }
        }
        Tab {
            title: "Second";
            Rectangle { background: pink; }
        }
    }
}
```

## `HorizontalBox`, `VerticalBox`, `GridBox`

That's the same as `HorizontalLayout`, `VerticalLayout` or `GridLayout` but the spacing and padding values
depending on the style instead of defaulting to 0.

## `AboutSlint`

This element displays the a "Made with Slint" badge.

```slint
import { AboutSlint } from "std-widgets.slint";
export component Example inherits Window {
    width: 128px;
    height: 128px;
    AboutSlint {
    }
}
```

## Selecting a Widget Style

The widget style is selected at compile time of your project. The details depend on which programming
language you're using Slint with.

<details data-snippet-language="rust">
<summary>Selecting a Widget Style when using Slint with Rust:</summary>

Before you start your compilation, you can select the style by setting the `SLINT_STYLE` variable
to one of the style names, such as `fluent` for example.

### Selecting the Widget Style When Using the `slint_build` Crate

Select the style with the [`slint_build::compile_with_config()`](https://docs.rs/slint-build/newest/slint_build/fn.compile_with_config.html) function in the compiler configuration argument.

### Selecting the Widget Style When Using the `slint_interpreter` Crate

Select the style with the [`slint_interpreter::ComponentCompiler::set_style()`](https://docs.rs/slint-interpreter/newest/slint_interpreter/struct.ComponentCompiler.html#method.set_style) function.

</details>

<details data-snippet-language="cpp">
<summary>Selecting a Widget Style when using Slint with C++:</summary>

Select the style by defining a `SLINT_STYLE` CMake cache variable to hold the style name as a string. This can be done for example on the command line:

```sh
cmake -DSLINT_STYLE="material" /path/to/source
```

</details>

### Selecting the Widget Style When Previewing Designs With `slint-viewer`

Select the style either by setting the `SLINT_STYLE` environment variable, or passing the style name with the `--style` argument:

```sh
slint-viewer --style material /path/to/design.slint
```

### Selecting the Widget Style When Previewing Designs With The Slint Visual Studio Code Extension

Select the style by first opening the Visual Studio Code settings editor:

-   On Windows/Linux - File > Preferences > Settings
-   On macOS - Code > Preferences > Settings

Then enter the style name under Extensions > Slint > Preview:Style

### Selecting the Widget Style When Previewing Designs With The Generic LSP Process

Select the style by setting the `SLINT_STYLE` environment variable before launching the process.
Alternatively, if your IDE integration allows passing command line parameters, you can specify the style via `--style`.
