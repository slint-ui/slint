# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cSpell:ignore socketpair

import asyncio
import contextlib
import gc
import platform
import socket
import sys
import threading
import time
import typing
import weakref
from datetime import timedelta

import aiohttp
import pytest
from aiohttp import web

import slint
import slint.loop
from slint import slint as native


def test_async_basic() -> None:
    async def quit_soon(call_check: list[bool]) -> None:
        await asyncio.sleep(1)
        call_check[0] = True
        slint.quit_event_loop()

    call_check = [False]

    slint.run_event_loop(quit_soon(call_check))

    assert call_check[0]


def test_async_aiohttp() -> None:
    def probe_port() -> int:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("127.0.0.1", 0))
        port = typing.cast(int, s.getsockname()[1])
        # This is a race condition, but should be good enough for test environments
        s.close()
        return port

    async def hello(request: web.Request) -> web.Response:
        return web.Response(text="Hello, world")

    async def run_network_requests(port: int, exceptions: list[Exception]) -> None:
        try:
            app = web.Application()
            app.add_routes([web.get("/", hello)])
            runner = web.AppRunner(app)
            await runner.setup()

            site = web.TCPSite(runner, "127.0.0.1", port)
            await site.start()

            async with (
                aiohttp.ClientSession() as session,
                session.get(f"http://127.0.0.1:{port}") as response,
            ):
                print("Status:", response.status)
                print("Content-type:", response.headers["content-type"])
                html = await response.text()
                print("Body:", html[:15], "...")
                assert html == "Hello, world"

            await runner.cleanup()
        except Exception as e:  # noqa: BLE001 -- surface any failure to the main thread
            exceptions.append(e)
        finally:
            slint.quit_event_loop()

    exceptions: list[Exception] = []
    slint.run_event_loop(run_network_requests(probe_port(), exceptions))
    assert len(exceptions) == 0


def test_basic_socket() -> None:
    def server_thread(server_socket: socket.socket) -> None:
        server_socket.listen(1)
        conn, _ = server_socket.accept()
        try:
            data = conn.recv(1024)
            if data == b"ping":
                conn.sendall(b"pong")
            else:
                conn.sendall(b"error")
        finally:
            conn.close()
            server_socket.close()

    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.bind(("127.0.0.1", 0))
    port = server_socket.getsockname()[1]
    thread = threading.Thread(target=server_thread, args=(server_socket,))
    thread.start()

    async def run_network_request(port: int) -> None:
        reader, writer = await asyncio.open_connection("127.0.0.1", port)

        writer.write(b"ping")
        await writer.drain()

        response = []
        while chunk := await reader.read(1024):
            response.append(chunk)

        writer.close()
        await writer.wait_closed()

        assert response[0] == b"pong"
        slint.quit_event_loop()

    slint.run_event_loop(run_network_request(port))
    thread.join()


