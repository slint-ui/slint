<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Memory Tile

With the skeleton code in place, this step looks at the first element of the game, the memory tile. It's the
visual building block that consists of an underlying filled rectangle background, the icon image. Later steps add a covering rectangle that acts as a curtain.

You declare the background rectangle as 64 logical pixels wide and tall
filled with a soothing tone of blue.

Lengths in Slint have a unit, here, the `px` suffix.
This makes the code easier to read and the compiler can detect when you accidentally
mix values with different units attached to them.

Copy the following code into `ui/appwindow.slint` file, replacing the current content:

```slint
{{#include memory_tile.slint:main_window}}
```

The code exports the <span class="hljs-title">MainWindow</span> component so that the C++ code can access it later.

Inside the <span class="hljs-built_in">Rectangle</span> place an <span class="hljs-built_in">Image</span> element that
loads an icon with the <span class="hljs-built_in">@image-url()</span> macro. The path is relative to the location of `ui/appwindow.slint`.

You need to install this icon and others you use later first. You can download a pre-prepared
[Zip archive](https://slint.dev/blog/memory-game-tutorial/icons.zip) to the `ui` folder,

If you are on Linux or macOS, download and extract it with the following commands:

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

Compiling the program with `cmake --build build` and running with the `./build/my_application` opens a window that shows the icon of a bus on a blue background.

![Screenshot of the first tile](https://slint.dev/blog/memory-game-tutorial/memory-tile.png "Memory Tile Screenshot")
