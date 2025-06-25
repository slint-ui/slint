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


globals().update(vars(slint._load_file_checked(path=os.path.join(os.path.dirname(__file__), '../ui/printerdemo.slint'), expected_api_base64_compressed=r'H4sIAAAAAAAA/51VTW/bMAz9K4POQbBdc1ywAQHWoUMw7BAEAm2zrhZZciXKWVbkv0+Wk0Jy/YVdbJh6JB8fKfqVNWis0Ipt2Kf1R7ZipdQZSMs2h1emoEJ/8GiEIjQ/HDr0iNroGg0JTEA5qBwl/60zD6GLt9ClFqpcb0FKyCQeDk9SAx1XH75rhUd2Xb351uAs/qdrx42/3MgFdyu9cf2gC5SHmPyOsEq8LYGhycSWzFBa70jO7skHL2d82ye7HlcMpAAbRDtGkW789kjkve24vs+gSuTSPx2UuFSpXl7/meuq9meKkvAPINQvoQp9HmUAOYkGeR1lD8liXQowJ1554e+ITGsZA4Q6cYkNSjvUrJ06fWsPE61fnKDRcqcq9cq7nCwHVXBUruqq2Qcr27zF/+yItNpqqU1L6kmgLJLCMx80ZRvAMcdn7e8RFjOo2qC1I6ge/dbrHdOvQuIgw/C6R6V+zkbgOc25q9omLsp5b8lg3jxwny5a3rzjgVmSuJ2Iyabkp9Jop+ZEL7CbAz+7s+0RFZjLDMpirlUxjyP8Q3xZyACdjrtEsf6qG9RNnxWasVnxU2FH77bfCmU7wKMAK/6OTWG3L8cOXVYJ2/6FeAE0FoIEyfRsQJMv/ppHiuxd9titqwaMgN7OaxcHZ2FPNCDd3cT62zmFdKYIs/e/vhQSLBFiq+tLigiWCPHTZimgNbwr8Hj9ByzRI2WwBwAA', generated_file=__file__)))
