# Recipes and Examples

This page provides a collection of common use-cases and how to implement them using Slint.

## Get Started

### A clickable Button

```slint
import { VerticalBox, Button } from "std-widgets.slint";
export Recipe := Window {
    property <int> counter: 0;
    VerticalBox {
        button := Button {
            text: "Button, pressed \{root.counter} times";
            clicked => {
                root.counter += 1;
            }
        }
    }
}
```

### React to a Button in native code

```slint
import { VerticalBox, Button } from "std-widgets.slint";
export Recipe := Window {
    property <int> counter: 0;
    callback button_pressed <=> button.clicked;
    VerticalBox {
        button := Button {
            text: "Button, pressed \{root.counter} times";
        }
    }
}
```

<details data-snippet-language="rust">
<summary>Rust code</summary>
In Rust you can write

```rust,no_run
slint::slint!(import { Recipe } from "docs/recipes/button_native.slint";);

fn main() {
    let recipe = Recipe::new();
    let recipe_weak = recipe.as_weak();
    recipe.on_button_pressed(move || {
        let recipe = recipe_weak.unwrap();
        let mut value = recipe.get_counter();
        value = value + 1;
        recipe.set_counter(value);
    });
    recipe.run();
}
```
</details>

<details data-snippet-language="cpp">
<summary>C++ code</summary>
In C++ you can write

```cpp
int main(int argc, char **argv)
{
    auto recipe = Recipe::create();
    recipe->on_button_pressed([&]() {
        auto value = recipe->get_counter();
        value += 1;
        recipe->set_counter(value);
    });
    recipe->run();
}
```
</details>

### Use property bindings to synchronize controls

```slint
import { VerticalBox, Slider } from "std-widgets.slint";
export Recipe := Window {
    VerticalBox {
        slider := Slider {
            maximum: 100;
        }
        Text {
            text: "Value: \{round(slider.value)}";
        }
    }
}
```

## Animations

### Animate the position of an element


```slint
import { CheckBox } from "std-widgets.slint";
export Recipe := Window {
    width: 200px;
    height: 100px;

    rect := Rectangle {
        y: 5px;
        width: 40px;
        height: 40px;
        background: blue;
        animate x {
            duration: 500ms;
            easing: ease-in-out;
        }
    }


    CheckBox {
        y: 25px;
        text: "Align rect to the right";
        toggled => {
            if (self.checked) {
                 rect.x = parent.width - rect.width;
            } else {
                 rect.x = 0px;
            }
        }
    }
}
```

### Animation Sequence

```slint
import { CheckBox } from "std-widgets.slint";
export Recipe := Window {
    width: 200px;
    height: 100px;

    rect := Rectangle {
        y: 5px;
        width: 40px;
        height: 40px;
        background: blue;
        animate x {
            duration: 500ms;
            easing: ease-in-out;
        }
        animate y {
            duration: 250ms;
            delay: 500ms;
            easing: ease-in;
        }
    }


    CheckBox {
        y: 25px;
        text: "Align rect bottom right";
        toggled => {
            if (self.checked) {
                 rect.x = parent.width - rect.width;
                 rect.y = parent.height - rect.height;
            } else {
                 rect.x = 0px;
                 rect.y = 0px;
            }
        }
    }
}
```

## States

### Associate multiple property values with states

```slint
import { HorizontalBox, VerticalBox, Button } from "std-widgets.slint";

Circle := Rectangle {
    width: 30px;
    height: 30px;
    border-radius: width / 2;
    animate x { duration: 250ms; easing: ease-in; }
    animate y { duration: 250ms; easing: ease-in-out; }
    animate background { duration: 250ms; }
}

export Recipe := Window {
    states [
        left-aligned when b1.pressed: {
            circle1.x: 0px; circle1.y: 40px; circle1.background: green;
            circle2.x: 0px; circle2.y: 0px; circle2.background: blue;
        }
        right-aligned when b2.pressed: {
            circle1.x: 170px; circle1.y: 70px; circle1.background: green;
            circle2.x: 170px; circle2.y: 00px; circle2.background: blue;
        }
    ]

    VerticalBox {
        HorizontalBox {
            max-height: min-height;
            b1 := Button {
                text: "State 1";
            }
            b2 := Button {
                text: "State 2";
            }
        }
        Rectangle {
            background: root.background.darker(20%);
            width: 200px;
            height: 100px;

            circle1 := Circle { background: green; x: 85px; }
            circle2 := Circle { background: green; x: 85px; y: 40px; }
        }
    }
}
```

