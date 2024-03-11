<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Introduction

This tutorial introduces you to the Slint UI framework in a playful way by implementing a memory game. It combines the Slint language for the graphics with the game rules implemented in JavaScript.

The game consists of a grid of 16 rectangular tiles. Clicking on a tile uncovers an icon underneath.
There are 8 different icons in total, so each tile has a sibling somewhere in the grid with the
same icon. The objective is to locate all icon pairs. The player can uncover two tiles at the same time. If they
aren't the same, the game obscures the icons again.
If the player uncovers two tiles with the same icon, then they remain visible - they're solved.

This is how the game looks in action:

<video autoplay loop muted playsinline src="https://slint.dev/blog/memory-game-tutorial/memory_clip.mp4"
        class="img-fluid img-thumbnail rounded"></video>
