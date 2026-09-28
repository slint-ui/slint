# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Resolves which native Slint extension module to load.

There are two binary-compatible variants of the native extension, exposing the
exact same surface:

  * the default lean one (``slint.slint``), shipped with the ``slint`` package,
    and
  * the "dev" one (the top-level ``slint_dev_native`` module), shipped by the
    optional ``slint-dev`` package, which additionally has the ``system-testing``
    and ``mcp`` features compiled in.

The dev binary is only loaded when its capabilities are actually requested via
the environment — ``SLINT_TEST_SERVER`` (system testing) or ``SLINT_MCP_PORT``
(MCP server) — so a plain ``import slint`` stays on the lean release binary even
when ``slint-dev`` is installed. The variable must be set before ``slint`` is
first imported, as the choice is made here at import time. A version check
refuses a ``slint-dev`` whose version does not match ``slint``, to avoid pairing
incompatible Python glue and native code.

Use ``slint.native.build_features()`` to query which capabilities the loaded
binary has.
"""

import importlib
import importlib.metadata
import os
import warnings
from types import ModuleType
from typing import TYPE_CHECKING


def _dev_requested() -> bool:
    return bool(os.environ.get("SLINT_TEST_SERVER") or os.environ.get("SLINT_MCP_PORT"))


def _load_native() -> ModuleType:
    if _dev_requested():
        try:
            dev_version = importlib.metadata.version("slint-dev")
        except importlib.metadata.PackageNotFoundError:
            dev_version = None

        if dev_version is not None:
            own_version = importlib.metadata.version("slint")
            if dev_version != own_version:
                warnings.warn(
                    f"Ignoring slint-dev {dev_version}: it does not match slint "
                    f"{own_version}. Install slint-dev=={own_version} to enable the "
                    "development binary with system-testing and MCP support.",
                    stacklevel=2,
                )
            else:
                try:
                    return importlib.import_module("slint_dev_native")
                except ImportError as error:
                    # Installed, but its native binary is unavailable on this
                    # platform. Fall back to the lean release binary.
                    warnings.warn(
                        f"Could not load the slint-dev development binary: {error}",
                        stacklevel=2,
                    )

    from . import slint as native

    return native


if TYPE_CHECKING:
    # Both binary variants expose the same surface, so type-check against the
    # bundled extension's stub (slint.pyi).
    from . import slint as native
else:
    native = _load_native()
