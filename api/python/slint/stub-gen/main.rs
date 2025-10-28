// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3_stub_gen::Result;

fn main() -> Result<()> {
    let stub = core::stub_info()?;
    stub.generate()?;
    Ok(())
}
