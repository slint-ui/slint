# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import importlib.util
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path
from types import SimpleNamespace

import pytest

from events import Writer
from preflight import preflight
from runner import Command, checked_command
from runner import TestProcess as Process
from state import RunState, matching, record, rerun_selection, tree_rows
from storage import Store


def finish(process):
    events = []
    deadline = time.monotonic() + 10
    try:
        while time.monotonic() < deadline:
            events.extend(process.poll())
            if process.finished:
                return events
            time.sleep(0.02)
        pytest.fail(process.log_path.read_text())
    finally:
        process.close()


@pytest.fixture
def suite(tmp_path):
    path = tmp_path / "tools/editor/ui-tests/tests"
    path.mkdir(parents=True)
    (path.parent / "pyproject.toml").write_text(
        '[tool.pytest.ini_options]\nmarkers=["sample: sample"]\n'
    )
    return tmp_path, path


def test_full_outcomes_and_tree(suite):
    repo, path = suite
    (path / "test_sample.py").write_text("""import pytest
class TestGroup:
 @pytest.mark.sample
 @pytest.mark.parametrize("value", [1, 2])
 def test_value(self, value): assert value
@pytest.mark.xfail
def test_xfail(): assert False
@pytest.mark.xfail
def test_xpass(): pass
@pytest.mark.xfail(strict=True)
def test_strict(): pass
@pytest.fixture
def broken():
 yield
 raise RuntimeError("teardown broken")
def test_teardown(broken): pass
""")
    events = finish(Process(repo, Path(sys.executable), Path("unused"), ["tests"]))
    results = [e for e in events if e["kind"] == "test-end"]
    assert [r["status"] for r in results] == [
        "Passed",
        "Passed",
        "Expected failure",
        "Unexpected pass",
        "Unexpected pass",
        "Error",
    ]
    assert results[4]["strict_xpass"]
    assert events[-1]["code"] == 1
    state = RunState()
    for event in events:
        state.apply(event)
    ids = matching(state.records, marker="sample")
    assert len(ids) == 2
    rows = tree_rows(state.records, ids, set())
    assert [r["title"] for r in rows if r["group"]] == [
        "tests",
        "test_sample.py",
        "TestGroup",
        "value".capitalize(),
    ]
    assert tree_rows(state.records, ids, {"dir:tests"})[0]["members"] == ids
    assert len(tree_rows(state.records, ids, {"dir:tests"})) == 1
    rerun, missing = rerun_selection(state.records, {results[5]["nodeid"]: {}})
    assert rerun == [results[5]["nodeid"]]
    assert missing == [results[4]["nodeid"]]


@pytest.mark.parametrize(
    "source,code,errors", [("", 5, False), ("def broken(:", 2, True)]
)
def test_empty_and_failed_collection(suite, source, code, errors):
    repo, path = suite
    (path / "test_empty.py").write_text(source)
    events = finish(
        Process(repo, Path(sys.executable), Path("unused"), ["tests"], collect=True)
    )
    assert events[-1]["code"] == code
    assert any(e["kind"] == "error" for e in events) == errors
    assert not any(e["kind"] == "test-start" for e in events)


def test_reducer_preserves_completed_and_unstarted():
    state = RunState(
        records={n: record({"title": n}) for n in ("done", "active", "queued")}
    )
    state.apply(
        {"kind": "test-end", "nodeid": "done", "status": "Passed", "duration": 1}
    )
    state.apply({"kind": "test-start", "nodeid": "active"})
    state.apply({"kind": "finished", "code": 130, "cancelled": True})
    assert [r["status"] for r in state.records.values()] == [
        "Passed",
        "Cancelled",
        "Not run",
    ]


