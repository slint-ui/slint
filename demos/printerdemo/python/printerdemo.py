# This file is auto-generated

import slint
import typing
import enum
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


class SubPage(enum.StrEnum):
    None_ = "None"
    Print = "Print"
    Scan = "Scan"
    Copy = "Copy"
    Usb = "Usb"


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


globals().update(vars(slint._load_file_checked(path=os.path.join(os.path.dirname(__file__), '../ui/printerdemo.slint'), expected_api=r'{"version":"1.0","globals":[{"name":"PrinterQueue","properties":[{"name":"cancel_job","ty":"typing.Callable[[float], None]"},{"name":"pause_job","ty":"typing.Callable[[float], None]"},{"name":"printer_queue","ty":"slint.Model[PrinterQueueItem]"},{"name":"start_job","ty":"typing.Callable[[str], None]"},{"name":"statusString","ty":"typing.Callable[[str], str]"}],"aliases":[]},{"name":"PrinterSettings","properties":[{"name":"change_language","ty":"typing.Callable[[float], None]"}],"aliases":[]}],"components":[{"name":"MainWindow","properties":[{"name":"active_page","ty":"float"},{"name":"dark_mode","ty":"bool"},{"name":"ink_levels","ty":"slint.Model[InkLevel]"},{"name":"quit","ty":"typing.Callable[[], None]"}],"aliases":[]}],"structs_and_enums":[{"Struct":{"name":"ButtonColors","fields":[{"name":"base","ty":"slint.Color"},{"name":"hovered","ty":"slint.Color"},{"name":"pressed","ty":"slint.Color"}],"aliases":[]}},{"Struct":{"name":"File","fields":[{"name":"name","ty":"str"},{"name":"preview","ty":"slint.Image"}],"aliases":[]}},{"Struct":{"name":"InkLevel","fields":[{"name":"color","ty":"slint.Color"},{"name":"level","ty":"float"}],"aliases":[]}},{"Struct":{"name":"ModeColors","fields":[{"name":"background","ty":"slint.Color"},{"name":"destructive","ty":"slint.Color"},{"name":"primary","ty":"slint.Color"},{"name":"secondary","ty":"slint.Color"},{"name":"text_primary","ty":"slint.Color"},{"name":"text_secondary","ty":"slint.Color"}],"aliases":[]}},{"Struct":{"name":"PrinterQueueItem","fields":[{"name":"owner","ty":"str"},{"name":"pages","ty":"float"},{"name":"progress","ty":"float"},{"name":"size","ty":"str"},{"name":"status","ty":"str"},{"name":"submission_date","ty":"str"},{"name":"title","ty":"str"}],"aliases":[]}},{"Enum":{"name":"SubPage","variants":[{"name":"None_","strvalue":"None"},{"name":"Print","strvalue":"Print"},{"name":"Scan","strvalue":"Scan"},{"name":"Copy","strvalue":"Copy"},{"name":"Usb","strvalue":"Usb"}],"aliases":[]}}]}')))
