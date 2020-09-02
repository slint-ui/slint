Widgets are not imported by default, and need to be imported from `"sixtyfps_widgets.60"`

Their appearence can change depending on the style

# `Button`

## Properties

* **`text`** (*string*): The text written in the button.
* **`pressed`**: (*bool*): Set to true when the button is pressed.

## Signals

* **`clicked`**

## Example

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

# `CheckBox`

## Properties

* **`text`** (*string*): The text written next to the checkbox.
* **`checked`**: (*bool*): Whether the checkbox is checked or not.

## Signals

* **`toggled`**: The checkbox value changed

## Example

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


# `SpinBox`

## Properties

* **`value`** (*int*): The value.

## Example

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


# `Slider`

## Properties

* **`value`** (*float*): The value.
* **`min`** (*float*): The minimum value (default: 0)
* **`max`** (*float*): The maximum value (default: 100)

## Example

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


# `GroupBox`

## Properties

* **`title`** (*string*): A text written as the title of the group box.

## Example

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



