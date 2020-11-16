
Example based on the flutter slide_puzzle example:
https://flutter.github.io/samples/slide_puzzle

This will allow to compare SixtyFPS and Flutter.

Remaining feature to implement to have parity:
 * Images on the tiles in the "Berlin" theme.
 * Fonts.
 * "Spring" animation instead of a bezier curve.
 * Animation when clicking on a tile that cannot be moved.
 * Different visual when a piece is at the right location (bold in classic, white text on black in
   Seatle). Note that this feature is kind of broken in the flutter example as it is only applied
   when changing themes
 * Expanding cirle animation when pressing a tile.
 * Animation of the auto-play checkbox.
 * When the puzzle is finished, the last tile is added, and the tiles are growing in the Seatle theme,
   or a hand apears, and the puzzle cannot be moved.
 * The different styles are well separated in different files.
 * Shadow on the tiles

