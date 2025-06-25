# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

from briefcase.bootstraps.base import BaseGuiBootstrap


class SlintGuiBootstrap(BaseGuiBootstrap):
    display_name_annotation = "does not support Android/Web deployment"

    def app_source(self):
        return """\
import slint

class {{ cookiecutter.class_name }}(slint.loader.{{ cookiecutter.module_name }}.resources.app_window.AppWindow):
    @slint.callback
    def request_increase_value(self):
        self.counter = self.counter + 1


def main():
    main_window = {{ cookiecutter.class_name }}()
    main_window.show()
    main_window.run()
"""

    def app_start_source(self):
        return """\
from {{ cookiecutter.module_name }}.app import main

if __name__ == "__main__":
    main()
"""

    def pyproject_table_briefcase_app_extra_content(self):
        return """
requires = [
]
test_requires = [
{% if cookiecutter.test_framework == "pytest" %}
    "pytest",
{% endif %}
]
"""

    def pyproject_table_macOS(self):
        return """\
universal_build = false
requires = [
    "slint",
]
"""

    def pyproject_table_linux(self):
        return """\
requires = [
    "slint",
]
"""

    def pyproject_table_windows(self):
        return """\
requires = [
    "slint",
]
"""

    def pyproject_table_iOS(self):
        return """\
requires = [
    "slint",
]
"""

    def post_generate(self, base_path: Path) -> None:
        target_dir = base_path / self.context["source_dir"] / "resources"
        target_dir.mkdir(parents=True, exist_ok=True)
        with open(target_dir / "app-window.slint", "w") as slint_file:
            slint_file.write(r"""
import { Button, VerticalBox, AboutSlint } from "std-widgets.slint";

export component AppWindow inherits Window {
    in-out property<int> counter: 42;
    callback request-increase-value();
    VerticalBox {
        alignment: center;
        AboutSlint {}
        Text {
            text: "Counter: \{root.counter}";
        }
        Button {
            text: "Increase value";
            clicked => {
                root.request-increase-value();
            }
        }
     }
}
""")
