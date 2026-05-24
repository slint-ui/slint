# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Extract the `slint` API via griffe and emit Astro/Starlight MDX.

Run from the docs/python/ directory (or via `pnpm gen`):

    uv run python gen_mdx.py

Outputs:
    src/content/docs/api/<class>.mdx   one page per public top-level class
    src/content/docs/api/functions.mdx module-level functions and attributes
    src/api-manifest.json              qualified-name -> URL, for <XRef> resolution
    src/version.json                   documented package name and version

griffe parses the slint sources statically; this needs Python 3.12+ because the
sources use PEP 695 generics (`class Model[T]`). The `slint` package re-exports
its user-facing classes from the native extension (`slint.slint`, typed via
`slint/slint.pyi`) and from `slint.models` / `slint.loop`, so the top-level
names are griffe aliases that we resolve to their concrete, typed definitions.
"""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

import griffe

PACKAGE = "slint"
ROOT = Path(__file__).parent
# repo_root/api/python/slint holds the `slint` package (and its `pyproject.toml`).
PACKAGE_ROOT = ROOT.parent.parent / "api" / "python" / "slint"
API_DIR = ROOT / "src" / "content" / "docs" / "api"
MANIFEST = ROOT / "src" / "api-manifest.json"
PYPROJECT = PACKAGE_ROOT / "pyproject.toml"
VERSION_FILE = ROOT / "src" / "version.json"

# imports injected at the top of every generated page; path is relative to
# src/content/docs/api/<page>.mdx
IMPORTS = (
    'import XRef from "../../../components/XRef.astro";\n'
    'import Signature from "../../../components/Signature.astro";'
)

# ---- docstring -> MDX -------------------------------------------------------

FENCE = re.compile(r"```.*?```", re.DOTALL)
INLINE = re.compile(r"`[^`\n]+`")
SENTINEL = "\x00{}\x00"


def docstring_to_mdx(text_value: str, manifest: dict[str, str]) -> str:
    """Convert a Markdown docstring to MDX: fenced code is preserved verbatim,
    inline-code spans that name a documented symbol (e.g. `ListModel` or
    `run_event_loop()`) become <XRef> links, other inline code is kept as-is,
    and JSX-significant characters in the remaining prose are escaped."""
    stash: list[str] = []

    def protect(s: str) -> str:
        stash.append(s)
        return SENTINEL.format(len(stash) - 1)

    def inline(match: re.Match[str]) -> str:
        # symbol refs in docstrings are written as plain inline code; link the
        # ones we have a page for, keep the rest as literal code.
        symbol = match.group(0)[1:-1].strip().removesuffix("()")
        if symbol in manifest:
            return protect(f'<XRef to="{symbol}" />')
        return protect(match.group(0))

    # 1. protect fenced code blocks verbatim
    text_value = FENCE.sub(lambda m: protect(m.group(0)), text_value)
    # 2. inline code -> <XRef> link or protected literal code
    text_value = INLINE.sub(inline, text_value)
    # 3. escape JSX-significant chars in the remaining prose
    text_value = (
        text_value.replace("<", "&lt;").replace("{", "&#123;").replace("}", "&#125;")
    )
    # 4. restore protected segments
    return re.sub(r"\x00(\d+)\x00", lambda m: stash[int(m.group(1))], text_value)


# ---- griffe walking ---------------------------------------------------------


def is_private_doc(obj: griffe.Object) -> bool:
    """pdoc convention carried over from the sources: a `@private` docstring
    hides the member from the generated docs."""
    return bool(obj.docstring and obj.docstring.value.strip().startswith("@private"))


def text(value: str) -> str:
    """Escape a string for use as MDX flow text (JSX-significant chars)."""
    return (
        value.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace("{", "&#123;")
        .replace("}", "&#125;")
    )


def render_annotation(value: str | griffe.Expr | None, manifest: dict[str, str]) -> str:
    """Render a griffe annotation as MDX, turning every name that has its own
    page into an <XRef>. The `typing.` qualifier is dropped for readability."""
    if value is None:
        return ""
    if isinstance(value, str):
        # Forward references in the stubs are quoted (e.g. `-> "Color"`); link
        # them when the referenced symbol has its own page.
        bare = value.strip().strip("\"'")
        if bare in manifest:
            return f'<XRef to="{bare}" plain />'
        return text(bare)

    parts: list[str] = []
    skip_dot = False
    for tok in value.iterate(flat=True):
        if isinstance(tok, str):
            if skip_dot and tok == ".":
                skip_dot = False
                continue
            skip_dot = False
            parts.append(text(tok))
            continue
        name = getattr(tok, "name", str(tok))
        canonical = getattr(tok, "canonical_path", name)
        if name == "typing":
            skip_dot = True  # also drop the following "."
            continue
        if canonical in manifest:
            parts.append(f'<XRef to="{canonical}" plain />')
        elif name in manifest:
            parts.append(f'<XRef to="{name}" plain />')
        else:
            parts.append(text(name))
    return "".join(parts)


def render_signature(func: griffe.Function, manifest: dict[str, str]) -> str:
    params = []
    for p in func.parameters:
        if p.name in ("self", "cls"):
            continue
        s = text(p.name)
        if p.annotation is not None:
            s += ": " + render_annotation(p.annotation, manifest)
        if p.default is not None:
            s += " = " + text(str(p.default))
        params.append(s)
    sig = f"{text(func.name)}({', '.join(params)})"
    if func.returns is not None:
        sig += " -&gt; " + render_annotation(func.returns, manifest)
    return sig


def slug(name: str) -> str:
    return name.lower()


def url_for(top: str, member: str | None = None) -> str:
    base = f"/api/{slug(top)}/"
    return f"{base}#{member.lower()}" if member else base


# ---- selection --------------------------------------------------------------


def resolve(obj: griffe.Object | griffe.Alias) -> griffe.Object | None:
    """Resolve an alias to its concrete target, or return the object itself.
    Returns None for aliases that point outside the package (e.g. stdlib
    re-imports such as `os`, `asyncio`) which griffe cannot resolve statically."""
    if not obj.is_alias:
        return obj  # type: ignore[return-value]
    try:
        target = obj.final_target
    except Exception:
        return None
    if not target.path.startswith(PACKAGE + "."):
        return None
    return target


def public_named_members(
    obj: griffe.Object,
) -> list[tuple[str, griffe.Object]]:
    """(binding name, resolved object) for public, non-`@private` members,
    in source order. Aliases are resolved; stdlib re-exports are dropped."""
    out: list[tuple[str, griffe.Object]] = []
    for name, member in obj.members.items():
        if name.startswith("_"):
            continue
        resolved = resolve(member)
        if resolved is None or is_private_doc(resolved):
            continue
        out.append((name, resolved))
    return out


def class_members(
    cls: griffe.Class,
) -> tuple[list[griffe.Attribute], list[griffe.Function], list[griffe.Object]]:
    """Properties, methods and (for enums) values of a class.

    Includes public members inherited from base classes that live in the
    package (e.g. `Model` inherits `row_count` from the native `PyModelBase`),
    mirroring the fix-up the old pdoc generator performed by hand. `init_self`
    and underscore-prefixed names are excluded."""
    seen: set[str] = set()
    members: list[griffe.Object] = []
    for _, m in public_named_members(cls):
        seen.add(m.name)
        members.append(m)
    for name, inherited in cls.inherited_members.items():
        if name.startswith("_") or name == "init_self" or name in seen:
            continue
        resolved = resolve(inherited)
        if resolved is None or is_private_doc(resolved):
            continue
        members.append(resolved)

    props = [m for m in members if isinstance(m, griffe.Attribute)]
    methods = [m for m in members if isinstance(m, griffe.Function)]
    return props, methods, members


def is_enum(cls: griffe.Class) -> bool:
    return any("Enum" in str(b) for b in cls.bases)


def base_is_native(base: str | griffe.Expr) -> bool:
    """True if a base class lives in the native extension (`slint.slint`). That
    is the pyo3 implementation base (e.g. `PyModelBase`), an internal detail
    users don't subclass directly, so it is left off the rendered bases line."""
    if isinstance(base, str):
        return False
    for tok in base.iterate(flat=True):
        if isinstance(tok, str):
            continue
        canonical = getattr(tok, "canonical_path", "") or ""
        if canonical == "slint.slint" or canonical.startswith("slint.slint."):
            return True
    return False


