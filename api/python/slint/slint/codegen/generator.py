# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import enum
import shutil
from collections import defaultdict
from pathlib import Path
from typing import TYPE_CHECKING, Iterable

from ..api import _normalize_prop
from ..core import Brush, Color, CompilationResult, Compiler, DiagnosticLevel, Image
from .emitters import write_python_module, write_stub_module
from .models import (
    CallbackMeta,
    ComponentMeta,
    EnumMeta,
    EnumValueMeta,
    GenerationConfig,
    GlobalMeta,
    ModuleArtifacts,
    PropertyMeta,
    StructFieldMeta,
    StructMeta,
)

if TYPE_CHECKING:
    from slint.core import CallbackInfo, FunctionInfo, PyDiagnostic


def generate_project(
    *,
    inputs: Iterable[Path],
    output_dir: Path | None,
    config: GenerationConfig,
) -> None:
    source_roots = [Path(p).resolve() for p in inputs]
    files = list(_discover_slint_files(source_roots))
    if not files:
        raise SystemExit("No .slint files found in the supplied inputs")

    copied_slint: set[Path] = set()
    generated_modules = 0
    struct_only_modules = 0
    failed_files: list[Path] = []

    def copy_slint_file(source: Path, destination: Path) -> None:
        destination.parent.mkdir(parents=True, exist_ok=True)
        resolved = destination.resolve()
        if resolved in copied_slint:
            return
        shutil.copy2(source, destination)
        copied_slint.add(resolved)

    if output_dir is not None:
        output_dir.mkdir(parents=True, exist_ok=True)

    compiler = Compiler()

    if config.style:
        compiler.style = config.style
    if config.include_paths:
        compiler.include_paths = config.include_paths.copy()  # type: ignore[assignment]
    if config.library_paths:
        compiler.library_paths = config.library_paths.copy()  # type: ignore[assignment]
    if config.translation_domain:
        compiler.set_translation_domain(config.translation_domain)

    for source_path, root in files:
        source_resolved = source_path.resolve()
        relative = source_path.relative_to(root)
        compilation = _compile_slint(compiler, root, source_path, config)
        if compilation is None:
            failed_files.append(relative)
            continue

        source_descriptor = str(relative)
        artifacts = _collect_metadata(compilation, source_descriptor)

        sanitized_stem = _normalize_prop(source_path.stem)

        if output_dir is None:
            base_dir = source_path.parent
            slint_destination = source_path
            resource_name = source_path.name
            source_descriptor = source_path.name
            copy_slint = False
        else:
            base_dir = output_dir / relative.parent
            base_dir.mkdir(parents=True, exist_ok=True)
            _ensure_package_marker(base_dir)
            slint_destination = base_dir / relative.name
            resource_name = relative.name
            source_descriptor = str(relative)
            copy_slint = True

        target_stem = base_dir / sanitized_stem
        write_python_module(
            target_stem.with_suffix(".py"),
            source_relative=source_descriptor,
            resource_name=resource_name,
            config=config,
            artifacts=artifacts,
        )
        write_stub_module(target_stem.with_suffix(".pyi"), artifacts=artifacts)

        if copy_slint and slint_destination != source_path:
            copy_slint_file(source_path, slint_destination)

        if output_dir is not None:
            for dependency in artifacts.resource_paths:
                dep_path = Path(dependency)
                if not dep_path.exists():
                    continue
                dep_resolved = dep_path.resolve()
                if dep_resolved == source_resolved:
                    continue
                relative_dep: Path | None = None
                for source_root in source_roots:
                    try:
                        relative_dep = dep_resolved.relative_to(source_root)
                        break
                    except ValueError:
                        continue
                if relative_dep is None:
                    continue
                destination = output_dir / relative_dep
                copy_slint_file(dep_resolved, destination)

        generated_modules += 1
        if not artifacts.components:
            struct_only_modules += 1

    summary_lines: list[str] = []
    struct_note = f" ({struct_only_modules} struct-only)" if struct_only_modules else ""
    summary_lines.append(
        f"info: Generated {generated_modules} Python module(s){struct_note}"
    )

    if output_dir is not None:
        summary_lines.append(
            f"info: Copied {len(copied_slint)} .slint file(s) into {output_dir}"
        )

    if failed_files:
        sample = ", ".join(str(path) for path in failed_files[:3])
        if len(failed_files) > 3:
            sample += ", ..."
        summary_lines.append(
            f"info: Skipped {len(failed_files)} file(s) due to errors ({sample})"
        )

    for line in summary_lines:
        print(line)


def _discover_slint_files(inputs: Iterable[Path]) -> Iterable[tuple[Path, Path]]:
    for path in inputs:
        if path.is_file() and path.suffix == ".slint":
            resolved = path.resolve()
            yield resolved, resolved.parent
        elif path.is_dir():
            root = path.resolve()
            for file in sorted(root.rglob("*.slint")):
                resolved = file.resolve()
                yield resolved, root


