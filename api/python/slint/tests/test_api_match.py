# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint import _load_file_checked
from pathlib import Path
import base64
import gzip


def base_dir() -> Path:
    origin = __spec__.origin
    assert origin is not None
    base_dir = Path(origin).parent
    assert base_dir is not None
    return base_dir


def compress_and_encode(json: str) -> str:
    return base64.standard_b64encode(gzip.compress(json.encode("utf-8"))).decode(
        "utf-8"
    )


def test_no_change() -> None:
    _load_file_checked(
        base_dir() / "api-match.slint",
        expected_api_base64_compressed=compress_and_encode(r"""
        {
            "version":"1.0",
            "globals":[],
            "components":[
                {
                    "name":"Test",
                    "properties":[
                        {
                            "name": "name",
                            "ty": "str"
                        }
                    ],
                    "aliases":[]
                }
            ],
            "structs_and_enums":[]
    }"""),
        generated_file="/some/path.py",
    )


def test_incompatible_changes() -> None:
    with pytest.raises(RuntimeError) as excinfo:
        _load_file_checked(
            base_dir() / "api-match.slint",
            expected_api_base64_compressed=compress_and_encode(r"""
            {
                "version":"1.0",
                "globals":[],
                "components":[
                    {
                        "name":"Test",
                        "properties":[
                            {
                                "name": "name",
                                "ty": "str"
                            },
                            {
                                "name": "not_there_anymore",
                                "ty": "str"
                            }
                        ],
                        "aliases":[]
                    }
                ],
                "structs_and_enums":[]
        }"""),
            generated_file="/some/path.py",
        )
    assert (
        f"Incompatible API changes detected between /some/path.py and {base_dir() / 'api-match.slint'}"
        == str(excinfo.value)
    )