def render_bases(cls: griffe.Class, manifest: dict[str, str]) -> str:
    """A `**Bases:**` line listing the base classes, linking the documented
    ones via <XRef>. Subscript brackets are escaped so a generic like `[T]` is
    not parsed as a Markdown link reference in the surrounding prose."""
    rendered: list[str] = []
    for base in cls.bases:
        if base_is_native(base):
            continue
        part = render_annotation(base, manifest)
        if part:
            rendered.append(part.replace("[", "&#91;").replace("]", "&#93;"))
    if not rendered:
        return ""
    return "**Bases:** " + ", ".join(rendered)


# ---- emit -------------------------------------------------------------------


def display_name(name: str, cls: griffe.Class) -> str:
    """The class name with its PEP 695 type parameters, e.g. `Model[T]`, so the
    page title shows that a generic class is generic. The URL slug still uses
    the bare name."""
    params = [p.name for p in cls.type_parameters]
    return f"{name}[{', '.join(params)}]" if params else name


def render_class(name: str, cls: griffe.Class, manifest: dict[str, str]) -> str:
    lines = ["---", f'title: "{display_name(name, cls)}"', "---", IMPORTS, ""]

    if cls.docstring:
        lines += [docstring_to_mdx(cls.docstring.value, manifest), ""]

    props, methods, members = class_members(cls)

    if is_enum(cls):
        lines += ["## Values", ""]
        for m in members:
            if m.docstring:
                lines.append(
                    f"- **`{m.name}`** — {docstring_to_mdx(m.docstring.value, manifest)}"
                )
            else:
                lines.append(f"- **`{m.name}`**")
        lines.append("")
        return "\n".join(lines)

    bases = render_bases(cls, manifest)
    if bases:
        lines += [bases, ""]

    if props:
        lines += ["## Properties", ""]
        for m in props:
            children = text(m.name)
            if m.annotation is not None:
                children += ": " + render_annotation(m.annotation, manifest)
            lines += [
                f"### {m.name}",
                "",
                f'<Signature symbol="{m.path}">{children}</Signature>',
                "",
            ]
            if m.docstring:
                lines += [docstring_to_mdx(m.docstring.value, manifest), ""]

    if methods:
        lines += ["## Methods", ""]
        for m in methods:
            children = render_signature(m, manifest)
            lines += [
                f"### {m.name}",
                "",
                f'<Signature symbol="{m.path}">{children}</Signature>',
                "",
            ]
            if m.docstring:
                lines += [docstring_to_mdx(m.docstring.value, manifest), ""]

    return "\n".join(lines)