def _compile_slint(
    compiler: Compiler,
    root: Path,
    source_path: Path,
    config: GenerationConfig,
) -> CompilationResult | None:
    result = compiler.build_from_path(source_path)

    def is_error(diag: PyDiagnostic) -> bool:
        return diag.level == DiagnosticLevel.Error

    errors: list[PyDiagnostic] = []
    warnings: list[PyDiagnostic] = []

    for diag in result.diagnostics:
        if is_error(diag):
            errors.append(diag)
        else:
            warnings.append(diag)

    non_fatal_errors: list[PyDiagnostic] = []
    fatal_errors: list[PyDiagnostic] = []

    for err in errors:
        # Files that only export structs/globals yield this diagnostic. We can still collect
        # metadata for them, so treat it as a warning for generation purposes.
        if err.message == "No component found":
            non_fatal_errors.append(err)
        else:
            fatal_errors.append(err)

    warnings.extend(non_fatal_errors)

    source_relative = str(source_path.relative_to(root))

    if warnings and not config.quiet:
        print(f"info: Compilation of {source_relative} completed with warnings:")
        for warn in warnings:
            print(f"   warning: {warn}")

    if fatal_errors:
        print(f"error: Compilation of {source_relative} failed & skiped with errors:")
        for fatal in fatal_errors:
            print(f"   error: {fatal}")
        return

    return result


