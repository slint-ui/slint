# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

import nox

@nox.session(python="3.10")
def python(session: nox.Session):
    session.env["MATURIN_PEP517_ARGS"] = "--profile=dev"
    session.install(".[dev]")
    session.run("pytest", "-s")
