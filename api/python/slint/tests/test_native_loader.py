# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import importlib.metadata
import importlib.util
import json
import os
import subprocess
import sys
import warnings

import pytest

import slint
from slint import _native

# The development binary (slint-dev / slint_dev_native) is only present in an
# end-to-end environment where the dev wheel was installed alongside slint;
# the tests that exercise the positive load path are skipped otherwise.
_HAS_DEV_BINARY = importlib.util.find_spec("slint_dev_native") is not None


def test_build_features_is_a_list() -> None:
    features = slint.native.build_features()
    assert isinstance(features, list)
    assert all(isinstance(feature, str) for feature in features)


def test_lean_build_has_no_dev_features() -> None:
    # The default `slint` wheel must not ship the system-testing/MCP code.
    features = slint.native.build_features()
    assert "system-testing" not in features
    assert "mcp" not in features


def test_dev_not_requested_without_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("SLINT_TEST_SERVER", raising=False)
    monkeypatch.delenv("SLINT_MCP_PORT", raising=False)
    assert _native._dev_requested() is False


@pytest.mark.parametrize("var", ["SLINT_TEST_SERVER", "SLINT_MCP_PORT"])
def test_dev_requested_with_env(monkeypatch: pytest.MonkeyPatch, var: str) -> None:
    monkeypatch.delenv("SLINT_TEST_SERVER", raising=False)
    monkeypatch.delenv("SLINT_MCP_PORT", raising=False)
    monkeypatch.setenv(var, "8080")
    assert _native._dev_requested() is True


def test_falls_back_to_lean_when_dev_not_installed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("SLINT_TEST_SERVER", "1")

    def fake_version(name: str) -> str:
        if name == "slint-dev":
            raise importlib.metadata.PackageNotFoundError(name)
        return "1.0.0"

    monkeypatch.setattr(importlib.metadata, "version", fake_version)

    native = _native._load_native()
    # The lean bundled binary, exposing the same surface.
    assert hasattr(native, "build_features")


def test_version_mismatch_warns_and_falls_back(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("SLINT_MCP_PORT", "8080")

    def fake_version(name: str) -> str:
        return "9.9.9-dev" if name == "slint-dev" else "1.0.0"

    monkeypatch.setattr(importlib.metadata, "version", fake_version)

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        native = _native._load_native()

    assert any("does not match" in str(w.message) for w in caught)
    # Mismatch must not load the dev binary; the lean one is used instead.
    assert hasattr(native, "build_features")


def _import_slint_in_subprocess(trigger: dict[str, str]) -> dict:
    """Import slint in a fresh interpreter and report the native module it picked.

    The dev/lean choice is made once at `import slint` time, so it has to be
    exercised in a separate process with the environment set up beforehand.
    """
    env = dict(os.environ)
    env.pop("SLINT_TEST_SERVER", None)
    env.pop("SLINT_MCP_PORT", None)
    env.update(trigger)
    code = (
        "import json, slint;"
        "print(json.dumps({'name': slint.native.__name__, "
        "'features': slint.native.build_features()}))"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    # The native backend may print diagnostics; the JSON is the last stdout line.
    return json.loads(result.stdout.strip().splitlines()[-1])


@pytest.mark.skipif(
    not _HAS_DEV_BINARY, reason="slint-dev development binary not installed"
)
@pytest.mark.parametrize("trigger", ["SLINT_TEST_SERVER", "SLINT_MCP_PORT"])
def test_dev_binary_is_loaded_when_requested(trigger: str) -> None:
    info = _import_slint_in_subprocess({trigger: "9315"})
    assert info["name"] == "slint_dev_native"
    assert "system-testing" in info["features"]
    assert "mcp" in info["features"]


@pytest.mark.skipif(
    not _HAS_DEV_BINARY, reason="slint-dev development binary not installed"
)
def test_dev_binary_not_loaded_without_request() -> None:
    # Even with slint-dev installed, a plain import stays on the lean binary.
    info = _import_slint_in_subprocess({})
    assert info["name"] != "slint_dev_native"
    assert "system-testing" not in info["features"]
    assert "mcp" not in info["features"]
