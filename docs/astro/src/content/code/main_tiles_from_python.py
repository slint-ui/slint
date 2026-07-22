# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import slint
import sys
import os
import random
import itertools
import copy
import datetime


class MainWindow(slint.loader.ui.app_window.MainWindow):
    def __init__(self):
        super().__init__()
        initial_tiles = self.memory_tiles
        tiles = slint.ListModel(
            itertools.chain(
                map(copy.copy, initial_tiles), map(copy.copy, initial_tiles)
            )
        )
        random.shuffle(tiles)
        self.memory_tiles = tiles


main_window = MainWindow()
main_window.show()
main_window.run()