def test_storage_recovery_and_damaged_run(tmp_path):
    store = Store(tmp_path)
    directory, metadata = store.create({"repo": "sample"}, ["a"])
    writer = Writer(directory / "events.jsonl")
    writer.emit("collected", id="a", title="A")
    writer.emit("test-start", nodeid="a")
    writer.emit("action-start", nodeid="a", action_id="drag", title="Drag")
    writer.emit("debug-state", paused=True, pause_id=1)
    with (directory / "events.jsonl").open("a") as stream:
        stream.write('{"partial":')
    store.recover()
    loaded, state = store.load(metadata["id"])
    assert loaded["state"] == "Interrupted"
    assert state.records["a"]["status"] == "Interrupted"
    assert state.records["a"]["steps"][0]["status"] == "Interrupted"
    assert not state.debug["paused"]
    assert state.diagnostics
    (store.runs / ("a" * 32)).mkdir()
    assert len(store.history()) == 1
    assert store.warnings


def test_history_recovers_after_damaged_middle_event(tmp_path):
    store = Store(tmp_path)
    directory, metadata = store.create({"repo": "sample"}, ["a"])
    writer = Writer(directory / "events.jsonl")
    writer.emit("collected", id="a", title="A")
    writer.emit("test-start", nodeid="a")
    writer.emit("test-end", nodeid="a", status="Passed", duration=1)
    writer.emit("finished", code=0)
    lines = (directory / "events.jsonl").read_text().splitlines()
    lines[1] = "{broken"
    (directory / "events.jsonl").write_text("\n".join(lines) + "\n")
    metadata["state"] = "finished"
    store.save(directory, metadata)

    _, state = store.load(metadata["id"])

    assert state.state == "finished"
    assert state.records["a"]["status"] == "Passed"
    assert state.diagnostics


def test_retention_protects_active_pinned_and_external(tmp_path):
    store = Store(tmp_path / "data")
    dirs = []
    for index in range(4):
        directory, metadata = store.create({"repo": "sample"}, [])
        metadata.update(
            state="preflight" if index == 3 else "finished",
            pinned=index == 2,
            created=index,
        )
        store.save(directory, metadata)
        (directory / "bytes").write_bytes(b"a" * 100)
        dirs.append(directory)
    warning = store.prune({"count": 1, "days": 1, "gib": 0.00000001})
    assert warning
    assert not dirs[0].exists() and not dirs[1].exists()
    assert dirs[2].exists() and dirs[3].exists()
    with pytest.raises(ValueError):
        store.delete(dirs[2].name)
    external = tmp_path / ("b" * 32)
    external.mkdir()
    link = store.runs / external.name
    link.symlink_to(external, target_is_directory=True)
    with pytest.raises(ValueError):
        store.delete(link.name)
    assert external.exists()


def test_discovery_does_not_consume_execution_retention(tmp_path):
    store = Store(tmp_path)
    now = time.time()
    execution, execution_metadata = store.create({"repo": "sample"}, ["a"])
    execution_metadata.update(state="finished", created=now)
    store.save(execution, execution_metadata)
    old_discovery, old_metadata = store.create(
        {"repo": "sample"}, ["tests"], collect=True
    )
    old_metadata.update(state="finished", created=now + 1)
    store.save(old_discovery, old_metadata)
    discovery, discovery_metadata = store.create(
        {"repo": "sample"}, ["tests"], collect=True
    )
    discovery_metadata.update(state="finished", created=now + 2)
    store.save(discovery, discovery_metadata)

    store.prune({"count": 1, "days": 30, "gib": 5})

    assert execution.exists()
    assert discovery.exists()
    assert not old_discovery.exists()


def test_legacy_settings_imported_once(tmp_path):
    legacy = tmp_path / "local.json"
    legacy.write_text('{"repo": "first"}')
    store = Store(tmp_path / "data")
    settings = store.settings(legacy)
    assert settings["legacy"]["repo"] == "first"
    settings.pop("legacy")
    store.save_settings(settings)
    legacy.write_text('{"repo": "second"}')
    assert "legacy" not in store.settings(legacy)


