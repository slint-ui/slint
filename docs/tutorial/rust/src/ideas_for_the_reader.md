# Ideas For The Reader

The game is visually a little bare. Here are some ideas how you could make further changes to enhance it:

-   The tiles could have rounded corners, to look a little less sharp. The [border-radius](https://slint.dev/docs/slint/src/builtins/elements.html#rectangle)
    property of _Rectangle_ can be used to achieve that.

-   In real world memory games, the back of the tiles often have some common graphic. You could add an image with
    the help of another _[Image](https://slint.dev/docs/slint/src/builtins/elements.html#image)_
    element. Note that you may have to use _Rectangle_'s _clip property_
    element around it to ensure that the image is clipped away when the curtain effect opens.

Let us know in the comments on [Github Discussions](https://github.com/slint-ui/slint/discussions)
how you polished your code, or feel free to ask questions about how to implement something.
