<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Memory Tile

With the skeleton in place, this step looks at the first element of the game, the memory tile. It's the
visual building block that consists of an underlying filled rectangle background, the icon image. Later you'll add a
covering rectangle that acts as a curtain.

You declare the background rectangle as 64 logical pixels wide and tall
and it's filled with a soothing tone of blue.

Note how lengths in the `.slint` language have a unit, here
the `px` suffix. That makes the code easier to read and the compiler can detect when you accidentally
mix values with different units attached to them.

Copy the following into the `ui/appwindow.slint` file:

```slint
{{#include memory_tile.slint:main_window}}
```

The code exports the <span class="hljs-title">MainWindow</span> component so that the business logic can access it later.

Inside the <span class="hljs-built_in">Rectangle</span> place an <span class="hljs-built_in">Image</span> element that
loads an icon with the <span class="hljs-built_in">@image-url()</span> macro.

The path is relative to the folder that contains `ui/appwindow.slint`. You need to install this icon and others you use later first. You can download a pre-prepared
[Zip archive](https://slint.dev/blog/memory-game-tutorial/icons.zip) and extract it with the
following two commands:

If you are on Linux or macOS, download and extract it with the following two commands:

```sh
cd ui
curl -O https://slint.dev/blog/memory-game-tutorial/icons.zip
unzip icons.zip
cd ..
```

If you are on Windows, use the following commands:

```sh
cd ui
powershell curl -Uri https://slint.dev/blog/memory-game-tutorial/icons.zip -Outfile icons.zip
powershell Expand-Archive -Path icons.zip -DestinationPath .
cd ..
```

This unpacks an `icons` directory containing several icons.

Running the program with `npm start` now opens a window that shows the icon of a bus on a
blue background.

![Screenshot of the first tile](https://slint.dev/blog/memory-game-tutorial/memory-tile.png "Memory Tile Screenshot")
