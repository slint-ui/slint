# Memory Tile

With the skeleton in place, let's look at the first element of the game, the memory tile. It will be the
visual building block that consists of an underlying filled rectangle background, the icon image. Later we'll add a
covering rectangle that acts as a curtain. The background rectangle is declared to be 64 logical pixels wide and tall,
and it's filled with a soothing tone of blue. Note how lengths in the `.slint` language have a unit, here
the `px` suffix. That makes the code easier to read and the compiler can detect when your are accidentally
mixing values with different units attached to them.

We copy the following code inside of the `slint!` macro:

```slint
{{#include main_memory_tile.rs:tile}}
```

Inside the <span class="hljs-built_in">Rectangle</span> we place an <span class="hljs-built_in">Image</span> element that
loads an icon with the <span class="hljs-built_in">@image-url()</span> macro.
When using the `slint!` macro, the path is relative to the folder in which the Cargo.toml is located.
When using .slint files, it's relative to the folder of the .slint file containing it.
This icon and others we're going to use later need to be installed first. You can download a
[Zip archive](https://slint.dev/blog/memory-game-tutorial/icons.zip) that we have prepared and extract it with the
following two commands:

```sh
curl -O https://slint.dev/blog/memory-game-tutorial/icons.zip
unzip icons.zip
```

This should unpack an `icons` directory containing a bunch of icons.

Running the program with `cargo run` gives us a window on the screen that shows the icon of a bus on a
blue background.

![Screenshot of the first tile](https://slint.dev/blog/memory-game-tutorial/memory-tile.png "Memory Tile Screenshot")
