# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import slint
import asyncio
import aiohttp

Symbol = slint.loader.stockticker.Symbol


async def refresh_stocks(model: slint.ListModel[Symbol]) -> None:
    STOOQ_URL = "https://stooq.com/q/l/?s={symbols}&f=sd2t2ohlcvn&h&e=json"
    url = STOOQ_URL.format(symbols="+".join([symbol.name for symbol in model]))
    async with aiohttp.ClientSession() as session:
        async with session.get(url) as resp:
            json = await resp.json()
            json_symbols = json["symbols"]
            for row, symbol in enumerate(model):
                data_for_symbol = next(
                    (sym for sym in json_symbols if sym["symbol"] == symbol.name), None
                )
                if data_for_symbol:
                    symbol.price = data_for_symbol["close"]
                    model.set_row_data(row, symbol)


class MainWindow(slint.loader.stockticker.MainWindow):
    def __init__(self):
        super().__init__()
        self.stocks = slint.ListModel(
            [Symbol(name=name, price=0.0) for name in ["AAPL.US", "MSFT.US", "AMZN.US"]]
        )

    @slint.callback
    async def refresh(self):
        await refresh_stocks(self.stocks)


async def main() -> None:
    main_window = MainWindow()
    main_window.refresh()
    main_window.show()


slint.run_event_loop(main())
