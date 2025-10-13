# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import load_file
import slint
import pytest
from pathlib import Path
import asyncio


def base_dir() -> Path:
    origin = __spec__.origin
    assert origin is not None
    base_dir = Path(origin).parent
    assert base_dir is not None
    return base_dir


def test_callback_decorators(caplog: pytest.LogCaptureFixture) -> None:
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

    class SubClass(module.App):  # type: ignore
        @slint.callback()
        def say_hello_again(self, arg: str) -> str:
            return "say_hello_again:" + arg

        @slint.callback(name="say-hello")
        def renamed(self, arg: str) -> str:
            return "renamed:" + arg

        @slint.callback(global_name="MyGlobal", name="global-callback")
        def global_callback(self, arg: str) -> str:
            return "global:" + arg

    instance = SubClass()
    assert instance.invoke_say_hello("ok") == "renamed:ok"
    assert instance.invoke_say_hello_again("ok") == "say_hello_again:ok"
    assert instance.invoke_global_callback("ok") == "global:ok"
    del instance


def test_callback_decorators_async() -> None:
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

    class SubClass(module.App):  # type: ignore
        def __init__(self, in_queue: asyncio.Queue[int], out_queue: asyncio.Queue[int]):
            super().__init__()
            self.in_queue = in_queue
            self.out_queue = out_queue

        @slint.callback()
        async def call_void(self) -> None:
            value = await self.in_queue.get()
            await self.out_queue.put(value + 1)

    async def main(
        instance: SubClass, in_queue: asyncio.Queue[int], out_queue: asyncio.Queue[int]
    ) -> None:
        await in_queue.put(42)
        instance.invoke_call_void()
        assert await out_queue.get() == 43
        slint.quit_event_loop()

    in_queue: asyncio.Queue[int] = asyncio.Queue()
    out_queue: asyncio.Queue[int] = asyncio.Queue()
    instance = SubClass(in_queue, out_queue)
    slint.run_event_loop(main(instance, in_queue, out_queue))


def test_callback_decorators_async_err() -> None:
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

    class SubClass(module.App):  # type: ignore
        def __init__(self) -> None:
            super().__init__()

        @slint.callback()
        async def say_hello(self, msg: str) -> str:
            return msg

    with pytest.raises(RuntimeError) as excinfo:
        SubClass()
    err = excinfo.value
    assert (
        str(err)
        == "Callback 'say_hello' cannot be used with a callback decorator for an async function, as it doesn't return void"
    )
