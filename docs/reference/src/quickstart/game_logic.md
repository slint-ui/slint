<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Game Logic

This step implements the rules of the game in your coding language of choice.

Slint's general philosophy is that you implement the user interface in Slint and the business logic in your favorite programming
language.

The game rules enforce that at most two tiles have their curtain open. If the tiles match, then the game
considers them solved and they remain open. Otherwise, the game waits briefly so the player can memorize
the location of the icons, and then closes the curtains again.

:::::{tab-set}
::::{tab-item} C++
:sync: cpp

Add the following code inside the <span class="hljs-title">MainWindow</span> component to signal to the C++ code when the user clicks on a tile.

:::{literalinclude} main_game_logic_in_rust.rs
:language: slint,no-preview
:lines: 107-115
:::

This change adds a way for the <span class="hljs-title">MainWindow</span> to call to the C++ code that it should
check if a player has solved a pair of tiles. The Rust code needs an additional property to toggle to disable further
tile interaction, to prevent the player from opening more tiles than allowed. No cheating allowed!

The last change to the code is to act when the <span class="hljs-title">MemoryTile</span> signals that a player clicked it.

Add the following handler in the <span class="hljs-title">MainWindow</span> `for` loop `clicked` handler:

:::{literalinclude} main_game_logic_in_rust.rs
:language: slint,no-preview
:lines: 126-143
:::


On the C++ side, you can now add a handler to the `check_if_pair_solved` callback, that checks if a player opened two tiles.
If they match, the code sets the `solved` property to true in the model. If they don't
match, start a timer that closes the tiles after one second. While the timer is running, disable every tile so
a player can't click anything during this time.

Insert this code before the `main_window->run()` call:

:::{literalinclude} main_game_logic.cpp
:lines: 29-65
:::

The code uses a [ComponentWeakHandle](https://slint.dev/docs/cpp/api/classslint_1_1ComponentWeakHandle) pointer of the `main_window`. This is
important because capturing a copy of the `main_window` itself within the callback handler would result in circular ownership.
The `MainWindow` owns the callback handler, which itself owns a reference to the `MainWindow`, which must be weak
instead of strong to avoid a memory leak.

::::

::::{tab-item} NodeJS
:sync: nodejs

Change the contents of `memory.slint` to signal to the JavaScript code when the user clicks on a tile.

:::{literalinclude} main_game_logic_in_rust.rs
:language: slint,no-preview
:lines: 107-115
:::

This change adds a way for the <span class="hljs-title">MainWindow</span> to call to the JavaScript code that it should
check if a player has solved a pair of tiles. The Rust code needs an additional property to toggle to disable further
tile interaction, to prevent the player from opening more tiles than allowed. No cheating allowed!

The last change to the code is to act when the <span class="hljs-title">MemoryTile</span> signals that a player clicked it.

Add the following handler in the <span class="hljs-title">MainWindow</span> `for` loop `clicked` handler:

:::{literalinclude} main_game_logic_in_rust.rs
:lines: 126-143
:::

On the JavaScript side, now add a handler to the `check_if_pair_solved` callback, that checks if a player opened two tiles. If they match, the code sets the `solved` property to true in the model. If they don't
match, start a timer that closes the tiles after one second. While the timer is running, disable every tile so
a player can't click anything during this time.

Insert this code before the `mainWindow.run()` call:

:::{literalinclude} main_game_logic.js
:lines: 23-63
:::

::::

::::{tab-item} Rust
:sync: rust
:selected: true

Add the following code inside the <span class="hljs-title">MainWindow</span> component to signal to the Rust code when the user clicks on a tile.

:::{literalinclude} main_game_logic_in_rust.rs
:lines: 107-115
:::

This change adds a way for the <span class="hljs-title">MainWindow</span> to call to the Rust code that it should
check if a player has solved a pair of tiles. The Rust code needs an additional property to toggle to disable further
tile interaction, to prevent the player from opening more tiles than allowed. No cheating allowed!

The last change to the code is to act when the <span class="hljs-title">MemoryTile</span> signals that a player clicked it.

Add the following handler in the <span class="hljs-title">MainWindow</span> `for` loop `clicked` handler:

:::{literalinclude} main_game_logic_in_rust.rs
:lines: 126-143
:::

On the Rust side, you can now add a handler to the `check_if_pair_solved` callback, that checks if a player opened two tiles.
If they match, the code sets the `solved` property to true in the model. If they don't
match, start a timer that closes the tiles after one second. While the timer is running, disable every tile so
a player can't click anything during this time.

Add this code before the `main_window.run().unwrap();` call:

:::{literalinclude} main_game_logic_in_rust.rs
:lines: 25-52
:::

The code uses a [Weak](https://slint.dev/docs/rust/slint/struct.Weak) pointer of the `main_window`. This is
important because capturing a copy of the `main_window` itself within the callback handler would result in circular ownership.
The `MainWindow` owns the callback handler, which itself owns a reference to the `MainWindow`, which must be weak
instead of strong to avoid a memory leak.

::::

:::::

These were the last changes and running the code opens a window that allows a player to play the game by the rules.
