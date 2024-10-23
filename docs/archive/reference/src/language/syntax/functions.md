<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Functions

Similar to other programming languages, functions in Slint are way to name, organize and reuse
a piece of logic/code.

Functions can be defined as part of a component, or as part of an element within a component.
It is not possible to declare global (top-level) functions, or to declare them as part of a
struct or enum. It is also not possible to nest functions within other functions.

## Declaring functions

Functions in Slint are declared using the `function` keyword. For example:

```slint,no-preview
export component Example {
    // ...
    function my-function(parameter: int) -> string {
        // Function code goes here
        return "result";
    }
}
```

Functions can have parameters which are declared within parentheses, following the format `name: type`.
These parameters can be referenced by their names within the function body. Parameters are passed by
value.

Functions can also return a value. The return type is specified after `->` in the function signature.
The `return` keyword is used within the function body to return an expression of the declared type.
If a function does not explicitly return a value, the value of the last statement is returned by default.

Functions can be annotated with the `pure` keyword.
This indicates that the function does not cause any side effects.
More details can be found in the [Purity](../concepts/purity.md) chapter.

## Calling a function

A function can be called without an element name (like a function call in other languages) or with
an element name (like a method call in other languages):

```slint,no-preview
import { Button } from "std-widgets.slint"; 

export component Example {
    // Call without an element name:
    property <string> my-property: my-function();
    // Call with an element name:
    property <int> my-other-property: my_button.my-other-function();

    pure function my-function() -> string {
        return "result";
    }

    Text {
        // Called with a pre-defined element:
        text: root.my-function();
    }

    my_button := Button {
        pure function my-other-function() -> int {
            return 42;
        }
    }
}
```

## Name resolution

Function calls have the same name resolution rules as properties and callbacks. When called without
an element name:

- If the element that the function is called in (`self`) defines a function with that name, it is chosen.
- If not, name resolution continues to its parent element, and so on, until the root component.

When called with an element name (or `self`, `parent` or `root`), the function must be defined on that
element. Name resolution does not look at ancestor elements in this case. Note that this means
calling a function without an element name is _not_ equivalent to calling it with `self` (which is how methods work in many languages).

Multiple functions with the same name are allowed in the same component, as long as they are defined
on different elements. Therefore it is possible for a function to shadow another function from an ancestor
element.

```slint,no-preview
export component Example {
    property <int> secret_number: my-function();
    public pure function my-function() -> int {
        return 1;
    }

    VerticalLayout {
        public pure function my-function() -> int {
            return 2;
        }

        Text {
            text: "The secret number is " + my-function();
            public pure function my-function() -> int {
                return 3;
            }
        }

        Text {
            text: "The other secret number is " + my-function();
        }
    }
}
```

In the example above, the property `secret_number` will be set to 1, and the text labels will say "The
secret number is 3" and "The other secret number is 2".

## Function visibility

By default, functions are private and cannot be accessed from other components.

However, their accessibility can be modified using the `public` or `protected` keywords.

- A root-level function annotated with `public` can be accessed by any component.

To access such a function from a different component, you always need a target, which in practice
means the calling component must declare the called component as one of its child elements.

```slint,no-preview
export component HasFunction {
    public pure function double(x: int) -> int {
        return x * 2;
    }
}

export component CallsFunction {
    property <int> test: my-friend.double(1);

    my-friend := HasFunction {
    }
}
```

If a function is declared in a child element, even if marked public, it is not possible to call it
from another component, as the child elements themselves are not public and a valid target for the
function does not exist:

```slint,no-preview
export component HasFunction {
    t := Text {
        public pure function double(x: int) -> int {
            return x * 2;
        }
    }
}

export component CallsFunction {
    // Compiler error!
    // property <int> test: my-friend.t.double(1);

    my-friend := HasFunction {
    }
}
```

Functions marked `public` in an exported component can also be invoked from backend code (Rust, C++, JS).
See the language-specific documentation for the generated code to use.

- A function annotated with `protected` can only be accessed by components that directly inherit from it.

## Functions vs. callbacks

There are a lot of similarities between functions and [callbacks](callbacks.md):

- They are both callable blocks of logic/code
- They are invoked in the same way
- They can both have parameters and return values
- They can both be declared `pure`

But there are also differences:

- The code/logic in the callback can be set in the backend code and implemented in the backend language
  (Rust, C++, JS), while functions must be defined entirely in slint
- The syntax for defining a callback is different
- Callbacks can be declared without assigning a block of code to them
- Callbacks have a special syntax for declaring aliases using the two-way binding operator `<=>`
- Callback visiblity is always similar to `public` functions

In general, the biggest reason to use callbacks is to be able to handle them from the backend code. Use
a function if that is not needed.
