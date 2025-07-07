# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint import load_file, ListModel
from pathlib import Path


def base_dir() -> Path:
    origin = __spec__.origin
    assert origin is not None
    base_dir = Path(origin).parent
    assert base_dir is not None
    return base_dir


def test_enums() -> None:
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

    TestEnum = module.TestEnum

    assert TestEnum.Variant1.name == "Variant1"
    assert TestEnum.Variant1.value == "Variant1"
    assert TestEnum.Variant2.name == "Variant2"
    assert TestEnum.Variant2.value == "Variant2"
    with pytest.raises(
        AttributeError, match="type object 'TestEnum' has no attribute 'Variant3'"
    ):
        TestEnum.Variant3

    instance = module.App()
    assert instance.enum_property == TestEnum.Variant2
    assert instance.enum_property.__class__ is TestEnum
    instance.enum_property = TestEnum.Variant1
    assert instance.enum_property == TestEnum.Variant1
    assert instance.enum_property.__class__ is TestEnum

    model_with_enums = instance.model_with_enums
    assert len(model_with_enums) == 2
    assert model_with_enums[0] == TestEnum.Variant2
    assert model_with_enums[1] == TestEnum.Variant1
    assert model_with_enums[0].__class__ is TestEnum
    model_with_enums = None

    instance.model_with_enums = ListModel([TestEnum.Variant1, TestEnum.Variant2])
    assert len(instance.model_with_enums) == 2
    assert instance.model_with_enums[0] == TestEnum.Variant1
    assert instance.model_with_enums[1] == TestEnum.Variant2
    assert instance.model_with_enums[0].__class__ is TestEnum
