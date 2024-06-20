<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Functions

Similarly to other programming languages, functions in Slint are way to name, organize and reuse
a piece of logic/code.

Functions can be defined as part of a component, or as part of an element within a component. Functions
are always part of a component: it is not possible to declare global (top-level) functions, or to
declare them as part of a struct or enum. It is also not possible to nest functions within other
functions.

## Declaring a function

Functions in Slint are declared using the `function` keyword. For example:

```slint,no-preview
function my-function(parameter: int) -> string {
    // Function code goes here
    return "result"
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

A function can be called without a target (like a function call in other languages) or with
a target (like a method call in other languages):

```slint,no-preview
export component Example {
    property <string> my-property: my-function(); // Called without a target
    property <int> my-other-property: my_button.my-other-function(); // Called with a named target

    pure function my-function() -> string {
        return "result";
    }

    Text {
        text: root.my-function(); // Called with a pre-defined name
    }

    my_button := Button {
        pure function my-other-function() -> int {
            return 42;
        }
    }
}
```

The two types of calls function mostly the same. The only differences are:

- Functions can be called without a target only from the element that defines them and its children.
That means a function defined at the root of a component can be called from anywhere within that component
while a function defined in an element can be called from that element and any child elements of it.
- When a function is overridden, calling it with a target allows the caller to choose the exact version
it wants to call (see Function Overriding below).

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

    my-friend: HasFunction {
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

    my-friend: HasFunction {
    }
}
```

- A function annotated with `protected` can only be accessed by components that directly inherit from it.

Functions, even marked public, cannot be exported and cannot be called from backend code (in Rust, C++,
JS, etc.). Declare a [callback](callbacks.md) which can call the function.

## Function overriding

Function names must be unique at any given level in the component hierarchy. But two functions with the
same name can be declared in different elements. For example:

```slint,no-preview
export component Example {
    property <int> secret_number: my-function();
    public pure function my-function() -> int {
        return 1;
    }

    VerticalBox {
        public pure function my-function() -> int {
            return 2;
        }

        Text {
            text: "The secret number is " + my-function();
            public pure function my-function() -> int {
                return 3;
            }
        }
    }
}
```

When calling an overidden function without a target, the version that is called depends on where in the
element hierarchy the call happens. The compiler first looks for a function defined in the same element,
then, if none is found, goes up the parent chain of that element stopping at the first node that has a
function by that name.

In the example above, the property `secret_number` will be set to 1, but the text label will be set to
3.