def _collect_metadata(result: CompilationResult, source_descriptor: str) -> ModuleArtifacts:
    components: list[ComponentMeta] = []
    used_enum_class_names: set[str] = set()
    component_enum_props: dict[str, dict[str, str]] = defaultdict(dict)
    global_enum_props: dict[tuple[str, str], dict[str, str]] = defaultdict(dict)
    struct_enum_fields: dict[str, dict[str, str]] = defaultdict(dict)
    used_enum_class_names: set[str] = set()

    for name in result.component_names:
        comp = result.component(name)

        if comp is None:
            continue

        property_info = {info.name: info for info in comp.property_infos()}
        callback_info = {info.name: info for info in comp.callback_infos()}
        function_info = {info.name: info for info in comp.function_infos()}

        properties: list[PropertyMeta] = []
        for key in comp.properties:
            info = property_info[key]
            type_hint = info.python_type
            properties.append(
                PropertyMeta(
                    name=key,
                    py_name=_normalize_prop(key),
                    type_hint=type_hint,
                )
            )

        callbacks = [_callback_meta(cb, callback_info[cb]) for cb in comp.callbacks]
        functions = [_callback_meta(fn, function_info[fn]) for fn in comp.functions]

        globals_meta: list[GlobalMeta] = []
        for global_name in comp.globals:
            global_property_info = {
                info.name: info
                for info in comp.global_property_infos(global_name) or []
            }
            global_callback_info = {
                info.name: info
                for info in comp.global_callback_infos(global_name) or []
            }
            global_function_info = {
                info.name: info
                for info in comp.global_function_infos(global_name) or []
            }
            properties_meta: list[PropertyMeta] = []

            for key in comp.global_properties(global_name) or []:
                py_key = _normalize_prop(key)
                info = global_property_info[key]
                type_hint = info.python_type
                properties_meta.append(
                    PropertyMeta(
                        name=key,
                        py_name=py_key,
                        type_hint=type_hint,
                    )
                )

            callbacks_meta = [
                _callback_meta(cb, global_callback_info[cb])
                for cb in comp.global_callbacks(global_name) or []
            ]

            functions_meta = [
                _callback_meta(fn, global_function_info[fn])
                for fn in comp.global_functions(global_name) or []
            ]

            globals_meta.append(
                GlobalMeta(
                    name=global_name,
                    py_name=_normalize_prop(global_name),
                    properties=properties_meta,
                    callbacks=callbacks_meta,
                    functions=functions_meta,
                )
            )

        components.append(
            ComponentMeta(
                name=name,
                py_name=_normalize_prop(name),
                properties=properties,
                callbacks=callbacks,
                functions=functions,
                globals=globals_meta,
            )
        )

        try:
            instance = comp.create()
        except Exception:
            instance = None

        if instance is not None:
            for prop_name in comp.properties.keys():
                try:
                    value = instance.get_property(prop_name)
                except Exception:
                    continue
                if isinstance(value, enum.Enum):
                    used_enum_class_names.add(value.__class__.__name__)
                    component_enum_props[name][prop_name] = value.__class__.__name__

            for global_meta in globals_meta:
                for prop in global_meta.properties:
                    try:
                        value = instance.get_global_property(global_meta.name, prop.name)
                    except Exception:
                        continue
                    if isinstance(value, enum.Enum):
                        used_enum_class_names.add(value.__class__.__name__)
                        global_enum_props[(name, global_meta.name)][prop.name] = value.__class__.__name__

    structs_meta: list[StructMeta] = []
    enums_meta: list[EnumMeta] = []
    structs, enums = result.structs_and_enums

    for struct_name, struct_prototype in structs.items():
        fields: list[StructFieldMeta] = []
        for field_name, value in struct_prototype:
            if isinstance(value, enum.Enum):
                used_enum_class_names.add(value.__class__.__name__)
                struct_enum_fields[struct_name][field_name] = value.__class__.__name__
            fields.append(
                StructFieldMeta(
                    name=field_name,
                    py_name=_normalize_prop(field_name),
                    type_hint=_python_value_hint(value),
                )
            )
        structs_meta.append(
            StructMeta(
                name=struct_name,
                py_name=_normalize_prop(struct_name),
                fields=fields,
                is_builtin=False,
            )
        )

    for enum_name, enum_cls in enums.items():
        values: list[EnumValueMeta] = []
        for member, enum_member in enum_cls.__members__.items():  # type: ignore
            values.append(
                EnumValueMeta(
                    name=member,
                    py_name=_normalize_prop(member),
                    value=enum_member.name,
            )
        )

        is_used = enum_name in used_enum_class_names

        enums_meta.append(
            EnumMeta(
                name=enum_name,
                py_name=_normalize_prop(enum_name),
                values=values,
                is_builtin=False,
                is_used=is_used,
            )
        )

    enum_py_name_map = {enum.name: enum.py_name for enum in enums_meta if not enum.is_builtin}

    for component_meta in components:
        overrides = component_enum_props.get(component_meta.name, {})
        for prop in component_meta.properties:
            class_name = overrides.get(prop.name)
            if class_name:
                if class_name in enum_py_name_map:
                    prop.type_hint = enum_py_name_map[class_name]
                else:
                    print(
                        f"warning: {source_descriptor}: property '{prop.name}' of component '{component_meta.name}' "
                        f"relies on enum '{class_name}' that is not declared in this module; falling back to Any",
                    )
                    prop.type_hint = "Any"

        for global_meta in component_meta.globals:
            overrides = global_enum_props.get((component_meta.name, global_meta.name), {})
            for prop in global_meta.properties:
                class_name = overrides.get(prop.name)
                if class_name:
                    if class_name in enum_py_name_map:
                        prop.type_hint = enum_py_name_map[class_name]
                    else:
                        print(
                        f"warning: {source_descriptor}: global '{global_meta.name}.{prop.name}' relies on enum '{class_name}' "
                        "that is not declared in this module; falling back to Any",
                    )
                        prop.type_hint = "Any"

    for struct_meta in structs_meta:
        overrides = struct_enum_fields.get(struct_meta.name, {})
        for field in struct_meta.fields:
            class_name = overrides.get(field.name)
            if class_name:
                if class_name in enum_py_name_map:
                    field.type_hint = enum_py_name_map[class_name]
                else:
                    print(
                        f"warning: {source_descriptor}: struct '{struct_meta.name}.{field.name}' relies on enum '{class_name}' "
                        "that is not declared in this module; falling back to Any",
                    )
                    field.type_hint = "Any"

    named_exports = [(orig, alias) for orig, alias in result.named_exports]
    resource_paths = [Path(path) for path in result.resource_paths]

    return ModuleArtifacts(
        components=components,
        structs=structs_meta,
        enums=enums_meta,
        named_exports=named_exports,
        resource_paths=resource_paths,
    )


def _ensure_package_marker(module_dir: Path) -> None:
    """Ensure the generated directory is recognised as a regular Python package."""

    try:
        module_dir.mkdir(parents=True, exist_ok=True)
        init_file = module_dir / "__init__.py"
        if not init_file.exists():
            init_file.touch()
    except PermissionError:
        # If we cannot create the file, leave the namespace untouched. The
        # generated module can still be imported in environments that support
        # namespace packages.
        pass


__all__ = ["generate_project"]


def _python_value_hint(value: object) -> str:
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, int):
        return "int"
    if isinstance(value, float):
        return "float"
    if isinstance(value, str):
        return "str"
    if isinstance(value, Image):
        return "slint.Image"
    if isinstance(value, Brush):
        return "slint.Brush"
    if isinstance(value, Color):
        return "slint.Color"
    return "Any"


def _callback_meta(name: str, info: CallbackInfo | FunctionInfo) -> CallbackMeta:
    return CallbackMeta(
        name=name,
        py_name=_normalize_prop(name),
        arg_types=[param.python_type for param in info.parameters],
        return_type=info.return_type,
    )