def test_capability_rejection_and_cache_invalidation(suite, monkeypatch, tmp_path):
    import preflight as module

    repo, path = suite
    studio_path = repo / "tools/test-studio/preflight.py"
    monkeypatch.setattr(module, "__file__", str(studio_path))
    library = repo / "tools/slint-test/slint_test/core.py"
    library.parent.mkdir(parents=True)
    library.write_text("# first library")
    binary = tmp_path / "editor"
    binary.write_text("first")
    binary.chmod(0o755)
    project = {
        "repo": str(repo),
        "python": sys.executable,
        "binary": str(binary),
        "paths": ["tests"],
        "backend": "headless-skia",
    }
    info = {"packages": {"pytest": "9"}, "python": "test"}
    calls = []
    capabilities = {"schema_version": 1, "system_testing": False, "backends": []}

    def run(args, **kwargs):
        calls.append(kwargs["name"])
        return json.dumps(capabilities)

    monkeypatch.setattr(module, "checked_command", run)
    with pytest.raises(RuntimeError, match="does not support"):
        preflight(project, tmp_path, threading.Event(), {}, dict(info))
    assert calls == ["capabilities"]
    capabilities.update(system_testing=True, backends=["headless-skia"])
    calls.clear()
    cache = {}
    first = preflight(project, tmp_path, threading.Event(), cache, dict(info))
    assert calls == ["capabilities", "application-probe"]
    assert not first["probe_cached"]
    assert preflight(project, tmp_path, threading.Event(), cache, dict(info))[
        "probe_cached"
    ]
    (path / "conftest.py").write_text("# changed harness")
    assert not preflight(project, tmp_path, threading.Event(), cache, dict(info))[
        "probe_cached"
    ]
    library.write_text("# changed library")
    changed = preflight(project, tmp_path, threading.Event(), cache, dict(info))
    assert not changed["probe_cached"]
    assert changed["testing_library_sha256"] != first["testing_library_sha256"]
    library.with_name("native.descriptor").write_bytes(b"changed wire schema")
    schema_changed = preflight(project, tmp_path, threading.Event(), cache, dict(info))
    assert not schema_changed["probe_cached"]
    assert schema_changed["testing_library_sha256"] != changed["testing_library_sha256"]
    binary.write_text("second")
    assert (
        preflight(project, tmp_path, threading.Event(), cache, dict(info))[
            "binary_sha256"
        ]
        != first["binary_sha256"]
    )


@pytest.mark.parametrize("cancelled", [False, True])
def test_probe_timeout_and_cancellation(tmp_path, cancelled):
    cancel = threading.Event()
    if cancelled:
        cancel.set()
    with pytest.raises(InterruptedError if cancelled else TimeoutError):
        checked_command(
            [sys.executable, "-c", "import time; time.sleep(60)"],
            cwd=tmp_path,
            env=os.environ.copy(),
            directory=tmp_path,
            name="probe",
            cancel=cancel,
            timeout=0.15,
        )


def gone(pid):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return
        time.sleep(0.03)
    pytest.fail(f"Process {pid} survived cleanup")


@pytest.mark.skipif(os.name == "nt", reason="POSIX process supervision")
@pytest.mark.parametrize("parent_exits", [False, True])
def test_supervisor_reaps_stubborn_descendant(tmp_path, parent_exits):
    script = tmp_path / "parent.py"
    pidfile = tmp_path / "pid"
    child = (
        "import os,signal,time; from pathlib import Path; "
        "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
        f"Path({str(pidfile)!r}).write_text(str(os.getpid())); time.sleep(60)"
    )
    script.write_text(
        f"import subprocess,sys,time\nfrom pathlib import Path\np=subprocess.Popen([sys.executable,'-c',{child!r}])\n"
        f"while not Path({str(pidfile)!r}).exists(): time.sleep(0.01)\n"
        + ("" if parent_exits else "time.sleep(60)\n")
    )
    with (tmp_path / "log").open("w") as log:
        command = Command(
            [sys.executable, str(script)],
            cwd=tmp_path,
            env=os.environ.copy(),
            output=log,
        )
        deadline = time.monotonic() + 5
        while not pidfile.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        assert pidfile.exists()
        if not parent_exits:
            command.stop()
        command.process.wait(timeout=6)
        command.close()
    gone(int(pidfile.read_text()))


