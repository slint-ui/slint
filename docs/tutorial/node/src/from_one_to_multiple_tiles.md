# From One To Multiple Tiles

After modeling a single tile, let's create a grid of them. For the grid to be our game board, we need two features:

1. A data model: This shall be an array where each element describes the tile data structure, such as the
   url of the image, whether the image shall be visible and if this tile has been solved. We modify the model
   from JS code.
2. A way of creating many instances of the tiles, with the above `.slint` markup code.

In Slint we can declare an array of structures using brackets, to create a model. We can use the <span class="hljs-keyword">for</span> loop
to create many instances of the same element. In `.slint` the for loop is declarative and automatically updates when
the model changes. We instantiate all the different <span class="hljs-title">MemoryTile</span> elements and place them on a grid based on their
index with a little bit of spacing between the tiles.

First, we copy the tile data structure definition and paste it at top inside the `memory.slint` file:

```slint
{{#include ../../rust/src/main_multiple_tiles.rs:tile_data}}
```

Next, we replace the _export component <span class="hljs-title">MainWindow</span> inherits Window { ... }_ section at the bottom of the `memory.slint` file with the following snippet:

```slint
{{#include ../../rust/src/main_multiple_tiles.rs:main_window}}
```

The <code><span class="hljs-keyword">for</span> tile\[i\] <span class="hljs-keyword">in</span> memory_tiles:</code> syntax declares a variable `tile` which contains the data of one element from the `memory_tiles` array,
and a variable `i` which is the index of the tile. We use the `i` index to calculate the position of tile based on its row and column,
using the modulo and integer division to create a 4 by 4 grid.

Running this gives us a window that shows 8 tiles, which can be opened individually.

<video autoplay loop muted playsinline src="https://slint-ui.com/blog/memory-game-tutorial/from-one-to-multiple-tiles.mp4"></video>