def render_functions(
    functions: list[tuple[str, griffe.Function]],
    attributes: list[tuple[str, griffe.Attribute]],
    manifest: dict[str, str],
) -> str:
    lines = ["---", "title: Functions", "---", IMPORTS, ""]
    if functions:
        for name, m in functions:
            lines += [
                f"## {name}",
                "",
                f'<Signature symbol="{m.path}">{render_signature(m, manifest)}</Signature>',
                "",
            ]
            if m.docstring:
                lines += [docstring_to_mdx(m.docstring.value, manifest), ""]
    if attributes:
        lines += ["## Module attributes", ""]
        for name, m in attributes:
            children = text(name)
            if m.annotation is not None:
                children += ": " + render_annotation(m.annotation, manifest)
            lines += [
                f"### {name}",
                "",
                f'<Signature symbol="{m.path}">{children}</Signature>',
                "",
            ]
            if m.docstring:
                lines += [docstring_to_mdx(m.docstring.value, manifest), ""]
    return "\n".join(lines)


def single_line(value: str) -> str:
    """Collapse whitespace so a docstring fits on a single list-item line."""
    return " ".join(value.split())


def public_classes(mod: griffe.Module) -> list[tuple[str, griffe.Class]]:
    """Classes *defined* in the module, in source order. Unlike the top-level
    selection, re-imported aliases (e.g. `from slint import DataTransfer` in
    `language.pyi`) are skipped — they belong to their own page, not here."""
    return [
        (name, member)
        for name, member in mod.members.items()
        if not name.startswith("_")
        and not member.is_alias
        and isinstance(member, griffe.Class)
        and not is_private_doc(member)
    ]


def render_module(name: str, mod: griffe.Module, manifest: dict[str, str]) -> str:
    """Render a submodule (e.g. `language`) as one page: each public class is a
    section listing its enum values or its struct fields. These types only exist
    as a build-generated stub (`language.pyi`), so the page is empty until the
    crate's build script has run."""
    lines = ["---", f"title: {name}", "---", IMPORTS, ""]
    if mod.docstring:
        lines += [docstring_to_mdx(mod.docstring.value, manifest), ""]
    for cls_name, cls in public_classes(mod):
        lines += [f"## {cls_name}", ""]
        if cls.docstring:
            lines += [docstring_to_mdx(cls.docstring.value, manifest), ""]
        props, _methods, members = class_members(cls)
        if is_enum(cls):
            lines += ["**Values:**", ""]
            for m in members:
                doc = (
                    f" — {single_line(docstring_to_mdx(m.docstring.value, manifest))}"
                    if m.docstring
                    else ""
                )
                lines.append(f"- **`{m.name}`**{doc}")
            lines.append("")
        elif props:
            lines += ["**Fields:**", ""]
            for m in props:
                annotation = ""
                if m.annotation is not None:
                    rendered = render_annotation(m.annotation, manifest)
                    annotation = ": " + rendered.replace("[", "&#91;").replace(
                        "]", "&#93;"
                    )
                doc = (
                    f" — {single_line(docstring_to_mdx(m.docstring.value, manifest))}"
                    if m.docstring
                    else ""
                )
                lines.append(f"- **`{m.name}`**{annotation}{doc}")
            lines.append("")
    return "\n".join(lines)


