from pathlib import Path
import os

import slint_testing


REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_PROBE_BINARY = REPO_ROOT / "target" / "debug" / "inertia-scroll-probe"


def test_inertia_scroll_probe_exposes_scroll_scene():
    probe_binary = Path(os.environ.get("INERTIA_SCROLL_PROBE_BIN", DEFAULT_PROBE_BINARY))
    assert probe_binary.exists(), f"missing probe binary: {probe_binary}"

    env = {}
    if "SLINT_BACKEND" in os.environ:
        env["SLINT_BACKEND"] = os.environ["SLINT_BACKEND"]

    with slint_testing.Application([str(probe_binary)], env=env, launch_timeout=10) as aut:
        window = aut.first_window
        assert window is not None
        assert window.size[0] > 0
        assert window.size[1] > 0

        root = window.root_element
        assert root.is_valid

        flickables = window.find_elements_by_id("MainWindow::flick")
        assert len(flickables) == 1
        assert flickables[0].is_valid
        assert flickables[0].size[1] > 0