@pytest.mark.skipif(os.name == "nt", reason="POSIX parent-liveness pipe")
def test_unexpected_studio_exit_cleans_children(tmp_path):
    module_dir = Path(__file__).resolve().parents[1]
    pidfile = tmp_path / "child"
    parent = tmp_path / "studio.py"
    child_code = f"import os,time; from pathlib import Path; Path({str(pidfile)!r}).write_text(str(os.getpid())); time.sleep(60)"
    parent.write_text(
        f"import sys,os,time\nsys.path.insert(0,{str(module_dir)!r})\nfrom runner import Command\nfrom pathlib import Path\nlog=open({str(tmp_path / 'log')!r},'w')\nc=Command([sys.executable,'-c',{child_code!r}],cwd=Path({str(tmp_path)!r}),env=os.environ.copy(),output=log)\ntime.sleep(60)\n"
    )
    process = subprocess.Popen([sys.executable, str(parent)])
    try:
        deadline = time.monotonic() + 5
        while not pidfile.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        assert pidfile.exists()
        process.kill()
        process.wait()
        gone(int(pidfile.read_text()))
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def test_harness_observer_is_optional_and_isolated(monkeypatch):
    path = Path(__file__).resolve().parents[2] / "editor/ui-tests/tests/ui_reporting.py"
    monkeypatch.setitem(
        sys.modules, "slint_testing", SimpleNamespace(Application=object)
    )
    spec = importlib.util.spec_from_file_location("studio_observer_test", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, spec.name, module)
    spec.loader.exec_module(module)
    with module.replay_stage("ordinary"):
        pass
    events = []
    token = module.install_observer(lambda kind, **data: events.append((kind, data)))
    try:
        with pytest.raises(AssertionError), module.replay_stage("failure"):
            raise AssertionError("original failure")
    finally:
        module.reset_observer(token)
    assert [e[0] for e in events] == ["stage-start", "stage-end"]
    assert events[-1][1]["failed"]

    def broken(*args, **kwargs):
        raise RuntimeError("observer broken")

    token = module.install_observer(broken)
    try:
        with module.replay_stage("still passes"):
            pass
    finally:
        module.reset_observer(token)


def test_data_directory_lock_prevents_live_run_recovery(tmp_path):
    first = Store(tmp_path)
    with first.acquire(), pytest.raises(RuntimeError, match="Another Studio"):
        Store(tmp_path).acquire()
    with Store(tmp_path).acquire():
        pass


def test_crash_requires_process_evidence(tmp_path):
    from bridge import StudioPlugin

    plugin = StudioPlugin(tmp_path / "events.jsonl", tmp_path)
    plugin.nodeid = "test"
    plugin.observe("application-closing", returncode=-9, failed=True)
    events = [json.loads(line) for line in plugin.events.read_text().splitlines()]
    assert events[0]["kind"] == "application-crash"
    assert plugin.outcome == "Crashed"
    plugin.outcome = "Passed"
    plugin.observe("application-closing", returncode=None, failed=True)
    assert plugin.outcome == "Passed"
    assert json.loads(plugin.events.read_text().splitlines()[-1])["status"] == "Failed"


def test_environment_failure_is_actionable(suite, monkeypatch, tmp_path):
    import preflight as module

    repo, _ = suite

    def unavailable(*args, **kwargs):
        raise RuntimeError("No module named slint_testing")

    monkeypatch.setattr(module, "checked_command", unavailable)
    project = {
        "repo": str(repo),
        "python": sys.executable,
        "binary": "unused",
        "paths": ["tests"],
        "backend": "headless-skia",
    }
    with pytest.raises(RuntimeError, match="uv sync --locked"):
        module.check_environment(project, tmp_path, threading.Event())


def test_protocol_error_does_not_hide_later_finished_event(tmp_path):
    path = tmp_path / "events.jsonl"
    writer = Writer(path)
    writer.emit("state", state="running")
    with path.open("a") as stream:
        stream.write('{"truncated":')
    Writer(path).emit("finished", code=3, cancelled=False)
    lines = path.read_text().splitlines()
    assert json.loads(lines[-1])["kind"] == "finished"


