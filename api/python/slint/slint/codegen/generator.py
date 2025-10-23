from __future__ import annotations

import shutil
from pathlib import Path
from typing import Iterable, TYPE_CHECKING

from slint import slint as native

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
from .utils import normalize_identifier

if TYPE_CHECKING:
    from slint.slint import CallbackInfo, FunctionInfo, PyDiagnostic


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

    if output_dir is not None:
        output_dir.mkdir(parents=True, exist_ok=True)

    compiler = native.Compiler()
    if config.style:
        compiler.style = config.style
    if config.include_paths:
        compiler.include_paths = config.include_paths.copy()  # type: ignore[assignment]
    if config.library_paths:
        compiler.library_paths = config.library_paths.copy()  # type: ignore[assignment]
    if config.translation_domain:
        compiler.translation_domain = config.translation_domain

    for source_path, root in files:
        compilation = _compile_slint(compiler, source_path, config)
        if compilation is None:
            continue

        artifacts = _collect_metadata(compilation)
        relative = source_path.relative_to(root)

        if output_dir is None:
            module_dir = source_path.parent
            target_stem = module_dir / source_path.stem
            copy_slint = False
            slint_destination = source_path
            resource_name = source_path.name
            source_descriptor = source_path.name
        else:
            module_dir = output_dir / relative.parent
            module_dir.mkdir(parents=True, exist_ok=True)
            target_stem = module_dir / relative.stem
            copy_slint = True
            slint_destination = target_stem.with_suffix(".slint")
            resource_name = relative.name
            source_descriptor = str(relative)

        write_python_module(
            target_stem.with_suffix(".py"),
            source_relative=source_descriptor,
            resource_name=resource_name,
            config=config,
            artifacts=artifacts,
        )
        write_stub_module(target_stem.with_suffix(".pyi"), artifacts=artifacts)

        if copy_slint and slint_destination != source_path:
            shutil.copy2(source_path, slint_destination)


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
    compiler: native.Compiler,
    source_path: Path,
    config: GenerationConfig,
) -> native.CompilationResult | None:
    result = compiler.build_from_path(source_path)

    diagnostics = result.diagnostics
    diagnostic_error = getattr(native, "DiagnosticLevel", None)
    error_enum = getattr(diagnostic_error, "Error", None)

    def is_error(diag: PyDiagnostic) -> bool:
        if error_enum is not None:
            return diag.level == error_enum
        return str(diag.level).lower().startswith("error")

    errors = [diag for diag in diagnostics if is_error(diag)]
    warnings = [diag for diag in diagnostics if not is_error(diag)]

    if warnings and not config.quiet:
        for diag in warnings:
            print(f"warning: {diag}")

    if errors:
        for diag in errors:
            print(f"error: {diag}")
        print(f"Skipping generation for {source_path}")
        return None

    return result


def _collect_metadata(result: native.CompilationResult) -> ModuleArtifacts:
    components: list[ComponentMeta] = []
    for name in result.component_names:
        comp = result.component(name)

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
                    py_name=normalize_identifier(key),
                    type_hint=type_hint,
                )
            )

        callbacks = [_callback_meta(cb, callback_info[cb]) for cb in comp.callbacks]
        functions = [_callback_meta(fn, function_info[fn]) for fn in comp.functions]

        globals_meta: list[GlobalMeta] = []
        for global_name in comp.globals:
            global_property_info = {
                info.name: info for info in comp.global_property_infos(global_name)
            }
            global_callback_info = {
                info.name: info for info in comp.global_callback_infos(global_name)
            }
            global_function_info = {
                info.name: info for info in comp.global_function_infos(global_name)
            }
            properties_meta: list[PropertyMeta] = []

            for key in comp.global_properties(global_name):
                py_key = normalize_identifier(key)
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
                for cb in comp.global_callbacks(global_name)
            ]

            functions_meta = [
                _callback_meta(fn, global_function_info[fn])
                for fn in comp.global_functions(global_name)
            ]

            globals_meta.append(
                GlobalMeta(
                    name=global_name,
                    py_name=normalize_identifier(global_name),
                    properties=properties_meta,
                    callbacks=callbacks_meta,
                    functions=functions_meta,
                )
            )

        components.append(
            ComponentMeta(
                name=name,
                py_name=normalize_identifier(name),
                properties=properties,
                callbacks=callbacks,
                functions=functions,
                globals=globals_meta,
            )
        )

    structs_meta: list[StructMeta] = []
    enums_meta: list[EnumMeta] = []
    structs, enums = result.structs_and_enums

    for struct_name, struct_prototype in structs.items():
        fields: list[StructFieldMeta] = []
        for field_name, value in struct_prototype:
            fields.append(
                StructFieldMeta(
                    name=field_name,
                    py_name=normalize_identifier(field_name),
                    type_hint=_python_value_hint(value),
                )
            )
        structs_meta.append(
            StructMeta(
                name=struct_name,
                py_name=normalize_identifier(struct_name),
                fields=fields,
            )
        )

    for enum_name, enum_cls in enums.items():
        values: list[EnumValueMeta] = []
        for member, enum_member in enum_cls.__members__.items():  # type: ignore
            values.append(
                EnumValueMeta(
                    name=member,
                    py_name=normalize_identifier(member),
                    value=enum_member.name,
                )
            )
        enums_meta.append(
            EnumMeta(
                name=enum_name,
                py_name=normalize_identifier(enum_name),
                values=values,
            )
        )

    named_exports = [(orig, alias) for orig, alias in result.named_exports]

    return ModuleArtifacts(
        components=components,
        structs=structs_meta,
        enums=enums_meta,
        named_exports=named_exports,
    )


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
    if isinstance(value, native.Image):
        return "slint.Image"
    if isinstance(value, native.Brush):
        return "slint.Brush"
    return "Any"


def _callback_meta(name: str, info: CallbackInfo | FunctionInfo) -> CallbackMeta:
    return CallbackMeta(
        name=name,
        py_name=normalize_identifier(name),
        arg_types=[param.python_type for param in info.parameters],
        return_type=info.return_type,
    )
