# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import contextlib
import math
import re
import struct
import sys
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass
from types import TracebackType
from typing import Any, Self, TypeVar

import slint_testing as low
from slint_testing import keys

from . import native
from .control import active_time
from .diagnostics import StaleElement, WaitTimeout
from .reporting import step

T = TypeVar("T")
Name = str | re.Pattern[str]


class OperationTimeout(AssertionError):
    pass


class StrictMatchError(AssertionError):
    pass


class UnsupportedCapability(RuntimeError):
    pass


@dataclass(frozen=True)
class Point:
    x: float
    y: float


@dataclass(frozen=True)
class Bounds:
    x: float
    y: float
    width: float
    height: float

    @property
    def center(self) -> Point:
        return Point(self.x + self.width / 2, self.y + self.height / 2)


class Session:
    def __init__(
        self,
        *,
        timeout: float = 5000,
        cancel: threading.Event | None = None,
        process: Any = None,
    ):
        self.timeout = timeout
        self.cancel = cancel or threading.Event()
        self.process = process
        self.deadline: float | None = None
        self.cleaning = False

    @contextlib.contextmanager
    def operation(self, timeout: float | None = None):
        duration = self.timeout if timeout is None else timeout
        if not math.isfinite(duration) or duration < 0:
            raise ValueError("timeout must be finite nonnegative milliseconds")
        previous = self.deadline
        deadline = active_time() + duration / 1000
        self.deadline = min(previous, deadline) if previous is not None else deadline
        try:
            self.check()
            yield
        finally:
            self.deadline = previous

    @contextlib.contextmanager
    def cleanup(self):
        previous = self.cleaning
        self.cleaning = True
        try:
            yield
        finally:
            self.cleaning = previous

    def check(self) -> None:
        if self.cleaning:
            return
        if self.deadline is not None and active_time() >= self.deadline:
            raise OperationTimeout("Operation deadline exceeded")
        if self.cancel.is_set():
            raise InterruptedError("Test operation cancelled")
        if self.process is not None and self.process.poll() is not None:
            raise RuntimeError(
                f"Application exited with code {self.process.returncode}"
            )

    def wait(
        self,
        read: Callable[[], T],
        matches: Callable[[T], bool],
        *,
        timeout: float | None = None,
        description: str = "condition",
    ) -> T:
        duration = self.timeout if timeout is None else timeout
        if not math.isfinite(duration) or duration < 0:
            raise ValueError("timeout must be finite nonnegative milliseconds")
        deadline = active_time() + duration / 1000
        if self.deadline is not None:
            deadline = min(deadline, self.deadline)
        previous_deadline = self.deadline
        self.deadline = deadline
        try:
            actual: Any = "not observed"
            observed = False
            while True:
                self.check()
                try:
                    actual = read()
                    observed = True
                    if matches(actual):
                        return actual
                except StaleElement as error:
                    actual = str(error) or "element replaced or missing"
                    observed = False
                remaining = deadline - active_time()
                if remaining <= 0:
                    raise WaitTimeout(
                        f"{description}; timeout={duration:g}ms; last observed: {actual!r}",
                        actual=actual,
                        observed=observed,
                        timeout=duration,
                    )
                self.cancel.wait(min(0.02, remaining))
        except (OperationTimeout, TimeoutError) as error:
            raise WaitTimeout(
                f"{description}; timeout={duration:g}ms; last observed: {actual!r}; {error}",
                actual=actual,
                observed=observed,
                timeout=duration,
            ) from error
        finally:
            self.deadline = previous_deadline


def _role(name: str | low.AccessibleRole) -> low.AccessibleRole:
    if isinstance(name, low.AccessibleRole):
        return name
    normalized = name.replace("-", "").lower()
    for role in low.AccessibleRole:
        if role.name.lower() == normalized:
            return role
    raise ValueError(f"Unknown Slint accessibility role: {name}")


def _matches(actual: str, wanted: Name | None, exact: bool) -> bool:
    if wanted is None:
        return True
    if isinstance(wanted, re.Pattern):
        return wanted.search(actual) is not None
    return actual == wanted if exact else wanted.casefold() in actual.casefold()


