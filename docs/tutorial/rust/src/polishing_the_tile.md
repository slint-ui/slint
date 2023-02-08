# Polishing the Tile

Next, let's add a curtain like cover that opens up when clicking. We achieve this by declaring two rectangles
below the <span class="hljs-built_in">Image</span>, so that they are drawn afterwards and thus on top of the image.
The <span class="hljs-built_in">TouchArea</span> element declares a transparent rectangular region that allows
reacting to user input such as a mouse click or tap. We use that to forward a callback to the <em>MainWindow</em>
that the tile was clicked on. In the <em>MainWindow</em> we react by flipping a custom <em>open_curtain</em> property.
That in turn is used in property bindings for the animated width and x properties. Let's look at the two states a bit
more in detail:

| *open_curtain* value: | false | true |
| --- | --- | --- |
| Left curtain rectangle | Fill the left half by setting the width *width* to half the parent's width   | Width of zero makes the rectangle invisible                                                                       |
| Right curtain rectangle | Fill the right half by setting *x* and *width* to half of the parent's width | *width* of zero makes the rectangle invisible. *x* is moved to the right, to slide the curtain open when animated |

In order to make our tile extensible, the hard-coded icon name is replaced with an *icon*
property that can be set from the outside when instantiating the element. For the final polish, we add a
*solved* property that we use to animate the color to a shade of green when we've found a pair, later. We
replace the code inside the `slint!` macro with the following:

```slint
{{#include main_polishing_the_tile.rs:tile}}
```

Note the use of `root` and `self` in the code. `root` refers to the outermost
element in the component, that's the <span class="hljs-title">MemoryTile</span> in this case. `self` refers
to the current element.

Note that we export the <span class="hljs-title">MainWindow</span> component. This is necessary so that we can later access it
from our business logic.

Running this gives us a window on the screen with a rectangle that opens up to show us the bus icon, when clicking on
it. Subsequent clicks will close and open the curtain again.

<video autoplay loop muted playsinline src="https://slint-ui.com/blog/memory-game-tutorial/polishing-the-tile.mp4"></video>
