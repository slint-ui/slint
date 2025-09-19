# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint
from slint import slint as native
import asyncio
import typing
import aiohttp
from aiohttp import web
import socket
import threading
import pytest
import sys
import platform
from datetime import timedelta


def test_async_basic() -> None:
    async def quit_soon(call_check: typing.List[bool]) -> None:
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

    async def run_network_requests(
        port: int, exceptions: typing.List[Exception]
    ) -> None:
        try:
            app = web.Application()
            app.add_routes([web.get("/", hello)])
            runner = web.AppRunner(app)
            await runner.setup()

            site = web.TCPSite(runner, "127.0.0.1", port)
            await site.start()

            async with aiohttp.ClientSession() as session:
                async with session.get(f"http://127.0.0.1:{port}") as response:
                    #
                    print("Status:", response.status)
                    print("Content-type:", response.headers["content-type"])
                    #
                    html = await response.text()
                    print("Body:", html[:15], "...")
                    assert html == "Hello, world"

            await runner.cleanup()
        except Exception as e:
            exceptions.append(e)
        finally:
            slint.quit_event_loop()

    exceptions: typing.List[Exception] = []
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

    async def run_server_and_client(exception_check: typing.List[Exception]) -> None:
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

    exception_check: typing.List[Exception] = []
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
    except Exception:
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
    async def launch_process(exception_check: typing.List[Exception]) -> None:
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

    exception_check: typing.List[Exception] = []
    slint.run_event_loop(launch_process(exception_check))
    if len(exception_check) > 0:
        raise exception_check[0]


def test_exception_thrown() -> None:
    async def throws() -> None:
        raise RuntimeError("Boo")

    with pytest.raises(RuntimeError, match="Boo"):
        slint.run_event_loop(throws())
