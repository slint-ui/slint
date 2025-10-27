from __future__ import annotations

import enum

from typing import Any, Callable

import slint

__all__ = ['Diag', 'App', 'Secret_Struct', 'MyData', 'ImageFit', 'TextHorizontalAlignment', 'SortOrder', 'PointerEventKind', 'ImageRendering', 'ImageTiling', 'OperatingSystemType', 'InputType', 'Orientation', 'TextWrap', 'ScrollBarPolicy', 'ImageVerticalAlignment', 'PointerEventButton', 'TextOverflow', 'LayoutAlignment', 'StandardButtonKind', 'AccessibleRole', 'EventResult', 'MouseCursor', 'AnimationDirection', 'TextVerticalAlignment', 'TextStrokeStyle', 'LineCap', 'ImageHorizontalAlignment', 'FocusReason', 'FillRule', 'ColorScheme', 'PathEvent', 'TestEnum', 'DialogButtonRole', 'PopupClosePolicy', 'MyDiag', 'Public_Struct']

class Secret_Struct:
    def __init__(self, **kwargs: Any) -> None:
        ...
    balance: float

class MyData:
    def __init__(self, **kwargs: Any) -> None:
        ...
    age: float
    name: str

class ImageFit(enum.Enum):
    fill = 'fill'
    contain = 'contain'
    cover = 'cover'
    preserve = 'preserve'

class TextHorizontalAlignment(enum.Enum):
    left = 'left'
    center = 'center'
    right = 'right'

class SortOrder(enum.Enum):
    unsorted = 'unsorted'
    ascending = 'ascending'
    descending = 'descending'

class PointerEventKind(enum.Enum):
    cancel = 'cancel'
    down = 'down'
    up = 'up'
    move = 'move'

class ImageRendering(enum.Enum):
    smooth = 'smooth'
    pixelated = 'pixelated'

class ImageTiling(enum.Enum):
    none = 'none'
    repeat = 'repeat'
    round = 'round'

class OperatingSystemType(enum.Enum):
    android = 'android'
    ios = 'ios'
    macos = 'macos'
    linux = 'linux'
    windows = 'windows'
    other = 'other'

class InputType(enum.Enum):
    text = 'text'
    password = 'password'
    number = 'number'
    decimal = 'decimal'

class Orientation(enum.Enum):
    horizontal = 'horizontal'
    vertical = 'vertical'

class TextWrap(enum.Enum):
    nowrap = 'nowrap'
    wordwrap = 'wordwrap'
    charwrap = 'charwrap'

class ScrollBarPolicy(enum.Enum):
    asneeded = 'asneeded'
    alwaysoff = 'alwaysoff'
    alwayson = 'alwayson'

class ImageVerticalAlignment(enum.Enum):
    center = 'center'
    top = 'top'
    bottom = 'bottom'

class PointerEventButton(enum.Enum):
    other = 'other'
    left = 'left'
    right = 'right'
    middle = 'middle'
    back = 'back'
    forward = 'forward'

class TextOverflow(enum.Enum):
    clip = 'clip'
    elide = 'elide'

class LayoutAlignment(enum.Enum):
    stretch = 'stretch'
    center = 'center'
    start = 'start'
    end = 'end'
    spacebetween = 'spacebetween'
    spacearound = 'spacearound'
    spaceevenly = 'spaceevenly'

class StandardButtonKind(enum.Enum):
    ok = 'ok'
    cancel = 'cancel'
    apply = 'apply'
    close = 'close'
    reset = 'reset'
    help = 'help'
    yes = 'yes'
    no = 'no'
    abort = 'abort'
    retry = 'retry'
    ignore = 'ignore'

class AccessibleRole(enum.Enum):
    none = 'none'
    button = 'button'
    checkbox = 'checkbox'
    combobox = 'combobox'
    groupbox = 'groupbox'
    image = 'image'
    list = 'list'
    slider = 'slider'
    spinbox = 'spinbox'
    tab = 'tab'
    tablist = 'tablist'
    tabpanel = 'tabpanel'
    text = 'text'
    table = 'table'
    tree = 'tree'
    progressindicator = 'progressindicator'
    textinput = 'textinput'
    switch = 'switch'
    listitem = 'listitem'

