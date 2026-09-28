#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""End-to-end test of the slint / slint-dev PyPI distribution.

Publishes the built distributions to a local (synthetic) package index
(pypiserver), then installs them as a real consumer would (uv, against that
index) and verifies that:

  * installing `slint` stays lean: slint-dev is not pulled in, the lean native
    binary is loaded, and no development features are present — also when the
    dev capabilities are requested but slint-dev is not installed,
  * installing `slint[dev]` resolves the slint-dev wheel at the matching
    version, and the development binary (system-testing/mcp) is loaded only
    when requested via SLINT_TEST_SERVER / SLINT_MCP_PORT — exercised both
    directly and through the loader test suite, whose dev-binary tests must
    run (not skip) here, and
  * a slint-dev whose version does not match slint is refused with a warning,
    falling back to the lean release binary.

Two modes:

  * Gate (publish workflow): SLINT_E2E_WHEEL_DIR points at the real,
    already-built distributions and SLINT_E2E_REQUIRE_SDIST=1 additionally
    requires the sdist — see upload_pypi.yaml, where this blocks the publish
    jobs. (Platform completeness is the workflow's job: the gate depends on
    all build jobs, and the artifact uploads fail when no files were built.)
  * Dev (local): build the two wheels for the host and point
    SLINT_E2E_WHEEL_DIR at them (--compatibility linux skips the manylinux
    library bundling, which is not needed for a host-local install):

        cd api/python/slint
        uvx maturin build --release --compatibility linux --out /tmp/slint-e2e
        cd ../slint-dev
        uvx maturin build --release --compatibility linux --out /tmp/slint-e2e
        SLINT_E2E_WHEEL_DIR=/tmp/slint-e2e python3 ../slint/tests/registry/e2e.py

Requires `uv` on PATH; pypiserver and twine are fetched on the fly via uvx.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent  # api/python/slint/tests/registry
PACKAGE_DIR = HERE.parent.parent  # api/python/slint

# Reports which native module variant `import slint` picked.
PROBE = (
    "import json, slint;"
    "print(json.dumps({'name': slint.native.__name__,"
    " 'features': slint.native.build_features()}))"
)

LOADER_ENV_VARS = ("SLINT_TEST_SERVER", "SLINT_MCP_PORT")


def log(message: str) -> None:
    print(f"[e2e] {message}", flush=True)


