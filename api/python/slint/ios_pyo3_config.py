# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Write the PyO3 build configuration for the iOS wheels.

Since maturin 1.14.1 every abi3 build gets a generated PYO3_CONFIG_FILE whose
abi3 floor doubles as the libpython to link against. iOS is the only wheel
target that links libpython, and its framework ships only the interpreter this
build runs against, so the generated file names a libpython that isn't there.
maturin leaves PYO3_CONFIG_FILE alone once it is set, so write it here instead.

Run this with the interpreter the wheel is built for: everything but the abi3
floor comes from it, including the extension suffix, which maturin derives on
its own for every platform except iOS.
"""

import sys
import sysconfig
from pathlib import Path

# The abi3 floor of the wheel. Keep in sync with the pyo3 feature in Cargo.toml.
ABI3_FLOOR = "3.11"


def config_var(name: str) -> str:
    value = sysconfig.get_config_var(name)
    if not value:
        raise SystemExit(f"{sys.executable} reports no {name}")
    return str(value)


def main() -> None:
    version = f"{sys.version_info.major}.{sys.version_info.minor}"
    shared = "true" if sysconfig.get_config_var("Py_ENABLE_SHARED") else "false"
    config = "\n".join(
        [
            "implementation=CPython",
            f"version={version}",
            f"target_abi=CPython-abi3-{ABI3_FLOOR}",
            f"shared={shared}",
            f"lib_name=python{version}",
            f"lib_dir={config_var('LIBDIR')}",
            f"ext_suffix={config_var('EXT_SUFFIX')}",
            "build_flags=",
            "suppress_build_script_link_lines=false",
            "pointer_width=64",
        ]
    )
    Path(sys.argv[1]).write_text(config + "\n")
    print(f"{sys.argv[1]} (from {sys.executable}):\n{config}")


if __name__ == "__main__":
    main()