def test_malformed_settings_are_preserved_and_recoverable(tmp_path):
    store = Store(tmp_path)
    (tmp_path / "settings.json").write_text('{"version":1,"projects":null}')
    result = store.settings()
    assert result["projects"] == []
    assert list(tmp_path.glob("settings-invalid-*.json"))
    assert store.warnings


def test_preflight_failure_preserves_selected_source(tmp_path, monkeypatch):
    import service as module

    def missing(*args, **kwargs):
        raise RuntimeError("missing pytest")

    monkeypatch.setattr(module, "check_environment", missing)
    service = module.Service(tmp_path)
    try:
        service.submit(
            "run",
            project={"repo": "sample"},
            selectors=["a"],
            collect=False,
            retention={"count": 20, "days": 30, "gib": 5},
            items={"a": {"id": "a", "title": "A", "source": "assert True"}},
        )
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            kind, result = service.updates.get(timeout=5)
            if kind == "run" and result["final"]:
                assert result["snapshot"]["records"]["a"]["source"] == "assert True"
                assert result["snapshot"]["records"]["a"]["status"] == "Not run"
                assert result["snapshot"]["diagnostics"][0]["category"] == "preflight"
                break
        else:
            pytest.fail("Preflight did not finish")
    finally:
        service.close()


@pytest.mark.parametrize("relative", [False, True])
@pytest.mark.parametrize("mixed", [False, True])
def test_external_collection_ids_execute_and_rerun(suite, tmp_path, relative, mixed):
    repo, path = suite
    external = tmp_path / "external tests"
    external.mkdir()
    (external / "pytest.ini").write_text("[pytest]\n")
    source = external / "test_external.py"
    source.write_text("""import pytest
class TestExternal:
 @pytest.mark.parametrize("value", [True, False], ids=["pass::case", "fail[case]"])
 def test_value(self, value): assert value
""")
    internal = path / "test_internal.py"
    internal.write_text("def test_internal(): pass\n")
    external_selector = (
        os.path.relpath(source, path.parent) if relative else str(source)
    )
    selectors = [external_selector]
    if mixed:
        selectors.append("tests/test_internal.py")
    collected = finish(
        Process(repo, Path(sys.executable), Path("unused"), selectors, collect=True)
    )
    rows = [e for e in collected if e["kind"] == "collected"]
    ids = [row["id"] for row in rows]
    assert ids[:2] == [
        f"{source}::TestExternal::test_value[pass::case]",
        f"{source}::TestExternal::test_value[fail[case]]",
    ]
    assert rows[0]["path"] == f"{source}:3"
    assert all(g["title"] for g in rows[0]["groups"])
    assert {g["id"] for g in rows[0]["groups"]} >= {
        f"file:{source}",
        f"class:{source}::TestExternal",
    }
    if mixed:
        assert ids[2] == "tests/test_internal.py::test_internal"
    assert collected[-1]["code"] == 0
    events = finish(Process(repo, Path(sys.executable), Path("unused"), ids))
    results = [e for e in events if e["kind"] == "test-end"]
    assert [e["nodeid"] for e in results] == ids
    assert [e["status"] for e in results] == ["Passed", "Failed"] + (
        ["Passed"] if mixed else []
    )
    assert events[-1]["code"] == 1
    state = RunState()
    for event in events:
        state.apply(event)
    rerun, unavailable = rerun_selection(state.records, state.records)
    assert rerun == [ids[1]] and not unavailable
    repeated = finish(Process(repo, Path(sys.executable), Path("unused"), rerun))
    assert [
        (e["nodeid"], e["status"]) for e in repeated if e["kind"] == "test-end"
    ] == [(ids[1], "Failed")]
    source.unlink()
    refreshed = finish(
        Process(repo, Path(sys.executable), Path("unused"), ["tests"], collect=True)
    )
    available = {e["id"]: e for e in refreshed if e["kind"] == "collected"}
    assert rerun_selection(state.records, available) == ([], [ids[1]])
