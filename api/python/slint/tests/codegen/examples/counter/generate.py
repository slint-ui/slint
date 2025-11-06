# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

from pathlib import Path

from slint.codegen.generator import generate_project
from slint.codegen.models import GenerationConfig


def main() -> None:
    base_dir = Path(__file__).parent
    config = GenerationConfig(
        include_paths=[base_dir],
        library_paths={},
        style=None,
        translation_domain=None,
        quiet=False,
    )

    generate_project(
        inputs=[base_dir / "counter.slint"], output_dir=None, config=config
    )
    print("Generated Python bindings next to counter.slint")


if __name__ == "__main__":
    main()
