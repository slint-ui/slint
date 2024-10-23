.. Copyright © SixtyFPS GmbH <info@slint.dev>
.. SPDX-License-Identifier: MIT

Quickstart
==========

This tutorial introduces you to the Slint UI framework in a playful way by implementing a memory game. It combines the Slint language for the graphics with the game rules implemented in C++, Rust, or NodeJS.

The game consists of a grid of 16 rectangular tiles. Clicking on a tile uncovers an icon underneath.
There are 8 different icons in total, so each tile has a sibling somewhere in the grid with the
same icon. The objective is to locate all icon pairs. The player can uncover two tiles at the same time. If they
aren't the same, the game obscures the icons again.
If the player uncovers two tiles with the same icon, then they remain visible - they're solved.

This is how the game looks in action:

.. raw:: html
   
   <video autoplay loop muted playsinline src="https://slint.dev/blog/memory-game-tutorial/memory_clip.mp4" class="img-fluid img-thumbnail rounded"></video>

.. toctree::
   :hidden:
   :maxdepth: 2
   :caption: Quickstart

   getting_started.md
   memory_tile.md   
   polishing_the_tile.md   
   from_one_to_multiple_tiles.md   
   creating_the_tiles.md   
   game_logic.md   
   running_in_a_browser.md
   ideas_for_the_reader.md   
   conclusion.md   
