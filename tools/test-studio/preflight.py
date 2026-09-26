# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import hashlib
import json
import os
import subprocess
from pathlib import Path

from runner import checked_command, environment

BUILD_COMMAND = "SLINT_EMIT_DEBUG_INFO=1 SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 cargo build --locked -p slint-editor --all-features --features slint/mcp"
PYTHON_CHECK = """import json, sys, importlib.metadata as m
import pytest, slint_testing, PIL
print(json.dumps({"python": sys.version, "executable": sys.executable, "packages": {p: m.version(p) for p in ["pytest", "slint-testing", "pillow"]}}))
"""


def digest(paths, cancel):
    sha = hashlib.sha256()
    for path in paths:
        sha.update(str(path).encode())
        with path.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                if cancel.is_set():
                    raise InterruptedError("Operation cancelled")
                sha.update(chunk)
    return sha.hexdigest()


def file_sha(path, cancel):
    sha = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            if cancel.is_set():
                raise InterruptedError("Operation cancelled")
            sha.update(chunk)
    return sha.hexdigest()


def validate_project(project):
    repo = Path(project["repo"]).expanduser().absolute()
    suite = repo / "tools/editor/ui-tests"
    if not suite.is_dir():
        raise ValueError(f"Visual Editor test suite not found: {suite}")
    python = Path(project["python"]).expanduser().absolute()
    if not python.is_file() or not os.access(python, os.X_OK):
        raise ValueError(
            f"Test interpreter is not executable: {python}\nRun: cd '{suite}' && uv sync --locked"
        )
    if not project["paths"] or any(
        not p.strip() or p.startswith("-") for p in project["paths"]
    ):
        raise ValueError(
            "Discovery paths must be nonempty paths or pytest node IDs, not command options."
        )
    if project["backend"] not in ("headless-skia", "winit-skia"):
        raise ValueError("Choose headless-skia or winit-skia.")
    return repo, suite, python


def check_environment(project, directory, cancel):
    repo, suite, python = validate_project(project)
    try:
        output = checked_command(
            [python, "-c", PYTHON_CHECK],
            cwd=suite,
            env=environment(project["binary"], project["backend"]),
            directory=directory,
            name="python-check",
            cancel=cancel,
            timeout=10,
        )
        result = json.loads(output)
    except (RuntimeError, ValueError) as error:
        raise RuntimeError(
            f"{error}\nPrepare the test environment: cd '{suite}' && uv sync --locked"
        ) from error
    for key, args in (
        ("revision", ["rev-parse", "HEAD"]),
        ("dirty", ["status", "--porcelain"]),
    ):
        completed = subprocess.run(
            ["git", "-C", str(repo), *args],
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        result[key] = completed.stdout.strip() if completed.returncode == 0 else None
    if result["dirty"] is not None:
        result["dirty"] = bool(result["dirty"])
    result["backend"] = project["backend"]
    return result


def preflight(project, directory, cancel, cache, info):
    repo, suite, python = validate_project(project)
    binary = Path(project["binary"])
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError(
            f"Editor binary is not executable: {binary}\nBuild from {repo}:\n{BUILD_COMMAND}"
        )
    info["binary_sha256"] = file_sha(binary, cancel)
    info["binary_path"] = str(binary)
    harness = sorted((suite / "tests").rglob("*.py")) + [suite / "pyproject.toml"]
    generic = Path(__file__).resolve().parents[1] / "slint-test" / "slint_test"
    library_files = sorted(generic.glob("*.py"))
    info["testing_library_sha256"] = digest(library_files, cancel)
    info["harness_sha256"] = digest(harness, cancel)
    key = (
        info["binary_sha256"],
        file_sha(python, cancel),
        info["harness_sha256"],
        info["testing_library_sha256"],
        json.dumps(info["packages"], sort_keys=True),
        str(python),
        info["python"],
        project["backend"],
    )
    if key in cache:
        return {**info, "capabilities": cache[key], "probe_cached": True}
    env = environment(binary, project["backend"])
    try:
        output = checked_command(
            [binary, "--test-capabilities"],
            cwd=repo,
            env=env,
            directory=directory,
            name="capabilities",
            cancel=cancel,
            timeout=5,
        )
        capabilities = json.loads(output)
        if capabilities.get("schema_version") != 1:
            raise ValueError("Unsupported capability schema")
        if not capabilities.get("system_testing") or project[
            "backend"
        ] not in capabilities.get("backends", []):
            raise ValueError(
                f"Binary does not support testing with {project['backend']}"
            )
    except (RuntimeError, ValueError, TimeoutError) as error:
        raise RuntimeError(
            f"Editor capability check failed: {error}\nRebuild this editor:\n{BUILD_COMMAND}"
        ) from error
    checked_command(
        [python, Path(__file__).with_name("probe.py"), binary],
        cwd=suite,
        env=env,
        directory=directory,
        name="application-probe",
        cancel=cancel,
        timeout=20,
    )
    cache[key] = capabilities
    return {**info, "capabilities": capabilities, "probe_cached": False}