class Scope:
    window: Window

    def _roots(self) -> list[low.Element]:
        raise NotImplementedError

    def get_by_role(
        self,
        role: str | low.AccessibleRole,
        *,
        name: Name | None = None,
        exact: bool = True,
    ) -> Locator:
        value = _role(role)
        return Locator(
            self.window,
            self,
            f"role={value.name}, name={name!r}",
            role=value,
            name=name,
            exact=exact,
        )

    def get_by_accessible_name(self, name: Name, *, exact: bool = True) -> Locator:
        return Locator(self.window, self, f"name={name!r}", name=name, exact=exact)

    def get_by_id(self, identifier: str) -> Locator:
        return Locator(self.window, self, f"id={identifier!r}", identifier=identifier)


class BoundApplication(low.Application):
    """Apply request deadlines without changing the installed transport globally."""

    def __init__(self, raw: low.Application, session: Session):
        self.raw = raw.raw if isinstance(raw, BoundApplication) else raw
        self.session = session
        self.process = raw.process

    def _send_request(self, request):
        self.session.check()
        connection = self.raw.aut_connection
        previous = connection.gettimeout()
        deadline = time.monotonic() + 5
        if self.session.deadline is not None and not self.session.cleaning:
            deadline = min(
                deadline, time.monotonic() + self.session.deadline - active_time()
            )
        buffer = bytearray()
        response_size = None
        expired = None
        try:
            connection.settimeout(max(0.001, deadline - time.monotonic()))
            payload = request.SerializeToString()
            connection.sendall(struct.pack(">I", len(payload)) + payload)
            while response_size is None or len(buffer) < response_size + 4:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    if expired is not None:
                        raise TimeoutError("Response cleanup deadline exceeded")
                    expired = OperationTimeout(
                        "Operation deadline exceeded during response"
                    )
                    deadline = time.monotonic() + 5
                    remaining = 5
                connection.settimeout(remaining)
                needed = 4 if response_size is None else response_size + 4
                try:
                    chunk = connection.recv(needed - len(buffer))
                except TimeoutError:
                    if expired is not None:
                        raise
                    expired = OperationTimeout(
                        "Operation deadline exceeded during response"
                    )
                    deadline = time.monotonic() + 5
                    continue
                if not chunk:
                    raise low.ApplicationConnectionError(
                        "Application disconnected during response"
                    )
                buffer.extend(chunk)
                if response_size is None and len(buffer) == 4:
                    response_size = struct.unpack(">I", buffer)[0]
            response = low.AUTResponse()
            response.ParseFromString(bytes(buffer[4:]))
        except BaseException:
            # A partial response cannot be mistaken for the next request's result.
            connection.close()
            raise
        finally:
            with contextlib.suppress(OSError):
                connection.settimeout(previous)
        if expired is not None:
            raise expired
        self.session.check()
        return response


class Window(Scope):
    @property
    def capabilities(self) -> dict[str, bool]:
        if self._capabilities is None:
            supported = native.supported(self.raw.app)
            self._capabilities = {
                "basic_readiness": True,
                "hit_testing": supported,
                "effective_clipping": supported,
                "scroll_into_view": supported,
            }
        return self._capabilities

    def __init__(self, raw: low.Window, *, session: Session | None = None):
        self.window = self
        self._capabilities: dict[str, bool] | None = None
        self.session = session or Session(process=getattr(raw.app, "process", None))
        self.raw = low.Window(BoundApplication(raw.app, self.session), raw.handle)
        self.keyboard = Keyboard(self)
        self.pointer = Pointer(self)
        _ = self.capabilities

    def _roots(self) -> list[low.Element]:
        return [self.raw.root_element]

    @property
    def root_element(self) -> low.Element:
        return self.raw.root_element

    @property
    def size(self) -> low.PhysicalSize:
        return self.raw.size

    @property
    def handle(self) -> low.Handle:
        return self.raw.handle

    def screenshot(self) -> bytes:
        with step("Screenshot", layer="generic"):
            self.session.check()
            return self.raw.grab_window_as_png()

    def drag_and_drop(
        self, start: Point | low.LogicalPosition, end: Point | low.LogicalPosition
    ) -> None:
        with step(
            "Drag and drop",
            layer="generic",
            start={"x": start.x, "y": start.y},
            end={"x": end.x, "y": end.y},
        ):
            self.raw.drag_and_drop(
                low.LogicalPosition(start.x, start.y),
                low.LogicalPosition(end.x, end.y),
            )

    def cleanup_input(self) -> None:
        with self.session.cleanup():
            try:
                self.pointer.release(cleanup=True)
            finally:
                self.keyboard.release_all()


