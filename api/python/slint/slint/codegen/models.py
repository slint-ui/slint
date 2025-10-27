# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List


@dataclass(slots=True)
class GenerationConfig:
    include_paths: List[Path]
    library_paths: Dict[str, Path]
    style: str | None
    translation_domain: str | None
    quiet: bool = False


@dataclass(slots=True)
class PropertyMeta:
    name: str
    py_name: str
    type_hint: str


@dataclass(slots=True)
class CallbackMeta:
    name: str
    py_name: str
    arg_types: List[str]
    return_type: str


@dataclass(slots=True)
class ComponentMeta:
    name: str
    py_name: str
    properties: List[PropertyMeta]
    callbacks: List[CallbackMeta]
    functions: List[CallbackMeta]
    globals: List["GlobalMeta"]


@dataclass(slots=True)
class GlobalMeta:
    name: str
    py_name: str
    properties: List[PropertyMeta]
    callbacks: List[CallbackMeta]
    functions: List[CallbackMeta]


@dataclass(slots=True)
class StructFieldMeta:
    name: str
    py_name: str
    type_hint: str


@dataclass(slots=True)
class StructMeta:
    name: str
    py_name: str
    fields: List[StructFieldMeta]


@dataclass(slots=True)
class EnumValueMeta:
    name: str
    py_name: str
    value: str


@dataclass(slots=True)
class EnumMeta:
    name: str
    py_name: str
    values: List[EnumValueMeta]


@dataclass(slots=True)
class ModuleArtifacts:
    components: List[ComponentMeta]
    structs: List[StructMeta]
    enums: List[EnumMeta]
    named_exports: List[tuple[str, str]]
    resource_paths: List[Path]


__all__ = [
    "GenerationConfig",
    "PropertyMeta",
    "CallbackMeta",
    "ComponentMeta",
    "GlobalMeta",
    "StructFieldMeta",
    "StructMeta",
    "EnumValueMeta",
    "EnumMeta",
    "ModuleArtifacts",
]
