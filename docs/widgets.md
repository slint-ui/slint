# Widgets

Widgets are not imported by default, and need to be imported from `"sixtyfps_widgets.60"`

Their appearance can change depending on the style

## `Button`

### Properties

* **`text`** (*string*): The text written in the button.
* **`pressed`**: (*bool*): Set to true when the button is pressed.
* **`enabled`**: (*bool*): Defaults to true. When false, the button cannot be pressed

### Callbacks

* **`clicked`**

### Example

```60
import { Button } from "sixtyfps_widgets.60";
Example := Window {
    width: 100px;
    height: 25px;
    Button {
        width: parent.width;
        height: parent.height;
        text: "Click Me";
        clicked => { self.text = "Clicked"; }
    }
}
```

## `CheckBox`

### Properties

* **`text`** (*string*): The text written next to the checkbox.
* **`checked`**: (*bool*): Whether the checkbox is checked or not.

### Callbacks

* **`toggled`**: The checkbox value changed

### Example

```60
import { CheckBox } from "sixtyfps_widgets.60";
Example := Window {
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

* **`value`** (*int*): The value.
* **`minimum`** (*int*): The minimum value (default: 0).
* **`maximum`** (*int*): The maximum value (default: 100).

### Example

```60
import { SpinBox } from "sixtyfps_widgets.60";
Example := Window {
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

* **`value`** (*float*): The value.
* **`minimum`** (*float*): The minimum value (default: 0)
* **`maximum`** (*float*): The maximum value (default: 100)

### Example

```60
import { Slider } from "sixtyfps_widgets.60";
Example := Window {
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

* **`title`** (*string*): A text written as the title of the group box.

### Example

```60
import { GroupBox } from "sixtyfps_widgets.60";
Example := Window {
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

### Properties

* **`text`** (*string*): The test being edited
* **`has_focus`**: (*bool*): Set to true when the line edit currently has the focus
* **`placeholder_text`**: (*string*): A placeholder text being shown when there is no text in the edit field
* **`enabled`**: (*bool*): Defaults to true. When false, nothing can be entered

### Callbacks

* **`accepted`**: Enter was pressed
* **`edited`**: Emitted when the text has changed because the user modified it

### Example

```60
import { LineEdit } from "sixtyfps_widgets.60";
Example := Window {
    width: 200px;
    height: 25px;
    LineEdit {
        width: parent.width;
        height: parent.height;
        placeholder_text: "Enter text here";
    }
}
```

## `ScrollView`

A Scrollview contains a viewport that is bigger than the view and can be scrolled.
It has scrollbar to interact with.

### Properties

* **`viewport_width`** and **`viewport_height`** (*length*): The `width` and `length` properties of the viewport
* **`viewport_x`** and **`viewport_y`** (*length*): The `x` and `y` properties of the viewport. Usually these are negative
* **`visible_width`** and **`visible_height`** (*length*): The size of the visible area of the ScrollView (not including the scrollbar)

### Example

```60
import { ScrollView } from "sixtyfps_widgets.60";
Example := Window {
    width: 200px;
    height: 200px;
    ScrollView {
        width: 200px;
        height: 200px;
        viewport_width: 300px;
        viewport_height: 300px;
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

```60
import { ListView } from "sixtyfps_widgets.60";
Example := Window {
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

* **`model`** (*`[StandardListViewItem]`*): The model
* **`current_item`** (*int*): The index of the currently active item. -1 mean none is selected, which is the default

### Example

```60
import { StandardListView } from "sixtyfps_widgets.60";
Example := Window {
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

## `HorizontalBox`, `VerticalBox`, `GridBox`

That's the same as `HorizontalLayout`, `VerticalLayout` or `GridLayout` but the spacing and padding values
 depending on the style instead of defaulting to 0.
