# The `.slint` language reference

The Slint design markup language is used to describe graphical user interfaces:

-   Place and compose a tree of visual elements in a window using a textual representation.
-   Configure the appearance of elements via properties. For example a `Text` element has font and text
    properties, while a `Rectangle` element offers a background color.
-   Assign binding expressions to properties to automatically compute values that depend on other properties.
-   Group binding expressions together with named states and conditions.
-   Declare animations on properties and states to make the user interface feel alive.
-   Build your own re-usable components and share them in `.slint` module files.
-   Define data structures and models and access them from programming languages.
-   Build highly customized user interfaces with the [builtin elements](builtin_elements.md) provided.

Slint also comes with a catalog of high-level [widgets](widgets.md), that are written in the `.slint`
language.
