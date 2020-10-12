# Widgets

Widgets are not imported by default, and need to be imported from `"sixtyfps_widgets.60"`

Their appearence can change depending on the style

## `Button`

### Properties

* **`text`** (*string*): The text written in the button.
* **`pressed`**: (*bool*): Set to true when the button is pressed.

### Signals

* **`clicked`**

### Example

```60
import { Button } from "sixtyfps_widgets.60";
Example := Window {
    width: 100lx;
    height: 25lx;
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

### Signals

* **`toggled`**: The checkbox value changed

### Example

```60
import { CheckBox } from "sixtyfps_widgets.60";
Example := Window {
    width: 200lx;
    height: 25lx;
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

### Example

```60
import { SpinBox } from "sixtyfps_widgets.60";
Example := Window {
    width: 200lx;
    height: 25lx;
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
* **`min`** (*float*): The minimum value (default: 0)
* **`max`** (*float*): The maximum value (default: 100)

### Example

```60
import { Slider } from "sixtyfps_widgets.60";
Example := Window {
    width: 200lx;
    height: 25lx;
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
    width: 200lx;
    height: 100lx;
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

### Signals

* **`accepted`**: Enter was pressed

### Example

```60
import { LineEdit } from "sixtyfps_widgets.60";
Example := Window {
    width: 200lx;
    height: 25lx;
    LineEdit {
        width: parent.width;
        height: parent.height;
        value: 42;
    }
}
```


## `ScrollView`

A Scrollview contains a viewport that is bigger than the view and can be scrolled.
It has scrollbar to interact with.

### Properties

* **`viewport_width`** and **`viewport_height`** (*lenght*): The `width` and `lenght` properties of the viewport
* **`viewport_x`** and **`viewport_y`** (*lenght*): The `x` and `y` properties of the viewport. Usually these are negative


### Example

```60
import { ScrollView } from "sixtyfps_widgets.60";
Example := Window {
    width: 200lx;
    height: 200lx;
    ScrollView {
        width: 200lx;
        height: 200lx;
        viewport_width: 300lx;
        viewport_height: 300lx;
        Rectangle { width: 30lx; height: 30lx; x: 275lx; y: 50lx; color: blue; }
        Rectangle { width: 30lx; height: 30lx; x: 175lx; y: 130lx; color: red; }
        Rectangle { width: 30lx; height: 30lx; x: 25lx; y: 210lx; color: yellow; }
        Rectangle { width: 30lx; height: 30lx; x: 98lx; y: 55lx; color: orange; }
    }
}
```


## `ListView`

A ListView is like a Scrollview but it should have a `for` element, and the content are
automatically layed out in a list.
Elements are only instentiated if they are visible

### Example

```60
import { ListView } from "sixtyfps_widgets.60";
Example := Window {
    width: 150lx;
    height: 150lx;
    ListView {
        width: 150lx;
        height: 150lx;
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
            height: 30lx;
            color: data.bg;
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

* **`model`** (*`[StandardListViewItem]`*): The model

### Example

```60
import { StandardListView } from "sixtyfps_widgets.60";
Example := Window {
    width: 150lx;
    height: 150lx;
    StandardListView {
        width: 150lx;
        height: 150lx;
        model: [ { text: "Blue"}, { text: "Red" }, { text: "Green" },
            { text: "Yellow" }, { text: "Black"}, { text: "White"},
            { text: "Magenta" }, { text: "Cyan" },
        ];
    }
}
```