class Locator(Scope):
    def __init__(
        self,
        window: Window,
        scope: Scope,
        description: str,
        *,
        role: low.AccessibleRole | None = None,
        name: Name | None = None,
        exact: bool = True,
        identifier: str | None = None,
        predicate: Callable[[low.Element], bool] | None = None,
        index: int | None = None,
    ):
        self.window, self.scope, self.description = window, scope, description
        self.role, self.name, self.exact, self.identifier = (
            role,
            name,
            exact,
            identifier,
        )
        self.predicate = predicate
        self.index = index
        self.source_context: Callable[[], dict[str, str]] | None = None

    def __repr__(self) -> str:
        prefix = f"{self.scope!r} >> " if isinstance(self.scope, Locator) else ""
        suffix = f".nth({self.index})" if self.index is not None else ""
        return prefix + self.description + suffix

    def _find(
        self,
        roots: list[low.Element] | None = None,
        *,
        include_clipped: bool = True,
    ) -> list[low.Element]:
        matches = []
        for root in self.scope._roots() if roots is None else roots:
            query = root.query_descendants()
            if self.role is not None:
                query = query.match_accessible_role(self.role)
            if self.identifier is not None:
                query = query.match_id(self.identifier)
            elements = (
                native.find_all(query)
                if include_clipped and self.window.capabilities["hit_testing"]
                else query.find_all()
            )
            for element in elements:
                if _matches(element.accessible_label, self.name, self.exact) and (
                    self.predicate is None or self.predicate(element)
                ):
                    if not element.is_valid:
                        continue
                    matches.append(element)
            if not root.is_valid:
                raise StaleElement
        if self.index is None:
            return matches
        return matches[self.index : self.index + 1]

    def _roots(self) -> list[low.Element]:
        return [self._unique()]

    def _unique(self, *, include_clipped: bool = True) -> low.Element:
        matches = self._find(include_clipped=include_clipped)
        if len(matches) > 1:
            candidates = [e.accessible_label for e in matches]
            raise StrictMatchError(
                f"{self!r} matched {len(matches)} elements: {candidates!r}"
            )
        if not matches:
            raise StaleElement
        return matches[0]

    def _action_candidate(self) -> low.Element:
        visible = self._find(include_clipped=False)
        if visible:
            if len(visible) > 1:
                candidates = [element.accessible_label for element in visible]
                raise StrictMatchError(
                    f"{self!r} matched {len(visible)} elements: {candidates!r}"
                )
            return visible[0]
        return self._unique(include_clipped=True)

    def filter(self, *, has: Locator) -> Locator:
        if has.window is not self.window or has.scope is not self.window:
            raise ValueError("has must be a locator rooted in the same window")
        previous = self.predicate
        return Locator(
            self.window,
            self.scope,
            f"{self.description}.filter(has={has!r})",
            role=self.role,
            name=self.name,
            exact=self.exact,
            identifier=self.identifier,
            predicate=lambda e: (
                (previous is None or previous(e)) and bool(has._find([e]))
            ),
            index=self.index,
        )

    def nth(self, index: int) -> Locator:
        if index < 0:
            raise ValueError("index must be nonnegative")
        return Locator(
            self.window,
            self.scope,
            self.description,
            role=self.role,
            name=self.name,
            exact=self.exact,
            identifier=self.identifier,
            predicate=self.predicate,
            index=index,
        )

    def read(self, getter: Callable[[low.Element], T]) -> T:
        element = self._unique(include_clipped=False)
        value = getter(element)
        if not element.is_valid:
            raise StaleElement
        return value

    def resolve(self, *, timeout: float | None = None) -> low.Element:
        return self.window.session.wait(
            lambda: self._unique(include_clipped=False),
            lambda _: True,
            timeout=timeout,
            description=f"Resolve {self!r}",
        )

    def count(self) -> int:
        return len(self._find(include_clipped=True))

    def visible_count(self) -> int:
        return len(self._find(include_clipped=False))

    def wait_for(self, *, state: str = "visible", timeout: float | None = None) -> None:
        if state not in ("visible", "hidden"):
            raise ValueError("state must be 'visible' or 'hidden'")
        with (
            self.window.session.operation(timeout),
            step(
                f"Wait for {state}",
                layer="generic",
                locator=repr(self),
            ),
        ):
            if state == "visible":
                self.window.session.wait(
                    lambda: self._unique(include_clipped=False),
                    lambda _: True,
                    timeout=timeout,
                    description=f"Wait for {self!r} to be visible",
                )
            else:
                self.window.session.wait(
                    self.visible_count,
                    lambda count: count == 0,
                    timeout=timeout,
                    description=f"Wait for {self!r} to be hidden",
                )

    def all(self) -> list[low.Element]:
        return self._find(include_clipped=False)

    def bounds(self) -> Bounds:
        def read(element: low.Element) -> Bounds:
            pos, size = element.absolute_position, element.size
            return Bounds(pos.x, pos.y, size.width, size.height)

        return self.read(read)

    def value(self) -> str:
        return self.read(lambda element: element.accessible_value)

    def accessible_description(self) -> str:
        return self.read(lambda element: element.accessible_description)

    def is_enabled(self) -> bool:
        return self.read(lambda element: element.accessible_enabled)

    def is_selected(self) -> bool:
        return self.read(lambda element: element.accessible_item_selected)

    def center(self, *, rotation_degrees: float = 0) -> Point:
        bounds = self.bounds()
        a = math.radians(rotation_degrees)
        return Point(
            bounds.x + bounds.width / 2 * math.cos(a) - bounds.height / 2 * math.sin(a),
            bounds.y + bounds.width / 2 * math.sin(a) + bounds.height / 2 * math.cos(a),
        )

    def _ready(
        self,
        *,
        timeout: float | None = None,
        pointer: bool = False,
        editable: bool = False,
        require_hit_target: bool = False,
        check_enabled: bool = True,
        verify_pointer: bool = False,
    ) -> low.Element:
        verified = (
            pointer
            and (verify_pointer or require_hit_target)
            and self.window.capabilities["hit_testing"]
        )
        if require_hit_target and not verified:
            raise UnsupportedCapability(
                "This transport cannot verify hit targets or effective clipping"
            )
        previous: Bounds | None = None
        last_target = {}

        def read() -> low.Element | None:
            nonlocal previous, last_target
            element = self._action_candidate()
            enabled = verified or not check_enabled or element.accessible_enabled
            if editable and (
                element.accessible_role != low.AccessibleRole.TextInput
                or element.accessible_read_only
            ):
                return None
            if verified:
                last_target = native.target(element)
                if last_target["status"] == "clipped" and last_target["scrollable"]:
                    with step("Scroll into view", layer="generic", locator=repr(self)):
                        last_target = native.target(element, scroll=True)
                    previous = None
                    return None
                if last_target["status"] == "unsupported":
                    raise UnsupportedCapability(last_target["detail"])
                if last_target["status"] != "ready":
                    previous = None
                    raise StaleElement(last_target["detail"])
            if pointer:
                position, size = element.absolute_position, element.size
                bounds = Bounds(position.x, position.y, size.width, size.height)
                stable = bounds == previous
                previous = bounds
                if (
                    not stable
                    or bounds.width <= 0
                    or bounds.height <= 0
                    or element.computed_opacity <= 0
                ):
                    return None
            if not element.is_valid:
                raise StaleElement
            return element if enabled else None

        with step(
            "Wait for control",
            layer="generic",
            locator=repr(self),
            pointer=pointer,
            editable=editable,
            check_enabled=check_enabled,
        ) as details:
            ready = self.window.session.wait(
                read,
                lambda value: value is not None,
                timeout=timeout,
                description=f"Readiness for {self!r}",
            )
            assert ready is not None
            if previous is not None:
                details["target_bounds"] = vars(previous)
            details["hit_target_verified"] = verified
            if last_target:
                details["pointer_target"] = last_target
            return ready

    def activate(self, *, timeout: float | None = None) -> None:
        with (
            self.window.session.operation(timeout),
            step(
                "Activate", layer="generic", locator=repr(self), input="accessibility"
            ),
        ):
            self._ready(
                timeout=timeout, check_enabled=False
            ).invoke_accessible_default_action()

    def set_accessible_value(self, value: str, *, timeout: float | None = None) -> None:
        with (
            self.window.session.operation(timeout),
            step(
                "Set accessible value", layer="generic", locator=repr(self), value=value
            ),
        ):
            self._ready(timeout=timeout, check_enabled=False).accessible_value = value

    def pointer_target(self) -> dict[str, Any]:
        if not self.window.capabilities["hit_testing"]:
            raise UnsupportedCapability("This transport cannot verify hit targets")
        return native.target(self._unique(include_clipped=True))

    def scroll_into_view(self, *, timeout: float | None = None) -> None:
        if not self.window.capabilities["scroll_into_view"]:
            raise UnsupportedCapability("This transport cannot scroll into view")
        with (
            self.window.session.operation(timeout),
            step("Scroll into view", layer="generic", locator=repr(self)),
        ):

            def reveal():
                result = native.target(self._unique(include_clipped=True), scroll=True)
                if result["status"] == "unsupported":
                    raise UnsupportedCapability(result["detail"])
                if result["status"] in ("clipped", "busy"):
                    raise StaleElement(result["detail"])
                return result

            self.window.session.wait(
                reveal,
                lambda result: True,
                timeout=timeout,
                description=f"Scroll {self!r}",
            )

    def _click(
        self,
        timeout=None,
        *,
        editable=False,
        force=False,
        require_hit_target=False,
    ):
        if force:
            self.resolve(timeout=timeout).single_click(low.PointerEventButton.Left)
            return
        while True:
            element = self._ready(
                timeout=timeout,
                pointer=True,
                editable=editable,
                require_hit_target=require_hit_target,
                verify_pointer=True,
            )
            if not self.window.capabilities["hit_testing"]:
                element.single_click(low.PointerEventButton.Left)
                return
            try:
                result = native.target(element, click=True)
            except StaleElement:
                continue
            if result["performed"]:
                return
            if result["status"] == "unsupported":
                raise UnsupportedCapability(result["detail"])
            self.window.session.check()
            self.window.session.cancel.wait(0.02)

    def click(
        self,
        *,
        timeout: float | None = None,
        force: bool = False,
        require_hit_target: bool = False,
    ) -> None:
        with (
            self.window.session.operation(timeout),
            step(
                "Click",
                layer="generic",
                locator=repr(self),
                readiness="native target"
                if self.window.capabilities["hit_testing"] and not force
                else "basic; hit target unverified",
                force=force,
            ),
        ):
            self._click(
                timeout,
                force=force,
                require_hit_target=require_hit_target,
            )

    def dblclick(self, *, timeout: float | None = None) -> None:
        with (
            self.window.session.operation(timeout),
            step("Double click", layer="generic", locator=repr(self)),
        ):
            if self.window.capabilities["hit_testing"]:
                self._click(timeout)
                self._click(timeout)
            else:
                self._ready(timeout=timeout, pointer=True).double_click(
                    low.PointerEventButton.Left
                )

    def hover(self, *, timeout: float | None = None) -> None:
        with (
            self.window.session.operation(timeout),
            step("Hover", layer="generic", locator=repr(self)),
        ):
            self._ready(timeout=timeout, pointer=True)
            self.window.pointer.move_to(self.center())

    def fill(self, text: str, *, timeout: float | None = None) -> None:
        with (
            self.window.session.operation(timeout),
            step("Fill", layer="generic", locator=repr(self), text=text),
        ):
            self._click(timeout, editable=True)
            self.window.keyboard.shortcut("Control", "a")
            self.window.keyboard.press("Backspace")
            self.window.keyboard.press_sequentially(text)

    def clear(self, *, timeout: float | None = None) -> None:
        with self.window.session.operation(timeout):
            self.fill("", timeout=timeout)

    def press(self, key: str, *, timeout: float | None = None) -> None:
        with (
            self.window.session.operation(timeout),
            step("Press on control", layer="generic", locator=repr(self), key=key),
        ):
            self._click(timeout)
            self.window.keyboard.press(key)

    def press_sequentially(self, text: str, *, timeout: float | None = None) -> None:
        with (
            self.window.session.operation(timeout),
            step("Type on control", layer="generic", locator=repr(self), text=text),
        ):
            self._click(timeout)
            self.window.keyboard.press_sequentially(text)

    def drag(
        self, *, rotation_degrees: float = 0, timeout: float | None = None
    ) -> Drag:
        with self.window.session.operation(timeout):
            self._ready(timeout=timeout, pointer=True)
            return self.window.pointer.drag_from(
                self.center(rotation_degrees=rotation_degrees)
            )

    def drag_to(self, target: Locator, *, timeout: float | None = None) -> None:
        with self.window.session.operation(timeout):
            target._ready(timeout=timeout, pointer=True)
            destination = target.center()
            with self.drag(timeout=timeout) as drag:
                for i in range(1, 11):
                    drag.move_to(
                        Point(
                            drag.start.x + (destination.x - drag.start.x) * i / 10,
                            drag.start.y + (destination.y - drag.start.y) * i / 10,
                        )
                    )
                drag.release()


