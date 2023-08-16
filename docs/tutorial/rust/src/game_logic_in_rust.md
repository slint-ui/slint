<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Game Logic In Rust

We'll implement the rules of the game in Rust as well. The general philosophy of Slint is that merely the user
interface is implemented in the `.slint` language and the business logic in your favorite programming
language. The game rules shall enforce that at most two tiles have their curtain open. If the tiles match, then we
consider them solved and they remain open. Otherwise we wait for a little while, so the player can memorize
the location of the icons, and then close them again.

We'll modify the `.slint` markup inside the `slint!` macro to signal to the Rust code when the user clicks on a tile.
Two changes to <span class="hljs-title">MainWindow</span> are needed: We need to add a way for the MainWindow to call to the Rust code that it should
check if a pair of tiles has been solved. And we need to add a property that Rust code can toggle to disable further
tile interaction, to prevent the player from opening more tiles than allowed. No cheating allowed! First, we paste
the callback and property declarations into <span class="hljs-title">MainWindow</span>:

```slint
{{#include main_game_logic_in_rust.rs:mainwindow_interface}}
```

The last change to the `.slint` markup is to act when the <span class="hljs-title">MemoryTile</span> signals that it was clicked on.
We add the following handler in <span class="hljs-title">MainWindow</span>:

```slint
{{#include main_game_logic_in_rust.rs:tile_click_logic}}
```

On the Rust side, we can now add an handler to the `check_if_pair_solved` callback, that will check if
two tiles are opened. If they match, the `solved` property is set to true in the model. If they don't
match, start a timer that will close them after one second. While the timer is running, we disable every tile so
one can't click anything during this time.

Insert this code before the `main_window.run()` call:

```rust,noplayground
{{#include main_game_logic_in_rust.rs:game_logic}}
```

Notice that we take a [Weak](https://slint.dev/docs/rust/slint/struct.Weak) pointer of our `main_window`. This is very
important because capturing a copy of the `main_window` itself within the callback handler would result in a circular ownership.
The `MainWindow` owns the callback handler, which itself owns a reference to the `MainWindow`, which must be weak
instead of strong to avoid a memory leak.

These were the last changes and running the result gives us a window on the screen that allows us
to play the game by the rules.
