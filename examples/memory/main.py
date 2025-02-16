# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT


from datetime import timedelta, datetime
import os
import random
import itertools
import copy
import slint
from slint import Color, ListModel, Timer, TimerMode


class MainWindow(slint.loader.memory.MainWindow):
    def __init__(self):
        super().__init__()
        initial_tiles = self.memory_tiles
        tiles = ListModel(
            itertools.chain(
                map(copy.copy, initial_tiles), map(copy.copy, initial_tiles)
            )
        )
        random.shuffle(tiles)
        self.memory_tiles = tiles

    @slint.callback
    def check_if_pair_solved(self):
        flipped_tiles = [
            (index, copy.copy(tile))
            for index, tile in enumerate(self.memory_tiles)
            if tile.image_visible and not tile.solved
        ]
        if len(flipped_tiles) == 2:
            tile1_index, tile1 = flipped_tiles[0]
            tile2_index, tile2 = flipped_tiles[1]
            is_pair_solved = tile1.image.path == tile2.image.path
            if is_pair_solved:
                tile1.solved = True
                self.memory_tiles[tile1_index] = tile1
                tile2.solved = True
                self.memory_tiles[tile2_index] = tile2
            else:
                self.disable_tiles = True

                def reenable_tiles():
                    self.disable_tiles = False
                    tile1.image_visible = False
                    self.memory_tiles[tile1_index] = tile1
                    tile2.image_visible = False
                    self.memory_tiles[tile2_index] = tile2

                Timer.single_shot(timedelta(seconds=1), reenable_tiles)


main_window = MainWindow()
main_window.run()
