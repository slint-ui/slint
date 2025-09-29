# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import load_file, init_translations
from pathlib import Path
import gettext
import typing


def base_dir() -> Path:
    origin = __spec__.origin
    assert origin is not None
    base_dir = Path(origin).parent
    assert base_dir is not None
    return base_dir


class DummyTranslation:
    def gettext(self, message: str) -> str:
        if message == "Yes":
            return "Ja"
        return message

    def pgettext(self, context: str, message: str) -> str:
        return self.gettext(message)


def test_load_file() -> None:
    module = load_file(base_dir() / "test-load-file.slint")

    testcase = module.App()

    assert testcase.translated == "Yes"
    init_translations(typing.cast(gettext.GNUTranslations, DummyTranslation()))
    try:
        assert testcase.translated == "Ja"
    finally:
        init_translations(None)
        assert testcase.translated == "Yes"
