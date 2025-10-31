# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import asyncio
import asyncio.events
import asyncio.selector_events
import datetime
import selectors
import socket
import typing
from collections.abc import Mapping

from . import core


class _HasFileno(typing.Protocol):
    def fileno(self) -> int: ...


def _fd_for_fileobj(fileobj: int | _HasFileno) -> int:
    if isinstance(fileobj, int):
        return fileobj
    return int(fileobj.fileno())


class _SlintSelectorMapping(Mapping[typing.Any, selectors.SelectorKey]):
    def __init__(self, slint_selector: "_SlintSelector") -> None:
        self.slint_selector = slint_selector

    def __len__(self) -> int:
        return len(self.slint_selector.fd_to_selector_key)

    def get(self, fileobj, default=None):  # type: ignore
        fd = _fd_for_fileobj(fileobj)
        return self.slint_selector.fd_to_selector_key.get(fd, default)

    def __getitem__(self, fileobj: typing.Any) -> selectors.SelectorKey:
        fd = _fd_for_fileobj(fileobj)
        return self.slint_selector.fd_to_selector_key[fd]

    def __iter__(self):  # type: ignore
        return iter(self.slint_selector.fd_to_selector_key)


class _SlintSelector(selectors.BaseSelector):
    def __init__(self) -> None:
        self.fd_to_selector_key: typing.Dict[typing.Any, selectors.SelectorKey] = {}
        self.mapping = _SlintSelectorMapping(self)
        self.adapters: typing.Dict[int, core.AsyncAdapter] = {}
        self._base_selector = selectors.DefaultSelector()
        self._wakeup_reader, self._wakeup_writer = socket.socketpair()
        self._wakeup_reader.setblocking(False)
        self._wakeup_writer.setblocking(False)
        self._base_selector.register(self._wakeup_reader, selectors.EVENT_READ)

    def register(
        self, fileobj: typing.Any, events: typing.Any, data: typing.Any = None
    ) -> selectors.SelectorKey:
        fd = _fd_for_fileobj(fileobj)
        key = selectors.SelectorKey(fileobj, fd, events, data)
        self.fd_to_selector_key[fd] = key

        adapter = core.AsyncAdapter(fd)
        self.adapters[fd] = adapter

        if events & selectors.EVENT_READ:
            adapter.wait_for_readable(self.read_notify)
        if events & selectors.EVENT_WRITE:
            adapter.wait_for_writable(self.write_notify)

        try:
            self._base_selector.register(fileobj, events, data)
        except KeyError:
            self._base_selector.modify(fileobj, events, data)

        return key

    def unregister(self, fileobj: typing.Any) -> selectors.SelectorKey:
        fd = _fd_for_fileobj(fileobj)
        key = self.fd_to_selector_key.pop(fd)

        try:
            del self.adapters[fd]
        except KeyError:
            pass

        try:
            self._base_selector.unregister(fileobj)
        except KeyError:
            pass

        return key

    def modify(
        self, fileobj: typing.Any, events: int, data: typing.Any = None
    ) -> selectors.SelectorKey:
        fd = _fd_for_fileobj(fileobj)
        key = self.fd_to_selector_key[fd]

        if key.events != events:
            self.unregister(fileobj)
            key = self.register(fileobj, events, data)
        elif key.data != data:
            key._replace(data=data)
            self.fd_to_selector_key[fd] = key

            try:
                self._base_selector.modify(fileobj, events, data)
            except KeyError:
                self._base_selector.register(fileobj, events, data)

        return key

    def select(
        self, timeout: float | None = None
    ) -> typing.List[typing.Tuple[selectors.SelectorKey, int]]:
        events = self._base_selector.select(timeout)
        ready: typing.List[typing.Tuple[selectors.SelectorKey, int]] = []
        for key, mask in events:
            if key.fileobj is self._wakeup_reader:
                self._drain_wakeup()
                continue

            fd = _fd_for_fileobj(key.fileobj)
            slint_key = self.fd_to_selector_key.get(fd)
            if slint_key is not None:
                ready.append((slint_key, mask))

        return ready

    def close(self) -> None:
        try:
            self._base_selector.unregister(self._wakeup_reader)
        except Exception:
            pass
        self._base_selector.close()
        self._wakeup_reader.close()
        self._wakeup_writer.close()

    def get_map(self) -> Mapping[int | _HasFileno, selectors.SelectorKey]:
        return self.mapping

    def read_notify(self, fd: int) -> None:
        key = self.fd_to_selector_key[fd]
        (reader, writer) = key.data
        reader._run()
        self._wakeup()

    def write_notify(self, fd: int) -> None:
        key = self.fd_to_selector_key[fd]
        (reader, writer) = key.data
        writer._run()
        self._wakeup()

    def _wakeup(self) -> None:
        try:
            self._wakeup_writer.send(b"\0")
        except BlockingIOError:
            pass

    def _drain_wakeup(self) -> None:
        try:
            while self._wakeup_reader.recv(1024):
                pass
        except BlockingIOError:
            pass