def test_server_socket() -> None:
    async def handle_client(
        reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        data = await reader.read(1024)
        if data == b"ping":
            writer.write(b"pong")
        else:
            writer.write(b"error")
        await writer.drain()
        writer.close()
        await writer.wait_closed()

    async def run_network_request(port: int) -> None:
        try:
            reader, writer = await asyncio.open_connection("127.0.0.1", port)

            writer.write(b"ping")
            await writer.drain()

            response = []
            while chunk := await reader.read(1024):
                response.append(chunk)

            writer.close()
            await writer.wait_closed()

            assert response[0] == b"pong"
        finally:
            slint.quit_event_loop()

    async def run_server_and_client(exception_check: list[Exception]) -> None:
        try:
            server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
            port = server.sockets[0].getsockname()[1]

            async with server:
                await asyncio.gather(
                    server.serve_forever(),
                    run_network_request(port),
                )
        except Exception as e:
            exception_check.append(e)
            raise

    exception_check: list[Exception] = []
    slint.run_event_loop(run_server_and_client(exception_check))
    if len(exception_check) > 0:
        raise exception_check[0]


def test_loop_close_while_main_future_runs() -> None:
    def q() -> None:
        native.quit_event_loop()

    async def never_quit() -> None:
        loop = asyncio.get_running_loop()
        # Call native.quit_event_loop() directly as if the user closed the last window. We should gracefully
        # handle that the future that this function represents isn't terminated.
        loop.call_later(0.1, q)
        while True:
            await asyncio.sleep(1)

    try:
        slint.run_event_loop(never_quit())
    except Exception:  # noqa: BLE001 -- any exception means the test failed
        pytest.fail("Should not throw a run-time error")


def test_loop_continues_when_main_coro_finished() -> None:
    async def quit_later(quit_event: asyncio.Event) -> None:
        await quit_event.wait()
        slint.quit_event_loop()

    async def simple(quit_event: asyncio.Event) -> None:
        loop = asyncio.get_event_loop()
        loop.create_task(quit_later(quit_event))

    quit_event = asyncio.Event()
    slint.Timer.single_shot(
        duration=timedelta(milliseconds=100), callback=lambda: quit_event.set()
    )
    slint.run_event_loop(simple(quit_event))
    assert quit_event.is_set()


@pytest.mark.skipif(platform.system() == "Windows", reason="pipes aren't supported yet")
def test_subprocess() -> None:
    async def launch_process(exception_check: list[Exception]) -> None:
        try:
            proc = await asyncio.create_subprocess_exec(
                sys.executable,
                "--version",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.STDOUT,
            )

            stdout, _ = await proc.communicate()
            output = stdout.decode().strip()
            print(f"Process output: {output}")

            assert proc.returncode == 0
            assert output != ""
            slint.quit_event_loop()
        except Exception as e:
            exception_check[0] = e
            raise

    exception_check: list[Exception] = []
    slint.run_event_loop(launch_process(exception_check))
    if len(exception_check) > 0:
        raise exception_check[0]


def test_exception_thrown() -> None:
    async def throws() -> None:
        raise RuntimeError("Boo")

    with pytest.raises(RuntimeError, match="Boo"):
        slint.run_event_loop(throws())


# Guarded with `sys.platform != "win32"` (in addition to the pytest skipif) so
# that `ty` narrows the branch away on Windows, where `signal.SIGUSR1` is not
# defined in the stdlib stubs.
if sys.platform != "win32":

    def _start_quit_watchdog(done: list[bool]) -> None:
        """Ends the loop if the signal a test is waiting for never arrives.

        The queue outlives the loop, so the watchdog mustn't quit once the test
        no longer needs it: that quit would sit in the queue and cut a later
        test's loop short.
        """
        import threading
        import time

        def watchdog() -> None:
            time.sleep(5)
            if not done[0]:
                native.invoke_from_event_loop(slint.quit_event_loop)

        threading.Thread(target=watchdog, daemon=True).start()

    def test_add_signal_handler() -> None:
        import os
        import signal

        handler_called = [False]

        async def setup_and_signal() -> None:
            loop = asyncio.get_running_loop()

            def handler() -> None:
                handler_called[0] = True
                slint.quit_event_loop()

            loop.add_signal_handler(signal.SIGUSR1, handler)
            os.kill(os.getpid(), signal.SIGUSR1)

            await asyncio.sleep(5)
            slint.quit_event_loop()

        slint.run_event_loop(setup_and_signal())
        assert handler_called[0]

    def test_signal_wakes_idle_loop() -> None:
        # Signal delivered from a background thread while the loop has no pending
        # Python work — it must be parked in the native wait and woken by the
        # signal wakeup byte.
        import os
        import signal
        import threading
        import time

        handler_called = [False]

        def deliver_signal_later() -> None:
            time.sleep(0.2)
            os.kill(os.getpid(), signal.SIGUSR1)

        async def run() -> None:
            loop = asyncio.get_running_loop()

            def handler() -> None:
                handler_called[0] = True
                slint.quit_event_loop()

            loop.add_signal_handler(signal.SIGUSR1, handler)

            threading.Thread(target=deliver_signal_later, daemon=True).start()
            _start_quit_watchdog(handler_called)

            await asyncio.Event().wait()

        slint.run_event_loop(run())
        assert handler_called[0]

    def test_sigint_raises_keyboard_interrupt() -> None:
        # Without any explicit add_signal_handler, Ctrl-C (SIGINT) delivered
        # while the loop is parked idle in the native wait must be turned into a
        # KeyboardInterrupt that propagates out of run_event_loop(), rather than
        # being swallowed (which forces callers/harnesses to escalate to
        # SIGQUIT and crash the process).
        import os
        import signal
        import threading
        import time

        interrupted = [False]

        def deliver_sigint_later() -> None:
            time.sleep(0.2)
            os.kill(os.getpid(), signal.SIGINT)

        async def run() -> None:
            threading.Thread(target=deliver_sigint_later, daemon=True).start()
            _start_quit_watchdog(interrupted)
            await asyncio.Event().wait()

        try:
            with pytest.raises(KeyboardInterrupt):
                slint.run_event_loop(run())
        finally:
            interrupted[0] = True


def test_sleep_does_not_leak_timers() -> None:
    """Repeatedly awaiting asyncio.sleep() must not accumulate objects.

    `SlintEventLoop.call_later()` arms a native timer per call, so a timer that
    outlives its callback leaks once per await.
    See https://github.com/slint-ui/slint/issues/12679.
    """

    def live_objects() -> int:
        gc.collect()
        return len(gc.get_objects())

    async def run() -> None:
        # A non-zero delay so that both timers per await are exercised: asyncio
        # short-circuits sleep(0) without going through call_later().
        delay = 0.001

        # Warm up, so that one-time allocations don't count towards the baseline.
        for _ in range(100):
            await asyncio.sleep(delay)
        baseline = live_objects()

        for _ in range(500):
            await asyncio.sleep(delay)
        growth = live_objects() - baseline

        # Before the fix this grew by ~15 objects per iteration.
        assert growth < 100, f"leaked {growth} objects over 500 awaits"

        # The timers must not be reclaimed by a collection pass either: call_later()
        # is on the per-await hot path, so it has to avoid building cycles at all.
        for _ in range(500):
            await asyncio.sleep(delay)
        cyclic = gc.collect()
        assert cyclic < 100, f"{cyclic} cyclic objects over 500 awaits"

        slint.quit_event_loop()

    slint.run_event_loop(run())


def test_selector_adapter_cycle_is_collectable() -> None:
    """`_SlintSelector.register()` hands the adapter its own bound methods.

    The adapter keeps them behind an `Rc` the GC cannot see, so selector and adapter
    would keep each other alive for the lifetime of the process.
    """

    class Owner:
        def __init__(self, fd: int) -> None:
            self.adapter = native.AsyncAdapter(fd)
            self.adapter.wait_for_readable(self.on_readable)

        def on_readable(self, fd: int) -> None:
            pass

    reader, writer = socket.socketpair()
    try:
        owner = Owner(reader.fileno())
        weak_owner = weakref.ref(owner)

        del owner
        gc.collect()

        assert weak_owner() is None
    finally:
        reader.close()
        writer.close()


def test_cancelling_handle_disarms_native_timer() -> None:
    """Cancelling a `call_later()` handle must stop the timer it armed.

    Otherwise the timer stays armed until its original deadline, waking the loop for a
    callback that will not run and pinning itself until then.
    """

    async def run() -> None:
        loop = typing.cast(slint.loop.SlintEventLoop, asyncio.get_event_loop())

        called = False

        def never() -> None:
            nonlocal called
            called = True

        before = len(loop._timers)
        handle = loop.call_later(3600, never)
        assert len(loop._timers) == before + 1

        handle.cancel()
        assert len(loop._timers) == before, "native timer left armed after cancel()"

        # Cancelling twice must not raise.
        handle.cancel()

        await asyncio.sleep(0.05)
        assert not called

        slint.quit_event_loop()

    slint.run_event_loop(run())


def test_socket_traffic_does_not_busy_loop() -> None:
    """A trickle of socket data must not keep the event loop awake.

    See https://github.com/slint-ui/slint/issues/12962.
    """
    duration = 2.0
    interval = 0.1

    async def send_periodically(
        _reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        with contextlib.suppress(ConnectionResetError, BrokenPipeError):
            while True:
                writer.write(b"x")
                await asyncio.sleep(interval)

    async def run() -> None:
        received = 0

        server = await asyncio.start_server(send_periodically, "127.0.0.1", 0)
        port = server.sockets[0].getsockname()[1]
        reader, writer = await asyncio.open_connection("127.0.0.1", port)

        async def drain() -> None:
            nonlocal received
            while chunk := await reader.read(4096):
                received += len(chunk)

        drainer = asyncio.create_task(drain())
        cpu_before = time.process_time()
        await asyncio.sleep(duration)
        cpu_seconds = time.process_time() - cpu_before

        drainer.cancel()
        writer.close()
        server.close()

        # Liveness only, so that the CPU assert can't pass vacuously. Not a rate check:
        # timer granularity makes the rate unreliable on a loaded CI runner.
        assert received > 0, "no data received"
        # Before the fix: 60-100% of a core. Idle: under 5%.
        assert cpu_seconds < duration * 0.25, (
            f"burned {cpu_seconds:.2f}s of CPU over {duration}s ({received} bytes)"
        )

        slint.quit_event_loop()

    slint.run_event_loop(run())
