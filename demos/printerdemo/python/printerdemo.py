# This file is auto-generated

import slint
import typing
import os

class ButtonColors:
    base: slint.Color
    hovered: slint.Color
    pressed: slint.Color

    def __init__(self, *, base: typing.Optional[slint.Color] = None, hovered: typing.Optional[slint.Color] = None, pressed: typing.Optional[slint.Color] = None) -> None: ...


class File:
    name: str
    preview: slint.Image

    def __init__(self, *, name: typing.Optional[str] = None, preview: typing.Optional[slint.Image] = None) -> None: ...


class InkLevel:
    color: slint.Color
    level: float

    def __init__(self, *, color: typing.Optional[slint.Color] = None, level: typing.Optional[float] = None) -> None: ...


class ModeColors:
    background: slint.Color
    destructive: slint.Color
    primary: slint.Color
    secondary: slint.Color
    text_primary: slint.Color
    text_secondary: slint.Color

    def __init__(self, *, background: typing.Optional[slint.Color] = None, destructive: typing.Optional[slint.Color] = None, primary: typing.Optional[slint.Color] = None, secondary: typing.Optional[slint.Color] = None, text_primary: typing.Optional[slint.Color] = None, text_secondary: typing.Optional[slint.Color] = None) -> None: ...


class PrinterQueueItem:
    owner: str
    pages: float
    progress: float
    size: str
    status: str
    submission_date: str
    title: str

    def __init__(self, *, owner: typing.Optional[str] = None, pages: typing.Optional[float] = None, progress: typing.Optional[float] = None, size: typing.Optional[str] = None, status: typing.Optional[str] = None, submission_date: typing.Optional[str] = None, title: typing.Optional[str] = None) -> None: ...


class PrinterQueue:
    cancel_job: typing.Callable[[float], None]
    pause_job: typing.Callable[[float], None]
    printer_queue: slint.Model[PrinterQueueItem]
    start_job: typing.Callable[[str], None]
    statusString: typing.Callable[[str], str]


class PrinterSettings:
    change_language: typing.Callable[[float], None]


class MainWindow(slint.Component):
    active_page: float
    dark_mode: bool
    ink_levels: slint.Model[InkLevel]
    quit: typing.Callable[[], None]
    PrinterQueue: PrinterQueue
    PrinterSettings: PrinterSettings


globals().update(vars(slint.load_file(os.path.join(os.path.dirname(__file__), '../ui/printerdemo.slint'))))
