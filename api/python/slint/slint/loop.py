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


def fd_for_fileobj(fileobj) -> int:
    if isinstance(fileobj, int):
        return fileobj
    return int(fileobj.fileno())


class _SlintSelectorMapping(Mapping):
    def __init__(self, slint_selector):
        self.slint_selector = slint_selector

    def __len__(self):
        return len(self.slint_selector.fd_to_selector_key)

    def get(self, fileobj, default=None):
        fd = fd_for_fileobj(fileobj)
        return self.slint_selector.fd_to_selector_key.get(fd, default)

    def __getitem__(self, fileobj):
        fd = fd_for_fileobj(fileobj)
        return self.slint_selector.fd_to_selector_key.get(fd)

    def __iter__(self):
        return iter(self.slint_selector.fd_to_selector_key)


class _SlintSelector(selectors.BaseSelector):
    def __init__(self) -> None:
        self.fd_to_selector_key = {}
        self.mapping = _SlintSelectorMapping(self)
        self.adapters = {}

    def register(
        self, fileobj: typing.Any, events: typing.Any, data: typing.Any = None
    ) -> selectors.SelectorKey:
        fd = fd_for_fileobj(fileobj)
        key = selectors.SelectorKey(fileobj, fd, events, data)
        self.fd_to_selector_key[fd] = key
        print("REGISTER ", fd, " for ", events)

        adapter = native.AsyncAdapter(fd)
        self.adapters[fd] = adapter

        if events & selectors.EVENT_READ:
            adapter.wait_for_readable(self.read_notify)
        if events & selectors.EVENT_WRITE:
            adapter.wait_for_writable(self.write_notify)

    def unregister(self, fileobj) -> selectors.SelectorKey:
        fd = fd_for_fileobj(fileobj)
        key = self.fd_to_selector_key.pop(fd)

        try:
            del self.adapters[fd]
        except KeyError:
            pass

        return key

    def modify(self, fileobj, events, data=None) -> selectors.SelectorKey:
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
        self, timeout=None
    ) -> typing.List[typing.Tuple[selectors.SelectorKey, typing.IO]]:
        raise NotImplementedError

    def close(self) -> None:
        pass

    def get_map(self) -> Mapping[typing.IO, selectors.SelectorKey]:
        return self.mapping

    def read_notify(self, fd):
        key = self.fd_to_selector_key[fd]
        (reader, writer) = key.data
        reader._run()

    def write_notify(self, fd):
        key = self.fd_to_selector_key[fd]
        (reader, writer) = key.data
        writer._run()


class SlintEventLoop(asyncio.SelectorEventLoop):
    def __init__(self) -> None:
        self._is_running = False
        self._timers = set()
        self.stop_run_forever_event = asyncio.Event()
        super().__init__(_SlintSelector())

    def run_forever(self) -> None:
        async def loop_stopper(event):
            await event.wait()
            native.quit_event_loop()

        asyncio.events._set_running_loop(self)
        self._is_running = True
        try:
            self.stop_run_forever_event.clear()
            task = self.create_task(loop_stopper(self.stop_run_forever_event))
            native.run_event_loop()
        finally:
            self._is_running = False
            asyncio.events._set_running_loop(None)

    def run_until_complete(self, future: asyncio.Future) -> None:
        def stop_loop(future):
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
            raise RuntimeError("run_until_complete's future isn't done", future)

    def _run_forever_setup(self):
        pass

    def _run_forever_cleanup(self):
        pass

    def stop(self) -> None:
        self.stop_run_forever_event.set()

    def is_running(self) -> bool:
        return self._is_running

    def close(self) -> None:
        # raise NotImplementedError
        #pass
        super().close()

    def is_closed(self) -> bool:
        # raise NotImplementedError
        return False

    def call_later(self, delay, callback, *args, context=None) -> asyncio.Handle:
        timer = native.Timer()

        handle = asyncio.TimerHandle(
            when=self.time() + delay,
            callback=callback,
            args=args,
            loop=self,
            context=context,
        )

        timers = self._timers

        def timer_done_cb():
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

    def call_at(self, when, callback, *args, context=None) -> asyncio.Handle:
        return self.call_later(when - self.time(), callback, *args, context=context)

    def call_soon(self, callback, *args, context=None) -> asyncio.Handle:
        return self.call_later(0, callback, *args, context=context)

    def call_soon_threadsafe(self, callback, *args, context=None) -> asyncio.Handle:
        handle = asyncio.Handle(
            callback=callback,
            args=args,
            loop=self,
            context=context,
        )

        def run_handle_cb():
            if not handle._cancelled:
                handle._run()

        native.invoke_from_event_loop(run_handle_cb)
        return handle
    #    raise NotImplementedError

    def _write_to_self(self):
        raise NotImplementedError

    #def add_signal_handler(self, sig, callback, *args):
    #    pass