class SlintEventLoop(asyncio.SelectorEventLoop):
    def __init__(self) -> None:
        self._is_running = False
        self._timers: typing.Set[core.Timer] = set()
        self.stop_run_forever_event = asyncio.Event()
        self._soon_tasks: typing.List[asyncio.TimerHandle] = []
        self._core_loop_started = False
        self._core_loop_running = False
        super().__init__(_SlintSelector())

    def run_forever(self) -> None:
        if self._core_loop_started:
            return asyncio.selector_events.BaseSelectorEventLoop.run_forever(self)

        async def loop_stopper(event: asyncio.Event) -> None:
            await event.wait()
            core.quit_event_loop()

        asyncio.events._set_running_loop(self)
        self._is_running = True
        self._core_loop_started = True
        self._core_loop_running = True
        try:
            self.stop_run_forever_event = asyncio.Event()
            self.create_task(loop_stopper(self.stop_run_forever_event))
            core.run_event_loop()
        finally:
            self._core_loop_running = False
            self._is_running = False
            self._stopping = False
            asyncio.events._set_running_loop(None)

    def run_until_complete[T](self, future: typing.Awaitable[T]) -> T | None:  # type: ignore[override]
        if self._core_loop_started and not self._core_loop_running:
            return asyncio.selector_events.BaseSelectorEventLoop.run_until_complete(
                self, future
            )

        def stop_loop(future: typing.Any) -> None:
            self.stop()

        future = asyncio.ensure_future(future, loop=self)
        future.add_done_callback(stop_loop)

        try:
            self.run_forever()
        finally:
            future.remove_done_callback(stop_loop)

        if future.done():
            return future.result()
        if self.stop_run_forever_event.is_set():
            raise RuntimeError("run_until_complete's future isn't done", future)

        # The Slint core event loop can terminate even though the awaiting coroutine
        # is still running (for example when the user closes the last window). Python's
        # BaseEventLoop would raise a RuntimeError in that case, but Slint's API expects
        # a graceful shutdown without surfacing an exception to the caller. Returning
        # None here mirrors the historical behaviour and avoids breaking applications.
        # Attempts at cancelling the Task at this point still leave it pending because
        # the underlying loop has already stopped, so we cannot currently satisfy the
        # TODO of propagating a proper CancelledError.
        return None

    def _run_forever_setup(self) -> None:
        pass

    def _run_forever_cleanup(self) -> None:
        pass

    def stop(self) -> None:
        if (
            self._core_loop_started
            and self._core_loop_running
            and self.stop_run_forever_event is not None
        ):
            self.stop_run_forever_event.set()

        super().stop()
        selector = self._selector  # type: ignore[attr-defined]
        if isinstance(selector, _SlintSelector):
            selector._wakeup()

    def is_running(self) -> bool:
        return self._is_running

    def close(self) -> None:
        super().close()

    def is_closed(self) -> bool:
        return False

    def call_later(self, delay, callback, *args, context=None) -> asyncio.TimerHandle:  # type: ignore
        if self._core_loop_started and not self._core_loop_running:
            return super().call_later(delay, callback, *args, context=context)

        timer = core.Timer()

        handle = asyncio.TimerHandle(
            when=self.time() + delay,
            callback=callback,
            args=args,
            loop=self,
            context=context,
        )

        timers = self._timers

        def timer_done_cb() -> None:
            timers.remove(timer)
            if not handle._cancelled:
                handle._run()

        timer.start(
            core.TimerMode.SingleShot,
            interval=datetime.timedelta(seconds=delay),
            callback=timer_done_cb,
        )

        timers.add(timer)

        return handle

    def call_at(self, when, callback, *args, context=None) -> asyncio.TimerHandle:  # type: ignore
        return self.call_later(when - self.time(), callback, *args, context=context)

    def call_soon(self, callback, *args, context=None) -> asyncio.TimerHandle:  # type: ignore
        if self._core_loop_started and not self._core_loop_running:
            return super().call_soon(callback, *args, context=context)  # type: ignore

        # Collect call-soon tasks in a separate list to ensure FIFO order, as there's no guarantee
        # that multiple single-shot timers in Slint are run in order.
        handle = asyncio.TimerHandle(
            when=self.time(), callback=callback, args=args, loop=self, context=context
        )
        self._soon_tasks.append(handle)
        self.call_later(0, self._flush_soon_tasks)
        return handle

    def _flush_soon_tasks(self) -> None:
        tasks_now = self._soon_tasks
        self._soon_tasks = []
        for handle in tasks_now:
            if not handle._cancelled:
                handle._run()

    def call_soon_threadsafe(self, callback, *args, context=None) -> asyncio.Handle:  # type: ignore
        if self._core_loop_started and not self._core_loop_running:
            return super().call_soon_threadsafe(callback, *args, context=context)

        handle = asyncio.Handle(
            callback=callback,
            args=args,
            loop=self,
            context=context,
        )

        def run_handle_cb() -> None:
            if not handle._cancelled:
                handle._run()

        core.invoke_from_event_loop(run_handle_cb)
        return handle

    def _write_to_self(self) -> None:
        selector = self._selector  # type: ignore[attr-defined]
        if isinstance(selector, _SlintSelector):
            selector._wakeup()
        else:
            asyncio.SelectorEventLoop._write_to_self(self)  # type: ignore[attr-defined]
