<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Game Logic In C++

This step implements the rules of the game in C++.

Slint's general philosophy is that you implement the user interface in Slint and the business logic in your favorite programming
language.

The game rules enforce that at most two tiles have their curtain open. If the tiles match, then the game
considers them solved and they remain open. Otherwise, the game waits briefly so the player can memorize
the location of the icons, and then closes the curtains again.

Add the following code inside the <span class="hljs-title">MainWindow</span> component to signal to the C++ code when the user clicks on a tile.

```slint
{{#include ../../rust/src/main_game_logic_in_rust.rs:mainwindow_interface}}
```

This change adds a way for the <span class="hljs-title">MainWindow</span> to call to the C++ code that it should
check if a player has solved a pair of tiles. The Rust code needs an additional property to toggle to disable further
tile interaction, to prevent the player from opening more tiles than allowed. No cheating allowed!

The last change to the code is to act when the <span class="hljs-title">MemoryTile</span> signals that a player clicked it.

Add the following handler in the <span class="hljs-title">MainWindow</span> `for` loop `clicked` handler:

```slint
{{#include ../../rust/src/main_game_logic_in_rust.rs:tile_click_logic}}
```

On the C++ side, you can now add a handler to the `check_if_pair_solved` callback, that checks if a player opened two tiles.
If they match, the code sets the `solved` property to true in the model. If they don't
match, start a timer that closes the tiles after one second. While the timer is running, disable every tile so
a player can't click anything during this time.

Insert this code before the `main_window->run()` call:

```cpp
{{#include main_game_logic.cpp:game_logic}}
```

The code uses a [ComponentWeakHandle](https://slint.dev/docs/cpp/api/classslint_1_1ComponentWeakHandle) pointer of the `main_window`. This is
important because capturing a copy of the `main_window` itself within the callback handler would result in circular ownership.
The `MainWindow` owns the callback handler, which itself owns a reference to the `MainWindow`, which must be weak
instead of strong to avoid a memory leak.

These were the last changes and running the code opens a window that allows a player to play the game by the rules.
