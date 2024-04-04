<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Polishing the Tile

In this step, you add a curtain-like cover that opens when clicked. You do this by declaring two rectangles
below the <span class="hljs-built_in">Image</span>, so that Slint draws them after the Image and thus on top of the image.

The <span class="hljs-built_in">TouchArea</span> element declares a transparent rectangular region that allows
reacting to user input such as a mouse click or tap. The element forwards a callback to the <em>MainWindow</em> indicating that a user clicked the tile.

The <em>MainWindow</em> reacts by flipping a custom <em>open_curtain</em> property.
Property bindings for the animated width and x properties also use the custom <em>open_curtain</em> property.

The following table shows more detail on the two states:

| _open_curtain_ value:   | false                                                                        | true                                                                                                          |
| ----------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Left curtain rectangle  | Fill the left half by setting the width _width_ to half the parent's width   | Width of zero makes the rectangle invisible                                                                   |
| Right curtain rectangle | Fill the right half by setting _x_ and _width_ to half of the parent's width | _width_ of zero makes the rectangle invisible. _x_ moves to the right, sliding the curtain open when animated |

To make the tile extensible, replace the hard-coded icon name with an _icon_
property that can be set when instantiating the element.

For the final polish, add a
_solved_ property used to animate the color to a shade of green when a player finds a pair.

Replace the code inside the `ui/appwindow.slint` file with the following:

```slint
{{#include ../../rust/src/main_polishing_the_tile.rs:tile}}
```

The code uses `root` and `self`. `root` refers to the outermost
element in the component, the <span class="hljs-title">MemoryTile</span> in this case. `self` refers
to the current element.

The code exports the <span class="hljs-title">MainWindow</span> component. This is necessary so that you can later access it
from application business logic.

Running the code opens a window with a rectangle that opens up to show the bus icon when clicked. Subsequent clicks close and open the curtain again.

<video autoplay loop muted playsinline src="https://slint.dev/blog/memory-game-tutorial/polishing-the-tile.mp4"></video>
