<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
<!-- cSpell: ignore dgettext ngettext xgettext -->


# Custom Control Introduction

## A Clickable Button

```slint,no-auto-preview
import { VerticalBox, Button } from "std-widgets.slint";
export component Recipe inherits Window {
    in-out property <int> counter: 0;
    VerticalBox {
        button := Button {
            text: "Button, pressed " + root.counter + " times";
            clicked => {
                root.counter += 1;
            }
        }
    }
}
```

In this first example, you see the basics of the Slint language:

 - We import the `VerticalBox` layout and the `Button` widget from the standard library
   using the `import` statement. This statement can import widgets or your own components
   declared in different files. You don't need to import built-in element such as `Window` or `Rectangle`.
 - We declare the `Recipe` component using the `component` keyword. `Recipe` inherits from `Window`
   and has elements: A layout (`VerticalBox`) with one button.
 - You instantiate elements using their name followed by a pair of braces (with optional contents.
   You can assign a name to a specific element using `:=`
 - Elements have properties. Use `:` to set property values. Here we assign a
   binding that computes a string by concatenating some string literals, and the
   `counter` property to the `Button`'s `text` property.
 - You can declare custom properties for any element with `property <...>`. A
   property needs to have a type, and can have a default value and an access
   specifier. Access specifiers like `private`, `in`, `out` or `in-out` defines
   how outside elements can interact with the property. `Private` is the default
   value and stops any outside element from accessing the property.
   The `counter` property is custom in this example.
 - Elements can also have callback. In this case we assign a callback
   handler to the `clicked` callback of the `button` with `=> { ... }`.
 - Property bindings are automatically re-evaluated if any of the properties the
   binding depends on changes. The `text` binding of the button is
   automatically re-computed whenever the `counter` changes.

## React to a Button Click in Native Code

This example increments the `counter` using native code:

```slint,no-preview
import { VerticalBox, Button } from "std-widgets.slint";
export component Recipe inherits Window {
    in-out property <int> counter: 0;
    callback button-pressed <=> button.clicked;
    VerticalBox {
        button := Button {
            text: "Button, pressed " + root.counter + " times";
        }
    }
}
```

The `<=>` syntax binds two callbacks together. Here the new `button-pressed`
callback binds to `button.clicked`.

The root element of the main component exposes all non-`private` properties and
callbacks to native code.

In Slint, `-` and `_` are equivalent and interchangable in all identifiers.
This is different in native code: Most programming languages forbid `-` in
identifiers, so `-` is replaced with `_`.

<details data-snippet-language="rust">
<summary>Rust code</summary>

For technical reasons, this example uses `import {Recipe}` in the `slint!` macro.
In real code, you can put the whole Slint code in the `slint!` macro, or use
an external `.slint` file together with a build script.


```rust,no_run
slint::slint!(import { Recipe } from "docs/reference/src/recipes/button_native.slint";);

fn main() {
    let recipe = Recipe::new().unwrap();
    let recipe_weak = recipe.as_weak();
    recipe.on_button_pressed(move || {
        let recipe = recipe_weak.upgrade().unwrap();
        let mut value = recipe.get_counter();
        value = value + 1;
        recipe.set_counter(value);
    });
    recipe.run().unwrap();
}
```

The Slint compiler generates a `struct Recipe` with a getter (`get_counter`) and
a setter (`set_counter`) for each accessible property of the root element of the
`Recipe` component. It also generates a function for each accessible callback,
like in this case `on_button_pressed`.

The `Recipe` struct implements the [`slint::ComponentHandle`] trait. A component
manages a strong and a weak reference count, similar to an `Rc`.
We call the `as_weak` function to get a weak handle to the component, which we
can move into the callback.

We can't use a strong handle here, because that would form a cycle: The component
handle has ownership of the callback, which itself has ownership of the
closure's captured variables.
</details>

<details data-snippet-language="cpp">
<summary>C++ code</summary>
In C++ you can write

```cpp
#include "button_native.h"

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

The CMake integration handles the Slint compiler invocations as needed,
which will parse the `.slint` file and generate the `button_native.h` header.

This header file contains a `Recipe` class with a getter and setter for each
accessible property, as well as a function to set up a callback
for each accessible callback in `Recipe`. In this case we will have `get_counter`,
`set_counter` to access the `counter` property and `on_button_pressed` to
set up the callback.

</details>

## Use Property Bindings to Synchronize Controls

```slint,no-auto-preview
import { VerticalBox, Slider } from "std-widgets.slint";
export component Recipe inherits Window {
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

This example introduces the `Slider` widget.

It also introduces interpolation in string literals: Use `\{...}` to render
the result of code between the curly braces as a string.

# Animation Examples

## Animate the Position of an Element

```slint,no-auto-preview
import { CheckBox } from "std-widgets.slint";
export component Recipe inherits Window {
    width: 200px;
    height: 100px;

    rect := Rectangle {
        x:0;
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

Layouts position elements automatically. In this example we manually position
elements instead, using the `x`, `y`, `width`, `height` properties.

Notice the `animate x` block that specifies an animation. It's run whenever the
property changes: Either because a callback sets the property, or because
its binding value changes.

## Animation Sequence

```slint,no-auto-preview
import { CheckBox } from "std-widgets.slint";
export component Recipe inherits Window {
    width: 200px;
    height: 100px;

    rect := Rectangle {
        x:0;
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

This example uses the `delay` property to make one animation run after another.

# States Examples

## Associate Property Values With States

```slint,no-auto-preview
import { HorizontalBox, VerticalBox, Button } from "std-widgets.slint";

component Circle inherits Rectangle {
    width: 30px;
    height: 30px;
    border-radius: root.width / 2;
    animate x { duration: 250ms; easing: ease-in; }
    animate y { duration: 250ms; easing: ease-in-out; }
    animate background { duration: 250ms; }
}

export component Recipe inherits Window {
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
            max-height: self.min-height;
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

            circle1 := Circle { y:0; background: green; x: 85px; }
            circle2 := Circle { background: green; x: 85px; y: 40px; }
        }
    }
}
```

## Transitions

```slint,no-auto-preview
import { HorizontalBox, VerticalBox, Button } from "std-widgets.slint";

component Circle inherits Rectangle {
    width: 30px;
    height: 30px;
    border-radius: root.width / 2;
}

export component Recipe inherits Window {
    states [
        left-aligned when b1.pressed: {
            circle1.x: 0px; circle1.y: 40px;
            circle2.x: 0px; circle2.y: 0px;
            in {
                animate circle1.x, circle2.x { duration: 250ms; }
            }
            out {
                animate circle1.x, circle2.x { duration: 500ms; }
            }
        }
        right-aligned when !b1.pressed: {
            circle1.x: 170px; circle1.y: 70px;
            circle2.x: 170px; circle2.y: 00px;
        }
    ]

    VerticalBox {
        HorizontalBox {
            max-height: self.min-height;
            b1 := Button {
                text: "Press and hold to change state";
            }
        }
        Rectangle {
            background: root.background.darker(20%);
            width: 250px;
            height: 100px;

            circle1 := Circle { y:0; background: green; x: 85px; }
            circle2 := Circle { background: blue; x: 85px; y: 40px; }
        }
    }
}
```

# Layout Examples

## Vertical

```slint,no-auto-preview
import { VerticalBox, Button } from "std-widgets.slint";
export component Recipe inherits Window {
    VerticalBox {
        Button { text: "First"; }
        Button { text: "Second"; }
        Button { text: "Third"; }
    }
}
```

## Horizontal

```slint,no-auto-preview
import { HorizontalBox, Button } from "std-widgets.slint";
export component Recipe inherits Window {
    HorizontalBox {
        Button { text: "First"; }
        Button { text: "Second"; }
        Button { text: "Third"; }
    }
}
```

## Grid

```slint,no-auto-preview
import { GridBox, Button, Slider } from "std-widgets.slint";
export component Recipe inherits Window {
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

# Global Callbacks

## Invoke a Globally Registered Native Callback from Slint

This example uses a global singleton to implement common logic in native code.
This singleton may also store properties that are accessible to native code.

Note: The preview visualize the Slint code only. It's not connected to the native code.

```slint,no-preview
import { HorizontalBox, VerticalBox, LineEdit } from "std-widgets.slint";

export global Logic  {
    pure callback to-upper-case(string) -> string;
    // You can collect other global properties here
}

export component Recipe inherits Window {
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
import { HorizontalBox, VerticalBox, LineEdit } from "std-widgets.slint";

export global Logic {
    pure callback to-upper-case(string) -> string;
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
    let recipe = Recipe::new().unwrap();
    recipe.global::<Logic>().on_to_upper_case(|string| {
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
    recipe->global<Logic>().on_to_upper_case([](slint::SharedString str) -> slint::SharedString {
        std::string arg(str);
        std::transform(arg.begin(), arg.end(), arg.begin(), toupper);
        return slint::SharedString(arg);
    });
    // ...
}
```
</details>

<details  data-snippet-language="javascript">
<summary>JavaScript code</summary>
In JavaScript you can set the callback like this:

```js
let slint = require("slint-ui");
let file = slint.loadFile("recipe.slint");
let recipe = new file.Recipe();
recipe.Logic.to_upper_case = (str) => {
    return str.toUpperCase();
};
// ...
```
</details>

# Custom Widgets

## Custom Button

```slint,no-auto-preview
component Button inherits Rectangle {
    in-out property text <=> txt.text;
    callback clicked <=> touch.clicked;
    border-radius: root.height / 2;
    border-width: 1px;
    border-color: root.background.darker(25%);
    background: touch.pressed ? #6b8282 : touch.has-hover ? #6c616c :  #456;
    height: txt.preferred-height * 1.33;
    min-width: txt.preferred-width + 20px;
    txt := Text {
        x: (parent.width - self.width)/2 + (touch.pressed ? 2px : 0);
        y: (parent.height - self.height)/2 + (touch.pressed ? 1px : 0);
        color: touch.pressed ? #fff : #eee;
    }
    touch := TouchArea { }
}

export component Recipe inherits Window {
    VerticalLayout {
        alignment: start;
        Button { text: "Button"; }
    }
}
```

## ToggleSwitch

```slint,no-auto-preview
export component ToggleSwitch inherits Rectangle {
    callback toggled;
    in-out property <string> text;
    in-out property <bool> checked;
    in-out property<bool> enabled <=> touch-area.enabled;
    height: 20px;
    horizontal-stretch: 0;
    vertical-stretch: 0;

    HorizontalLayout {
        spacing: 8px;
        indicator := Rectangle {
            width: 40px;
            border-width: 1px;
            border-radius: root.height / 2;
            border-color: self.background.darker(25%);
            background: root.enabled ? (root.checked ? blue: white)  : white;
            animate background { duration: 100ms; }

            bubble := Rectangle {
                width: root.height - 8px;
                height: bubble.width;
                border-radius: bubble.height / 2;
                y: 4px;
                x: 4px + self.a * (indicator.width - bubble.width - 8px);
                property <float> a: root.checked ? 1 : 0;
                background: root.checked ? white : (root.enabled ? blue : gray);
                animate a, background { duration: 200ms; easing: ease;}
            }
        }

        Text {
            min-width: max(100px, self.preferred-width);
            text: root.text;
            vertical-alignment: center;
            color: root.enabled ? black : gray;
        }

    }

    touch-area := TouchArea {
        width: root.width;
        height: root.height;
        clicked => {
            if (root.enabled) {
                root.checked = !root.checked;
                root.toggled();
            }
        }
    }
}

export component Recipe inherits Window {
    VerticalLayout {
        alignment: start;
        ToggleSwitch { text: "Toggle me"; }
        ToggleSwitch { text: "Disabled"; enabled: false; }
    }
}
```

## CustomSlider

The `TouchArea` is covering the entire widget, so you can drag this slider from
any point within itself.

```slint,no-auto-preview
import { VerticalBox } from "std-widgets.slint";

export component MySlider inherits Rectangle {
    in-out property<float> maximum: 100;
    in-out property<float> minimum: 0;
    in-out property<float> value;

    min-height: 24px;
    min-width: 100px;
    horizontal-stretch: 1;
    vertical-stretch: 0;

    border-radius: root.height/2;
    background: touch.pressed ? #eee: #ddd;
    border-width: 1px;
    border-color: root.background.darker(25%);

    handle := Rectangle {
        width: self.height;
        height: parent.height;
        border-width: 3px;
        border-radius: self.height / 2;
        background: touch.pressed ? #f8f: touch.has-hover ? #66f : #0000ff;
        border-color: self.background.darker(15%);
        x: (root.width - handle.width) * (root.value - root.minimum)/(root.maximum - root.minimum);
    }
    touch := TouchArea {
        property <float> pressed-value;
        pointer-event(event) => {
            if (event.button == PointerEventButton.left && event.kind == PointerEventKind.down) {
                self.pressed-value = root.value;
            }
        }
        moved => {
            if (self.enabled && self.pressed) {
                root.value = max(root.minimum, min(root.maximum,
                    self.pressed-value + (touch.mouse-x - touch.pressed-x) * (root.maximum - root.minimum) / (root.width - handle.width)));

            }
        }
    }
}

export component Recipe inherits Window {
    VerticalBox {
        alignment: start;
        slider := MySlider {
            maximum: 100;
        }
        Text {
            text: "Value: \{round(slider.value)}";
        }
    }
}
```

This example show another implementation that has a drag-able handle:
The handle only moves when we click on that handle.
The TouchArea is within the handle and moves with the handle.

```slint,no-auto-preview
import { VerticalBox } from "std-widgets.slint";

export component MySlider inherits Rectangle {
    in-out property<float> maximum: 100;
    in-out property<float> minimum: 0;
    in-out property<float> value;

    min-height: 24px;
    min-width: 100px;
    horizontal-stretch: 1;
    vertical-stretch: 0;

    border-radius: root.height/2;
    background: touch.pressed ? #eee: #ddd;
    border-width: 1px;
    border-color: root.background.darker(25%);

    handle := Rectangle {
        width: self.height;
        height: parent.height;
        border-width: 3px;
        border-radius: self.height / 2;
        background: touch.pressed ? #f8f: touch.has-hover ? #66f : #0000ff;
        border-color: self.background.darker(15%);
        x: (root.width - handle.width) * (root.value - root.minimum)/(root.maximum - root.minimum);

        touch := TouchArea {
            moved => {
                if (self.enabled && self.pressed) {
                    root.value = max(root.minimum, min(root.maximum,
                        root.value + (self.mouse-x - self.pressed-x) * (root.maximum - root.minimum) / root.width));
                }
            }
        }
    }
}

export component Recipe inherits Window {
    VerticalBox {
        alignment: start;
        slider := MySlider {
            maximum: 100;
        }
        Text {
            text: "Value: \{round(slider.value)}";
        }
    }
}
```

## Custom Tabs

Use this recipe as a basis to when you want to create your own custom tab widget.

```slint,no-auto-preview
import { Button } from "std-widgets.slint";

export component Recipe inherits Window {
    preferred-height: 200px;
    in-out property <int> active-tab;
    VerticalLayout {
        tab_bar := HorizontalLayout {
            spacing: 3px;
            Button {
                text: "Red";
                clicked => { root.active-tab = 0; }
            }
            Button {
                text: "Blue";
                clicked => { root.active-tab = 1; }
            }
            Button {
                text: "Green";
                clicked => { root.active-tab = 2; }
            }
        }
        Rectangle {
            clip: true;
            Rectangle {
                background: red;
                x: root.active-tab == 0 ? 0 : root.active-tab < 0 ? - self.width - 1px : parent.width + 1px;
                animate x { duration: 125ms; easing: ease; }
            }
            Rectangle {
                background: blue;
                x: root.active-tab == 1 ? 0 : root.active-tab < 1 ? - self.width - 1px : parent.width + 1px;
                animate x { duration: 125ms; easing: ease; }
            }
            Rectangle {
                background: green;
                x: root.active-tab == 2 ? 0 : root.active-tab < 2 ? - self.width - 1px : parent.width + 1px;
                animate x { duration: 125ms; easing: ease; }
            }
        }
    }
}
```

## Custom Table View

Slint provides a table widget, but you can also do something custom based on a
`ListView`.

```slint,no-auto-preview
import { VerticalBox, ListView } from "std-widgets.slint";

component TableView inherits Rectangle {
    in property <[string]> columns;
    in property <[[string]]> values;

    private property <length> e: self.width / root.columns.length;
    private property <[length]> column_sizes: [
        root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e,
        root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e,
        root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e, root.e,
    ];

    VerticalBox {
        padding: 5px;
        HorizontalLayout {
            padding: 5px; spacing: 5px;
            vertical-stretch: 0;
            for title[idx] in root.columns : HorizontalLayout {
                width: root.column_sizes[idx];
                Text { overflow: elide; text: title; }
                Rectangle {
                    width: 1px;
                    background: gray;
                    TouchArea {
                        width: 10px;
                        x: (parent.width - self.width) / 2;
                        property <length> cached;
                        pointer-event(event) => {
                            if (event.button == PointerEventButton.left && event.kind == PointerEventKind.down) {
                                self.cached = root.column_sizes[idx];
                            }
                        }
                        moved => {
                            if (self.pressed) {
                                root.column_sizes[idx] += (self.mouse-x - self.pressed-x);
                                if (root.column_sizes[idx] < 0) {
                                    root.column_sizes[idx] = 0;
                                }
                            }
                        }
                        mouse-cursor: ew-resize;
                    }
                }
            }
        }
        ListView {
            for r in root.values : HorizontalLayout {
                padding: 5px;
                spacing: 5px;
                for t[idx] in r : HorizontalLayout {
                    width: root.column_sizes[idx];
                    Text { overflow: elide; text: t; }
                }
            }
        }
    }
}

export component Example inherits Window {
   TableView {
       columns: ["Device", "Mount Point", "Total", "Free"];
       values: [
            ["/dev/sda1", "/", "255GB", "82.2GB"] ,
            ["/dev/sda2", "/tmp", "60.5GB", "44.5GB"] ,
            ["/dev/sdb1", "/home", "255GB", "32.2GB"] ,
       ];
   }
}
```

## Breakpoints for Responsive User Interfaces

Use recipe implements a responsive SideBar that collapses when the parent
width is smaller than the given break-point. When clicking the Button, the
SideBar expands again. Use the blue Splitter to resize the container and
test the responsive behavior.

```slint,no-auto-preview
import { Button, Palette } from "std-widgets.slint";

export component SideBar inherits Rectangle {
    private property <bool> collapsed: root.reference-width < root.break-point;

    /// Defines the reference width to check `break-point`.
    in-out property <length> reference-width;

    /// If `reference-width` is less `break-point` the `SideBar` collapses.
    in-out property <length> break-point: 600px;

    /// Set the text of the expand button.
    in-out property <string> expand-button-text;

    width: 160px;

    container := Rectangle {
        private property <bool> expaned;

        width: parent.width;
        background: Palette.background.darker(0.2);

        VerticalLayout {
            padding: 2px;
            alignment: start;

            HorizontalLayout {
                alignment: start;

                if (root.collapsed) : Button {
                    checked: container.expaned;
                    text: root.expand-button-text;

                    clicked => {
                        container.expaned = !container.expaned;
                    }
                }
            }

            @children
        }

        states [
            expaned when container.expaned && root.collapsed : {
                width: 160px;

                in {
                    animate width { duration: 200ms; }
                }
                out {
                    animate width { duration: 200ms; }
                }
                in {
                        animate width { duration: 200ms; }
                }
                out {
                        animate width { duration: 200ms; }
                }
            }
        ]
    }

    states [
        collapsed when root.collapsed : {
            width: 62px;
        }
    ]
}

component Splitter inherits TouchArea {
    width: 4px;
    mouse-cursor: ew-resize;

    Rectangle {
        width: 100%;
        height: 100%;
        background: blue;
    }
}

export component SideBarTest inherits Window {
    preferred-width: 700px;
    min-height: 400px;
    background: gray;

    GridLayout {
        x: 0;
        width: splitter.x;

        Rectangle {
            height: 100%;
            col: 1;
            background: white;

            HorizontalLayout {
                padding: 8px;

                Text {
                    color: black;
                    text: "Content";
                }
            }
        }
        SideBar {
            col: 0;
            reference-width: parent.width;
            expand-button-text: "E";
        }
    }

    splitter := Splitter {
        x: root.width - self.width;
        height: 100%;

        moved => {
            self.x = min(root.width - self.width, max(400px, self.x + self.mouse-x - self.pressed-x));
        }
    }
}
```

<!--

more content:

## Input Events

### Keyboard Input

Receive key events, pass them to native code

### Mouse and Touch Input

### Flickable

-->