# ---- manifest ---------------------------------------------------------------


def add_symbol(manifest: dict[str, str], obj: griffe.Object, url: str) -> None:
    """Register every name a docstring or annotation might use for `obj`."""
    manifest[obj.path] = url  # resolved path, e.g. slint.slint.Color
    manifest[obj.name] = url  # bare name, e.g. Color


def submodule_member_url(mod_name: str, cls_name: str) -> str:
    """A submodule's classes share one page; link to the class's anchor."""
    return f"/api/{slug(mod_name)}/#{slug(cls_name)}"


def build_manifest(
    classes: list[tuple[str, griffe.Class]],
    functions: list[tuple[str, griffe.Function]],
    submodules: list[tuple[str, griffe.Module]],
) -> dict[str, str]:
    manifest: dict[str, str] = {}
    for name, cls in classes:
        url = url_for(name)
        manifest[name] = url
        manifest[f"{PACKAGE}.{name}"] = url
        add_symbol(manifest, cls, url)
        _, _, members = class_members(cls)
        for m in members:
            member_url = url_for(name, m.name)
            manifest[f"{name}.{m.name}"] = member_url
            manifest[f"{m.path}"] = member_url
    for name, func in functions:
        url = url_for("functions", name)
        manifest[name] = url
        manifest[f"{PACKAGE}.{name}"] = url
        manifest[func.path] = url
    for mod_name, submod in submodules:
        for cls_name, cls in public_classes(submod):
            url = submodule_member_url(mod_name, cls_name)
            manifest[cls_name] = url
            manifest[f"{mod_name}.{cls_name}"] = url
            manifest[cls.path] = url
    return manifest


# ---- main -------------------------------------------------------------------


def exported_names(mod: griffe.Module) -> set[str] | None:
    """The names listed in the module's `__all__`, or None if it defines none.
    Used to document only the package's public surface (e.g. `slint` exports
    `loader` but not the internal `SlintAutoLoader` / `SlintEventLoop` types)."""
    if not mod.exports:
        return None
    return {
        e if isinstance(e, str) else getattr(e, "name", str(e)) for e in mod.exports
    }


def main() -> None:
    mod = griffe.load(PACKAGE, search_paths=[str(PACKAGE_ROOT)], resolve_aliases=True)
    assert isinstance(mod, griffe.Module)
    exported = exported_names(mod)

    classes: list[tuple[str, griffe.Class]] = []
    functions: list[tuple[str, griffe.Function]] = []
    attributes: list[tuple[str, griffe.Attribute]] = []
    submodules: list[tuple[str, griffe.Module]] = []
    for name, obj in public_named_members(mod):
        if exported is not None and name not in exported:
            continue
        if isinstance(obj, griffe.Class):
            classes.append((name, obj))
        elif isinstance(obj, griffe.Function):
            functions.append((name, obj))
        elif isinstance(obj, griffe.Attribute):
            attributes.append((name, obj))
        # A documented submodule (e.g. `language`); skip it until its
        # build-generated stub is present and it actually has classes.
        elif isinstance(obj, griffe.Module) and public_classes(obj):
            submodules.append((name, obj))

    manifest = build_manifest(classes, functions, submodules)

    API_DIR.mkdir(parents=True, exist_ok=True)
    for stale in API_DIR.glob("*.mdx"):
        stale.unlink()

    for name, cls in classes:
        (API_DIR / f"{slug(name)}.mdx").write_text(render_class(name, cls, manifest))
    if functions or attributes:
        (API_DIR / "functions.mdx").write_text(
            render_functions(functions, attributes, manifest)
        )
    for name, submod in submodules:
        (API_DIR / f"{slug(name)}.mdx").write_text(
            render_module(name, submod, manifest)
        )

    MANIFEST.write_text(json.dumps(manifest, indent=2, sort_keys=True))
    pages = len(classes) + (1 if functions or attributes else 0) + len(submodules)
    print(f"Wrote {pages} pages to {API_DIR}")
    print(f"Wrote manifest ({len(manifest)} symbols) to {MANIFEST}")

    version = tomllib.loads(PYPROJECT.read_text())["project"]["version"]
    VERSION_FILE.write_text(json.dumps({"package": PACKAGE, "version": version}))
    print(f"Wrote version ({PACKAGE} {version}) to {VERSION_FILE}")


if __name__ == "__main__":
    main()
