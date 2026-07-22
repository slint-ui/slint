# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import slint


class MainWindow(slint.loader.ui.app_window.MainWindow):
    pass


main_window = MainWindow()
main_window.show()
main_window.run()
