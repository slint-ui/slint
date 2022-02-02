# Introduction

This tutorial will introduce you to the SixtyFPS UI framework in a playful way by implementing a little memory game. We are going to combine the `.slint` language for the graphics with the game rules implemented in C++.

The game consists of a grid of 16 rectangular tiles. When clicking on a tile, an icon underneath is uncovered.
We know that there are 8 different icons in total, so each tile has a sibling somewhere in the grid with the
same icon. The objective is to locate all icon pairs. Only two tiles can be uncovered at the same time. If they
are not the same, then the icons will be obscured again. We need to remember under which tiles the matching
graphics are hiding. If two tiles with the same icon are uncovered, then they remain visible - they are solved.

This is how the game looks like in action:

<video autoplay loop muted playsinline src="https://sixtyfps.io/blog/memory-game-tutorial/memory_clip.mp4"
        class="img-fluid img-thumbnail rounded"></video>

A video-recording of this tutorial is also available on YouTube. After introducing the `.slint` language the video
continues with a Rust implementation, but around minute 42 the C++ begins:

<iframe width="560" height="315" src="https://www.youtube-nocookie.com/embed/_-Hxr6ZrHyo" title="YouTube video player" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" allowfullscreen></iframe>
