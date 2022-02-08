# Ideas For The Reader

The game is visually a little bare. Here are some ideas how you could make further changes to enhance it:

* The tiles could have rounded corners, to look a little less sharp. The [border-radius](https://slint-ui.com/docs/rust/sixtyfps/docs/builtin_elements/index.html#rectangle)
  property of *Rectangle* can be used to achieve that.

* In real world memory games, the back of the tiles often have some common graphic. You could add an image with
  the help of another *[Image](https://slint-ui.com/docs/rust/sixtyfps/docs/builtin_elements/index.html#image)*
  element. Note that you may have to use *Rectangle*'s *[clip](https://slint-ui.com/docs/rust/sixtyfps/docs/builtin_elements/index.html#properties-1) property*
  element around it to ensure that the image is clipped away when the curtain effect opens.

Let us know in the comments on Github Discussions how you polished your code, or feel free to ask questions about
how to implement something.
