# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint
from slint import slint as native
import asyncio
import typing
import aiohttp


def test_async_basic() -> None:
    async def quit_soon(call_check: typing.List[bool]) -> None:
        # reader, writer = await asyncio.open_connection(
        #        '127.0.0.1', 8888)
        # async with aiohttp.ClientSession() as session:
        #        async with session.get('http://python.org') as response:
        #
        #            print("Status:", response.status)
        #            print("Content-type:", response.headers['content-type'])
        #
        #            html = await response.text()
        #            print("Body:", html[:15], "...")

        await asyncio.sleep(1)
        call_check[0] = True
        slint.quit_event_loop()

    call_check = [False]

    slint.run_event_loop(quit_soon(call_check))

    assert call_check[0]


def test_async_aiohttp() -> None:
    async def run_network_requests() -> None:
        # reader, writer = await asyncio.open_connection(
        #        '127.0.0.1', 8888)
        async with aiohttp.ClientSession() as session:
            async with session.get("http://python.org") as response:
                #
                print("Status:", response.status)
                print("Content-type:", response.headers["content-type"])
                #
                html = await response.text()
                print("Body:", html[:15], "...")
                slint.quit_event_loop()

    slint.run_event_loop(run_network_requests())