class Keyboard:
    def __init__(self, window: Window):
        self.window = window
        self.held: list[str] = []

    @staticmethod
    def key(value: str) -> str:
        return getattr(keys, {"Enter": "Return"}.get(value, value), value)

    def down(self, key: str) -> None:
        self.window.session.check()
        with step("Key down", layer="generic", key=key):
            value = self.key(key)
            if value not in self.held:
                self.held.append(value)
            self.window.raw.dispatch_event(low.KeyPressedEvent(text=value))

    def up(self, key: str, *, cleanup: bool = False) -> None:
        if not cleanup:
            self.window.session.check()
        value = self.key(key)
        with step("Key up", layer="generic", key=key):
            self.window.raw.dispatch_event(low.KeyReleasedEvent(text=value))
            if value in self.held:
                self.held.remove(value)

    def press(self, key: str) -> None:
        try:
            self.down(key)
        finally:
            with self.window.session.cleanup():
                self.up(key, cleanup=True)

    def press_sequentially(self, text: str) -> None:
        for character in text:
            self.press(character)

    def shortcut(self, *keys_: str) -> None:
        before = list(self.held)
        try:
            for key in keys_:
                self.down(key)
        finally:
            for key in reversed(self.held.copy()):
                if key not in before:
                    with self.window.session.cleanup():
                        self.up(key, cleanup=True)

    def release_all(self) -> None:
        for key in reversed(self.held.copy()):
            self.up(key, cleanup=True)


