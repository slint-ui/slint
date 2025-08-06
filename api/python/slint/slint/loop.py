# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from . import slint as native
import asyncio.selector_events
import asyncio
import asyncio.events
import selectors
import typing
from collections.abc import Mapping
import datetime


class HasFileno(typing.Protocol):
    def fileno(self) -> int: ...


def fd_for_fileobj(fileobj: int | HasFileno) -> int:
    if isinstance(fileobj, int):
        return fileobj
    return int(fileobj.fileno())


class _SlintSelectorMapping(Mapping[typing.Any, selectors.SelectorKey]):
    def __init__(self, slint_selector: "_SlintSelector") -> None:
        self.slint_selector = slint_selector

    def __len__(self) -> int:
        return len(self.slint_selector.fd_to_selector_key)

    def get(self, fileobj, default=None):  # type: ignore
        fd = fd_for_fileobj(fileobj)
        return self.slint_selector.fd_to_selector_key.get(fd, default)

    def __getitem__(self, fileobj: typing.Any) -> selectors.SelectorKey:
        fd = fd_for_fileobj(fileobj)
        return self.slint_selector.fd_to_selector_key[fd]

    def __iter__(self):  # type: ignore
        return iter(self.slint_selector.fd_to_selector_key)


class _SlintSelector(selectors.BaseSelector):
    def __init__(self) -> None:
        self.fd_to_selector_key: typing.Dict[typing.Any, selectors.SelectorKey] = {}
        self.mapping = _SlintSelectorMapping(self)
        self.adapters: typing.Dict[int, native.AsyncAdapter] = {}

    def register(
        self, fileobj: typing.Any, events: typing.Any, data: typing.Any = None
    ) -> selectors.SelectorKey:
        fd = fd_for_fileobj(fileobj)
        key = selectors.SelectorKey(fileobj, fd, events, data)
        self.fd_to_selector_key[fd] = key

        adapter = native.AsyncAdapter(fd)
        self.adapters[fd] = adapter

        if events & selectors.EVENT_READ:
            adapter.wait_for_readable(self.read_notify)
        if events & selectors.EVENT_WRITE:
            adapter.wait_for_writable(self.write_notify)

        return key

    def unregister(self, fileobj: typing.Any) -> selectors.SelectorKey:
        fd = fd_for_fileobj(fileobj)
        key = self.fd_to_selector_key.pop(fd)

        try:
            del self.adapters[fd]
        except KeyError:
            pass

        return key

    def modify(
        self, fileobj: typing.Any, events: int, data: typing.Any = None
    ) -> selectors.SelectorKey:
        fd = fd_for_fileobj(fileobj)
        key = self.fd_to_selector_key[fd]

        if key.events != events:
            self.unregister(fileobj)
            key = self.register(fileobj, events, data)
        elif key.data != data:
            key._replace(data=data)
            self.fd_to_selector_key[fd] = key

        return key

    def select(
        self, timeout: float | None = None
    ) -> typing.List[typing.Tuple[selectors.SelectorKey, int]]:
        raise NotImplementedError

    def close(self) -> None:
        pass

    def get_map(self) -> Mapping[int | HasFileno, selectors.SelectorKey]:
        return self.mapping

    def read_notify(self, fd: int) -> None:
        key = self.fd_to_selector_key[fd]
        (reader, writer) = key.data
        reader._run()

    def write_notify(self, fd: int) -> None:
        key = self.fd_to_selector_key[fd]
        (reader, writer) = key.data
        writer._run()


class SlintEventLoop(asyncio.SelectorEventLoop):
    def __init__(self) -> None:
        self._is_running = False
        self._timers: typing.Set[native.Timer] = set()
        self.stop_run_forever_event = asyncio.Event()
        self._soon_tasks: typing.List[asyncio.TimerHandle] = []
        super().__init__(_SlintSelector())

    def run_forever(self) -> None:
        async def loop_stopper(event: asyncio.Event) -> None:
            await event.wait()
            native.quit_event_loop()

        asyncio.events._set_running_loop(self)
        self._is_running = True
        try:
            self.stop_run_forever_event = asyncio.Event()
            self.create_task(loop_stopper(self.stop_run_forever_event))
            native.run_event_loop()
        finally:
            self._is_running = False
            asyncio.events._set_running_loop(None)

    def run_until_complete[T](self, future: typing.Awaitable[T]) -> T | None:  # type: ignore[override]
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
        else:
            if self.stop_run_forever_event.is_set():
                raise RuntimeError("run_until_complete's future isn't done", future)
            else:
                # If the loop was quit for example because the user closed the last window, then
                # don't thrown an error but return a None sentinel. The return value of asyncio.run()
                # isn't used by slint.run_event_loop() anyway
                # TODO: see if we can properly cancel the future by calling cancel() and throwing
                # the task cancellation exception.
                return None

    def _run_forever_setup(self) -> None:
        pass

    def _run_forever_cleanup(self) -> None:
        pass

    def stop(self) -> None:
        self.stop_run_forever_event.set()

    def is_running(self) -> bool:
        return self._is_running

    def close(self) -> None:
        super().close()

    def is_closed(self) -> bool:
        return False

    def call_later(self, delay, callback, *args, context=None) -> asyncio.TimerHandle:  # type: ignore
        timer = native.Timer()

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
            native.TimerMode.SingleShot,
            interval=datetime.timedelta(seconds=delay),
            callback=timer_done_cb,
        )

        timers.add(timer)

        return handle

    def call_at(self, when, callback, *args, context=None) -> asyncio.TimerHandle:  # type: ignore
        return self.call_later(when - self.time(), callback, *args, context=context)

    def call_soon(self, callback, *args, context=None) -> asyncio.TimerHandle:  # type: ignore
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
        handle = asyncio.Handle(
            callback=callback,
            args=args,
            loop=self,
            context=context,
        )

        def run_handle_cb() -> None:
            if not handle._cancelled:
                handle._run()

        native.invoke_from_event_loop(run_handle_cb)
        return handle

    def _write_to_self(self) -> None:
        raise NotImplementedError
