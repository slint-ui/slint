<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Introduction

This tutorial will introduce you to the Slint UI framework in a playful way by implementing a little memory game. We're going to combine the `.slint` language for the graphics with the game rules implemented in Rust.

The game consists of a grid of 16 rectangular tiles. Clicking on a tile uncovers an icon underneath.
We know that there are 8 different icons in total, so each tile has a sibling somewhere in the grid with the
same icon. The objective is to locate all icon pairs. You can uncover two tiles at the same time. If they
aren't the same, the icons will be obscured again.
If you uncover two tiles with the same icon, then they remain visible - they're solved.

This is how the game looks like in action:

<video autoplay loop muted playsinline src="https://slint.dev/blog/memory-game-tutorial/memory_clip.mp4"
        class="img-fluid img-thumbnail rounded"></video>
