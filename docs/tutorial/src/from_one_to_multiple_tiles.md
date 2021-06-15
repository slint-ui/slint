# From One To Multiple Tiles

After modeling a single tile, let's create a grid of them. For the grid to be our game board, we need two features:
    
1. A data model: This shall be an array where each element describes the tile data structure, such as the
   url of the image, whether the image shall be visible and if this tile has been solved. We modify the model
   from Rust code.
1. A way of creating many instances of the tiles, with the above `.60` markup code.
    
In SixtyFPS we can declare an array of structures using brackets, to create a model. We can use the `for` loop
to create many instances of the same element. In `.60` the for loop is declarative and automatically updates when
the model changes. We instantiate all the different `MemoryTile` elements and place them on a grid based on their
index with a little bit of spacing between the tiles.

First, we copy the tile data structure definition and paste it at top inside the `sixtyfps!` macro:

```60
sixtyfps::sixtyfps!{

// Added:
struct TileData := {
    image: image,
    image_visible: bool,
    solved: bool,
}

MemoryTile := Rectangle {
// ...
```

Next, we replace the *`MainWindow` := { ... }* section at the bottom of the `sixtyfps!` macro with the following snippet:

```60
MainWindow := Window {
    width: 326px;
    height: 326px;

    property <[TileData]> memory_tiles: [
       { image: @image-url("icons/at.png") },
       { image: @image-url("icons/balance-scale.png") },
       { image: @image-url("icons/bicycle.png") },
       { image: @image-url("icons/bus.png") },
       { image: @image-url("icons/cloud.png") },
       { image: @image-url("icons/cogs.png") },
       { image: @image-url("icons/motorcycle.png") },
       { image: @image-url("icons/video.png") },
    ];
    for tile[i] in memory_tiles : MemoryTile {
        x: mod(i, 4) * 74px;
        y: floor(i / 4) * 74px;
        width: 64px;
        height: 64px;
        icon: tile.image;
        open_curtain: tile.image_visible || tile.solved;
        // propagate the solved status from the model to the tile
        solved: tile.solved;
        clicked => {
            tile.image_visible = !tile.image_visible;
        }
    }
}
```

The `for tile[i] in memory_tiles :` syntax declares a variable `tile` which contains the data of one element from the `memory_tiles` array,
and a variable `i` which is the index of the tile. We use the `i` index to calculate the position of tile based on its row and column,
using the modulo and integer division to create a 4 by 4 grid.

Running this gives us a window that shows 8 tiles, which can be opened individually.

<video autoplay loop muted playsinline src="https://sixtyfps.io/blog/memory-game-tutorial/from-one-to-multiple-tiles.mp4"></video>