class Pointer:
    def __init__(self, window: Window):
        self.window = window
        self.position = Point(0, 0)
        self.held = False

    def move_to(self, point: Point | low.LogicalPosition) -> None:
        self.window.session.check()
        with step("Pointer move", layer="generic", x=point.x, y=point.y):
            self.position = point
            self.window.raw.dispatch_event(
                low.PointerMoveEvent(low.LogicalPosition(point.x, point.y))
            )

    def press_at(self, point: Point | low.LogicalPosition) -> None:
        self.window.session.check()
        if self.held:
            raise RuntimeError("Pointer is already held")
        with step("Pointer down", layer="generic", x=point.x, y=point.y):
            self.position = Point(point.x, point.y)
            self.held = True
            self.window.raw.dispatch_event(
                low.PointerPressEvent(
                    low.LogicalPosition(point.x, point.y),
                    low.PointerEventButton.Left,
                )
            )

    def press(self) -> None:
        self.window.session.check()
        if self.held:
            raise RuntimeError("Pointer is already held")
        with step("Pointer down", layer="generic"):
            self.held = True
            self.window.raw.dispatch_event(
                low.PointerPressEvent(
                    low.LogicalPosition(self.position.x, self.position.y),
                    low.PointerEventButton.Left,
                )
            )

    def release(self, *, cleanup: bool = False) -> None:
        if not self.held:
            return
        if not cleanup:
            self.window.session.check()
        with step("Pointer up", layer="generic"):
            self.window.raw.dispatch_event(
                low.PointerReleaseEvent(
                    low.LogicalPosition(self.position.x, self.position.y),
                    low.PointerEventButton.Left,
                )
            )
            self.held = False

    def release_at(
        self, point: Point | low.LogicalPosition, *, cleanup: bool = False
    ) -> None:
        if not self.held:
            return
        if not cleanup:
            self.window.session.check()
        with step("Pointer up", layer="generic", x=point.x, y=point.y):
            self.position = Point(point.x, point.y)
            self.window.raw.dispatch_event(
                low.PointerReleaseEvent(
                    low.LogicalPosition(point.x, point.y),
                    low.PointerEventButton.Left,
                )
            )
            self.held = False

    def click_at(self, point: Point | low.LogicalPosition) -> None:
        with step("Pointer click", layer="generic", x=point.x, y=point.y):
            self.move_to(point)
            self.press_at(point)
            self.release_at(point)

    def scroll(
        self,
        delta_x: float,
        delta_y: float,
        *,
        at: Point | low.LogicalPosition | None = None,
    ) -> None:
        self.window.session.check()
        point = at or self.position
        with step(
            "Pointer scroll",
            layer="generic",
            x=point.x,
            y=point.y,
            delta_x=delta_x,
            delta_y=delta_y,
        ):
            self.position = Point(point.x, point.y)
            self.window.raw.dispatch_event(
                low.PointerScrolledEvent(
                    low.LogicalPosition(point.x, point.y),
                    delta_x=delta_x,
                    delta_y=delta_y,
                )
            )

    def exit(self) -> None:
        self.window.session.check()
        with step("Pointer exit", layer="generic"):
            self.window.raw.dispatch_event(low.PointerExitedEvent())

    def drag_from(self, start: Point) -> Drag:
        return Drag(self, start)


