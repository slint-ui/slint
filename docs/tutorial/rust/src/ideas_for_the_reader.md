<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Ideas For The Reader

The game is visually bare. Here are some ideas on how you could make further changes to enhance it:

-   The tiles could have rounded corners, to look less sharp. Use the [border-radius](https://slint.dev/docs/slint/src/language/builtins/elements#rectangle)
    property of _[Rectangle](https://slint.dev/docs/slint/src/language/builtins/elements#rectangle)_ to achieve that.

-   In real-world memory games, the back of the tiles often have some common graphic. You could add an image with
    the help of another _[Image](https://slint.dev/docs/slint/src/language/builtins/elements#image)_
    element. Note that you may have to use _Rectangle_'s _clip property_
    element around it to ensure that the image is clipped away when the curtain effect opens.

Let us know in the comments on [Github Discussions](https://github.com/slint-ui/slint/discussions)
how you polished your code, or feel free to ask questions about how to implement something.
