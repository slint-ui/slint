// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::Model;

use async_compat::Compat;

slint::slint! {
    export { MainWindow, Symbol } from "stockticker.slint";
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonSymbol {
    symbol: String,
    close: f32,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct JsonSymbols {
    symbols: Vec<JsonSymbol>,
}

async fn refresh_stocks(model: slint::ModelRc<Symbol>) {
    let url = format!(
        "https://stooq.com/q/l/?s={}&f=sd2t2ohlcvn&h&e=json",
        model.iter().map(|symbol| symbol.name.clone()).collect::<Vec<_>>().join("+")
    );

    let response = match reqwest::get(url).await {
        Ok(response) => response,
        Err(err) => {
            eprintln!("Error fetching update: {err}");
            return;
        }
    };

    let json_symbols: JsonSymbols = match response.json().await {
        Ok(json) => json,
        Err(err) => {
            eprintln!("Error decoding json response: {err}");
            return;
        }
    };

    for row in 0..model.row_count() {
        let mut symbol = model.row_data(row).unwrap();
        let Some(json_symbol) = json_symbols.symbols.iter().find(|s| *s.symbol == *symbol.name)
        else {
            continue;
        };
        symbol.price = json_symbol.close;
        model.set_row_data(row, symbol);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    let model = slint::VecModel::from_slice(&[
        Symbol { name: "AAPL.US".into(), price: 0.0 },
        Symbol { name: "MSFT.US".into(), price: 0.0 },
        Symbol { name: "AMZN.US".into(), price: 0.0 },
    ]);

    main_window.set_stocks(model.clone().into());

    main_window.show()?;

    slint::spawn_local(Compat::new(refresh_stocks(model.clone()))).unwrap();

    main_window.on_refresh(move || {
        slint::spawn_local(Compat::new(refresh_stocks(model.clone()))).unwrap();
    });

    main_window.run()
}