class EventResult(enum.Enum):
    reject = 'reject'
    accept = 'accept'

class MouseCursor(enum.Enum):
    default = 'default'
    none = 'none'
    help = 'help'
    pointer = 'pointer'
    progress = 'progress'
    wait = 'wait'
    crosshair = 'crosshair'
    text = 'text'
    alias = 'alias'
    copy = 'copy'
    move = 'move'
    nodrop = 'nodrop'
    notallowed = 'notallowed'
    grab = 'grab'
    grabbing = 'grabbing'
    colresize = 'colresize'
    rowresize = 'rowresize'
    nresize = 'nresize'
    eresize = 'eresize'
    sresize = 'sresize'
    wresize = 'wresize'
    neresize = 'neresize'
    nwresize = 'nwresize'
    seresize = 'seresize'
    swresize = 'swresize'
    ewresize = 'ewresize'
    nsresize = 'nsresize'
    neswresize = 'neswresize'
    nwseresize = 'nwseresize'

class AnimationDirection(enum.Enum):
    normal = 'normal'
    reverse = 'reverse'
    alternate = 'alternate'
    alternatereverse = 'alternatereverse'

class TextVerticalAlignment(enum.Enum):
    top = 'top'
    center = 'center'
    bottom = 'bottom'

class TextStrokeStyle(enum.Enum):
    outside = 'outside'
    center = 'center'

class LineCap(enum.Enum):
    butt = 'butt'
    round = 'round'
    square = 'square'

class ImageHorizontalAlignment(enum.Enum):
    center = 'center'
    left = 'left'
    right = 'right'

class FocusReason(enum.Enum):
    programmatic = 'programmatic'
    tabnavigation = 'tabnavigation'
    pointerclick = 'pointerclick'
    popupactivation = 'popupactivation'
    windowactivation = 'windowactivation'

class FillRule(enum.Enum):
    nonzero = 'nonzero'
    evenodd = 'evenodd'

class ColorScheme(enum.Enum):
    unknown = 'unknown'
    dark = 'dark'
    light = 'light'

class PathEvent(enum.Enum):
    begin = 'begin'
    line = 'line'
    quadratic = 'quadratic'
    cubic = 'cubic'
    endopen = 'endopen'
    endclosed = 'endclosed'

class TestEnum(enum.Enum):
    Variant1 = 'Variant1'
    Variant2 = 'Variant2'

class DialogButtonRole(enum.Enum):
    none = 'none'
    accept = 'accept'
    reject = 'reject'
    apply = 'apply'
    reset = 'reset'
    help = 'help'
    action = 'action'

class PopupClosePolicy(enum.Enum):
    closeonclick = 'closeonclick'
    closeonclickoutside = 'closeonclickoutside'
    noautoclose = 'noautoclose'

class Diag(slint.Component):
    def __init__(self, **kwargs: Any) -> None:
        ...
    class MyGlobal:
        global_prop: str
        global_callback: Callable[[str], str]
        minus_one: Callable[[int], None]
    class SecondGlobal:
        second: str

class App(slint.Component):
    def __init__(self, **kwargs: Any) -> None:
        ...
    builtin_enum: Any
    enum_property: Any
    hello: str
    model_with_enums: slint.ListModel[Any]
    translated: str
    call_void: Callable[[], None]
    invoke_call_void: Callable[[], None]
    invoke_global_callback: Callable[[str], str]
    invoke_say_hello: Callable[[str], str]
    invoke_say_hello_again: Callable[[str], str]
    say_hello: Callable[[str], str]
    say_hello_again: Callable[[str], str]
    plus_one: Callable[[int], None]
    class MyGlobal:
        global_prop: str
        global_callback: Callable[[str], str]
        minus_one: Callable[[int], None]
    class SecondGlobal:
        second: str

MyDiag = Diag

Public_Struct = Secret_Struct

