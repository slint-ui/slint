# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

from slint import Color, ListModel, Timer, TimerMode
import slint
from datetime import timedelta, datetime
import os
import copy
import sys

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

PrinterQueueItem = slint.loader.ui.printerdemo.PrinterQueueItem


class MainWindow(slint.loader.ui.printerdemo.MainWindow):
    def __init__(self):
        super().__init__()
        self.ink_levels = ListModel(
            [
                {"color": Color("#0ff"), "level": 0.4},
                {"color": Color("#ff0"), "level": 0.2},
                {"color": Color("#f0f"), "level": 0.5},
                {"color": Color("#000"), "level": 0.8},
            ]
        )
        # Copy the read-only mock data from the UI into a mutable ListModel
        self.printer_queue = ListModel(self.PrinterQueue.printer_queue)
        self.PrinterQueue.printer_queue = self.printer_queue
        self.print_progress_timer = Timer()
        self.print_progress_timer.start(
            TimerMode.Repeated, timedelta(seconds=1), self.update_jobs
        )

    @slint.callback
    def quit(self):
        self.hide()

    @slint.callback(global_name="PrinterQueue", name="start_job")
    def push_job(self, title):
        self.printer_queue.append(
            PrinterQueueItem(
                status="waiting",
                progress=0,
                title=title,
                owner="Me",
                pages=1,
                size="100kB",
                submission_date=str(datetime.now()),
            )
        )

    @slint.callback(global_name="PrinterQueue")
    def cancel_job(self, index):
        del self.printer_queue[index]

    def update_jobs(self):
        if len(self.printer_queue) <= 0:
            return
        top_item = copy.copy(self.printer_queue[0])
        top_item.progress += 1
        if top_item.progress >= 100:
            del self.printer_queue[0]
            if len(self.printer_queue) == 0:
                return
            top_item = copy.copy(self.printer_queue[0])
        self.printer_queue[0] = top_item


main_window = MainWindow()
main_window.run()
