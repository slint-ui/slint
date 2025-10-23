from __future__ import annotations

from pathlib import Path

from slint.codegen.generator import generate_project
from slint.codegen.models import GenerationConfig


def main() -> None:
    base_dir = Path(__file__).parent
    output = base_dir / "generated"
    config = GenerationConfig(
        include_paths=[base_dir],
        library_paths={},
        style=None,
        translation_domain=None,
        quiet=False,
    )

    generate_project(inputs=[base_dir / "counter.slint"], output_dir=output, config=config)
    print(f"Generated Python bindings into {output.relative_to(base_dir)}")


if __name__ == "__main__":
    main()