### Transitions

```slint
import { HorizontalBox, VerticalBox, Button } from "std-widgets.slint";

Circle := Rectangle {
    width: 30px;
    height: 30px;
    border-radius: width / 2;
}

export Recipe := Window {
    states [
        left-aligned when b1.pressed: {
            circle1.x: 0px; circle1.y: 40px;
            circle2.x: 0px; circle2.y: 0px;
        }
        right-aligned when !b1.pressed: {
            circle1.x: 170px; circle1.y: 70px;
            circle2.x: 170px; circle2.y: 00px;
        }
    ]

    transitions [
        in left-aligned: {
            animate circle1.x, circle2.x { duration: 250ms; }
        }
        out left-aligned: {
            animate circle1.x, circle2.x { duration: 500ms; }
        }
    ]

    VerticalBox {
        HorizontalBox {
            max-height: min-height;
            b1 := Button {
                text: "Press and hold to change state";
            }
        }
        Rectangle {
            background: root.background.darker(20%);
            width: 250px;
            height: 100px;

            circle1 := Circle { background: green; x: 85px; }
            circle2 := Circle { background: blue; x: 85px; y: 40px; }
        }
    }
}
```

## Layouts

### Vertical

```slint
import { VerticalBox, Button } from "std-widgets.slint";
export Recipe := Window {
    VerticalBox {
        Button { text: "First"; }
        Button { text: "Second"; }
        Button { text: "Third"; }
    }
}
```

### Horizontal

```slint
import { HorizontalBox, Button } from "std-widgets.slint";
export Recipe := Window {
    HorizontalBox {
        Button { text: "First"; }
        Button { text: "Second"; }
        Button { text: "Third"; }
    }
}
```

### Grid

```slint
import { GridBox, Button, Slider } from "std-widgets.slint";
export Recipe := Window {
    GridBox {
        Row {
            Button { text: "First"; }
            Button { text: "Second"; }
        }
        Row {
            Button { text: "Third"; }
            Button { text: "Fourth"; }
        }
        Row {
            Slider {
                colspan: 2;
            }
        }
    }
}
```

## Global Callbacks

### Invoke a globally registered native callback from Slint

```slint-no-run
import { VerticalBox, LineEdit } from "std-widgets.slint";

global Logic := {
    callback to-upper-case(string) -> string;
    // You can collect other global properties here
}

export Recipe := Window {
    VerticalBox {
        input := LineEdit {
            text: "Text to be transformed";
        }
        HorizontalBox {
            Text { text: "Transformed:"; }
            // Callback invoked in binding expression
            Text {
                text: {
                    Logic.to-upper-case(input.text);
                }
            }
        }
    }
}
```

<details  data-snippet-language="rust">
<summary>Rust code</summary>
In Rust you can set the callback like this:

```rust
slint::slint!{
import { VerticalBox, LineEdit } from "std-widgets.slint";

global Logic := {
    callback to-upper-case(string) -> string;
    // You can collect other global properties here
}

export Recipe := Window {
    VerticalBox {
        input := LineEdit {
            text: "Text to be transformed";
        }
        HorizontalBox {
            Text { text: "Transformed:"; }
            // Callback invoked in binding expression
            Text {
                text: {
                    Logic.to-upper-case(input.text);
                }
            }
        }
    }
}
}

fn main() {
    let recipe = Recipe::new();
    recipe.global::<Logic>().on_to_upper_case(|string: SharedString| {
        string.as_str().to_uppercase().into()
    });
    // ...
}
```
</details>

<details  data-snippet-language="cpp">
<summary>C++ code</summary>
In C++ you can set the callback like this:

```cpp
int main(int argc, char **argv)
{
    auto recipe = Recipe::create();
    recipe->global<Logic>().on_to_uppercase([](SharedString str) -> SharedString {
        std::string arg(str);
        std::transform(arg.begin(), arg.end(), arg.begin(), toupper);
        return SharedString(arg);
    });
    // ...
}
```
</details>


<!--

more content:

## Input Events

### Keyboard Input

Receive key events, pass them to native code

### Mouse and Touch Input

### Flickable

-->