class Drag:
    def __init__(self, pointer: Pointer, start: Point):
        self.pointer, self.start = pointer, start
        self.context: Any = None
        self.keys_before: list[str] = []

    def __enter__(self) -> Self:
        if self.pointer.held:
            raise RuntimeError("Pointer is already held")
        self.context = step("Drag", layer="generic", start=vars(self.start))
        self.context.__enter__()
        self.keys_before = self.pointer.window.keyboard.held.copy()
        try:
            self.pointer.move_to(self.start)
            self.pointer.press()
            return self
        except BaseException:
            self.__exit__(*sys.exc_info())
            raise

    def move_to(self, point: Point) -> None:
        self.pointer.move_to(point)

    def move_by(
        self, x: float, y: float, *, space: str = "window", origin: str = "current"
    ) -> None:
        if space != "window" or origin not in ("current", "start"):
            raise ValueError("Use window coordinates with current or start origin")
        base = self.start if origin == "start" else self.pointer.position
        self.move_to(Point(base.x + x, base.y + y))

    def release(self) -> None:
        self.pointer.release()

    def __exit__(
        self,
        kind: type[BaseException] | None,
        error: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        cleanup_error = None
        with self.pointer.window.session.cleanup():
            if self.pointer.held:
                with contextlib.suppress(Exception):
                    self.pointer.window.raw.dispatch_event(
                        low.KeyPressedEvent(text=keys.Escape)
                    )
                    self.pointer.window.raw.dispatch_event(
                        low.KeyReleasedEvent(text=keys.Escape)
                    )
                try:
                    self.pointer.release(cleanup=True)
                except Exception as failure:  # noqa: BLE001
                    cleanup_error = failure
            keyboard = self.pointer.window.keyboard
            for key in reversed(keyboard.held.copy()):
                if key not in self.keys_before:
                    try:
                        keyboard.up(key, cleanup=True)
                    except Exception as failure:  # noqa: BLE001
                        cleanup_error = cleanup_error or failure
        if error is None and cleanup_error is not None:
            self.context.__exit__(
                type(cleanup_error), cleanup_error, cleanup_error.__traceback__
            )
            raise cleanup_error
        self.context.__exit__(kind, error, tb)