def run(
    command: list[str | Path],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    log("+ " + " ".join(str(part) for part in command))
    try:
        return subprocess.run(
            [str(part) for part in command],
            check=True,
            text=True,
            cwd=cwd,
            env=env,
            capture_output=capture,
        )
    except subprocess.CalledProcessError as error:
        if error.stdout:
            print(error.stdout)
        if error.stderr:
            print(error.stderr, file=sys.stderr)
        raise


def probe_native(python: Path, extra_env: dict[str, str]) -> tuple[dict[str, Any], str]:
    env = {
        key: value for key, value in os.environ.items() if key not in LOADER_ENV_VARS
    }
    env.update(extra_env)
    result = run([python, "-c", PROBE], env=env, capture=True)
    # The JSON is the last stdout line; backend messages may precede it.
    info: dict[str, Any] = json.loads(result.stdout.strip().splitlines()[-1])
    log(f"  -> {info}")
    return info, result.stdout + result.stderr


def make_venv(work: Path, name: str) -> Path:
    venv = work / name
    run(["uv", "venv", "--python", "3.12", venv])
    if os.name == "nt":
        return venv / "Scripts" / "python.exe"
    return venv / "bin" / "python"


def check_artifacts(distributions: list[Path], version: str) -> None:
    names = [path.name for path in distributions]
    for name in names:
        log(f"  {name}")
    missing = []
    for name in names:
        if not (name.startswith("slint-") and name.endswith(".whl")):
            continue
        # slint-dev is not built for iOS; it is a development host tool.
        if name.removesuffix(".whl").rsplit("-", 1)[-1].startswith("ios_"):
            continue
        # Every other slint wheel must come with a slint-dev wheel of the
        # exact same version and python/abi/platform tags, so that
        # `slint[dev]` resolves wherever slint does.
        counterpart = "slint_dev-" + name.removeprefix("slint-")
        if counterpart not in names:
            missing.append(counterpart)
    if (
        os.environ.get("SLINT_E2E_REQUIRE_SDIST") == "1"
        and f"slint-{version}.tar.gz" not in names
    ):
        missing.append(f"slint-{version}.tar.gz (sdist)")
    if missing:
        sys.exit("missing distributions:\n  " + "\n  ".join(missing))


def start_index(work: Path) -> tuple[subprocess.Popen[bytes], str]:
    packages = work / "packages"
    packages.mkdir()
    server_log = work / "pypiserver.log"
    port = free_port()
    server = subprocess.Popen(
        # -P . -a . disables authentication, also for uploads.
        [
            "uvx",
            "--from",
            "pypiserver>=2,<3",
            "pypi-server",
            "run",
            "-p",
            str(port),
            "-P",
            ".",
            "-a",
            ".",
            str(packages),
        ],
        stdout=server_log.open("wb"),
        stderr=subprocess.STDOUT,
    )
    root = f"http://127.0.0.1:{port}/"
    deadline = time.time() + 120  # the first uvx run downloads pypiserver
    while True:
        try:
            urllib.request.urlopen(root + "simple/", timeout=5)
            break
        except (urllib.error.URLError, OSError):
            if server.poll() is not None or time.time() > deadline:
                sys.exit(f"local package index failed to start, see {server_log}")
            time.sleep(0.5)
    log(f"local package index listening on {root}")
    return server, root


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def publish(distributions: list[Path], root_url: str) -> None:
    run(["uvx", "twine", "check", *distributions])
    run(
        [
            "uvx",
            "twine",
            "upload",
            "--non-interactive",
            "--repository-url",
            root_url,
            "-u",
            "e2e",
            "-p",
            "e2e",
            *distributions,
        ]
    )


def scenario_lean(work: Path, simple_url: str, version: str) -> None:
    log("scenario: lean production install")
    python = make_venv(work, "consumer-lean")
    run(
        [
            "uv",
            "pip",
            "install",
            "--python",
            python,
            "--index-url",
            simple_url,
            f"slint=={version}",
        ]
    )

    # A plain install must not pull in slint-dev.
    run(
        [
            python,
            "-c",
            (
                "import importlib.util, sys;"
                "sys.exit(1 if importlib.util.find_spec('slint_dev_native') else 0)"
            ),
        ]
    )

    info, _ = probe_native(python, {})
    assert info["name"] == "slint.slint", info
    assert info["features"] == [], info

    # Requesting the dev capabilities without slint-dev installed must fall
    # back to the lean binary instead of failing.
    info, _ = probe_native(python, {"SLINT_MCP_PORT": "9315"})
    assert info["name"] == "slint.slint", info


def scenario_dev(work: Path, simple_url: str, version: str) -> None:
    log("scenario: development install (slint[dev])")
    python = make_venv(work, "consumer-dev")
    # The dev extra's test dependencies (pytest etc.) are not on the local
    # index, so PyPI is needed as the fallback index. `--index` takes priority
    # over `--default-index`, so slint and slint-dev resolve from the local
    # index (uv only considers the first index that contains a package).
    run(
        [
            "uv",
            "pip",
            "install",
            "--python",
            python,
            "--index",
            simple_url,
            "--default-index",
            "https://pypi.org/simple/",
            f"slint[dev]=={version}",
        ]
    )

    installed = run(
        [
            python,
            "-c",
            "import importlib.metadata; print(importlib.metadata.version('slint-dev'))",
        ],
        capture=True,
    ).stdout.strip()
    assert installed == version, (
        f"slint-dev resolved to {installed}, expected {version}"
    )

    # The dev binary is used only when requested via the environment.
    info, _ = probe_native(python, {"SLINT_TEST_SERVER": "1"})
    assert info["name"] == "slint_dev_native", info
    assert "system-testing" in info["features"], info
    assert "mcp" in info["features"], info
    info, _ = probe_native(python, {})
    assert info["name"] == "slint.slint", info
    assert info["features"] == [], info

    # Run the loader test suite against the installed packages, from a separate
    # directory so the repository's `slint/` source tree cannot shadow the
    # installed package. With the dev binary present, its e2e tests must run.
    tests = work / "loader-tests"
    tests.mkdir()
    shutil.copy(PACKAGE_DIR / "tests" / "test_native_loader.py", tests)
    result = run(
        [python, "-m", "pytest", "test_native_loader.py", "-v"],
        cwd=tests,
        capture=True,
    )
    print(result.stdout)
    assert "skipped" not in result.stdout, "loader tests were skipped"

    # A version mismatch must warn and fall back to the lean binary; rewrite
    # the installed slint-dev metadata to simulate one.
    purelib = Path(
        run(
            [python, "-c", "import sysconfig; print(sysconfig.get_paths()['purelib'])"],
            capture=True,
        ).stdout.strip()
    )
    metadata = next(purelib.glob("slint_dev-*.dist-info")) / "METADATA"
    metadata.write_text(
        re.sub(
            r"^Version: .*$",
            "Version: 0.0.0",
            metadata.read_text(),
            count=1,
            flags=re.MULTILINE,
        )
    )
    info, output = probe_native(python, {"SLINT_MCP_PORT": "9315"})
    assert "does not match" in output, output
    assert info["name"] == "slint.slint", info
    assert info["features"] == [], info


def main() -> None:
    wheel_dir = os.environ.get("SLINT_E2E_WHEEL_DIR")
    if not wheel_dir:
        sys.exit("SLINT_E2E_WHEEL_DIR is not set (see the module docstring for usage)")
    distributions = sorted(
        path
        for path in Path(wheel_dir).resolve().iterdir()
        if path.suffix == ".whl" or path.name.endswith(".tar.gz")
    )
    slint_wheels = [
        path
        for path in distributions
        if path.suffix == ".whl" and path.name.startswith("slint-")
    ]
    if not slint_wheels:
        sys.exit(f"no slint wheels found in {wheel_dir}")
    version = slint_wheels[0].name.split("-")[1]
    log(f"distribution version: {version}")
    check_artifacts(distributions, version)

    work = Path(tempfile.mkdtemp(prefix="slint-pypi-e2e-"))
    log(f"working directory: {work}")
    server, root_url = start_index(work)
    try:
        publish(distributions, root_url)
        simple_url = root_url + "simple/"
        scenario_lean(work, simple_url, version)
        scenario_dev(work, simple_url, version)
    finally:
        server.terminate()
        server.wait(timeout=10)
    log("all end-to-end scenarios passed")


if __name__ == "__main__":
    main()
