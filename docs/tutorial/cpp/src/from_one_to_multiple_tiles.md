<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# From One To Multiple Tiles

After modeling a single tile, this step creates a grid of them. For the grid to be a game board, you need two features:

1. **A data model**: An array created as a C++ model, where each element describes the tile data structure, such as:

    - URL of the image
    - Whether the image is visible
    - If the player has solved this tile.

2. A way of creating multiple instances of the tiles.

With Slint you declare an array of structures based on a model using square brackets. Use a <span class="hljs-keyword">for</span> loop
to create multiple instances of the same element.

The <span class="hljs-keyword">for</span> loop is declarative and automatically updates when
the model changes. The loop instantiates all the <span class="hljs-title">MemoryTile</span> elements and places them on a grid based on their
index with spacing between the tiles.

First, add the tile data structure definition at the top of the `ui/appwindow.slint` file:

```slint
{{#include ../../rust/src/main_multiple_tiles.rs:tile_data}}
```

Next, replace the _export component <span class="hljs-title">MainWindow</span> inherits Window { ... }_ section at the bottom of the `ui/appwindow.slint` file with the following:

```slint
{{#include ../../rust/src/main_multiple_tiles.rs:main_window}}
```

The <code><span class="hljs-keyword">for</span> tile\[i\] <span class="hljs-keyword">in</span> memory_tiles:</code> syntax declares a variable `tile` which contains the data of one element from the `memory_tiles` array,
and a variable `i` which is the index of the tile. The code uses the `i` index to calculate the position of a tile, based on its row and column,
using modulo and integer division to create a 4 by 4 grid.

Running the code opens a window that shows 8 tiles, which a player can open individually.

<video autoplay loop muted playsinline src="https://slint.dev/blog/memory-game-tutorial/from-one-to-multiple-tiles.mp4"></video>
