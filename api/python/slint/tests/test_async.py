# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint
from slint import slint as native
import asyncio
import typing
import aiohttp
import socket
import threading


def test_async_basic() -> None:
    async def quit_soon(call_check: typing.List[bool]) -> None:
        await asyncio.sleep(1)
        call_check[0] = True
        slint.quit_event_loop()

    call_check = [False]

    slint.run_event_loop(quit_soon(call_check))

    assert call_check[0]


def test_async_aiohttp() -> None:
    async def run_network_requests() -> None:
        async with aiohttp.ClientSession() as session:
            async with session.get("http://python.org") as response:
                #
                print("Status:", response.status)
                print("Content-type:", response.headers["content-type"])
                #
                html = await response.text()
                print("Body:", html[:15], "...")
                assert len(html) > 0
                slint.quit_event_loop()

    slint.run_event_loop(run_network_requests())


def test_basic_socket() -> None:
    def server_thread(server_socket):
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

    async def run_network_request(port) -> None:
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
