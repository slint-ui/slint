# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Extract the `slint` API via griffe and emit Astro/Starlight MDX.

Run from the docs/python/ directory (or via `pnpm gen`):

    uv run python gen_mdx.py

Outputs (one page per symbol, grouped into a sidebar group per kind):
    src/content/docs/api/{classes,enumerations,functions,variables}/<name>.mdx
    src/content/docs/api/language/{classes,enumerations}/<name>.mdx
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
import shutil
import tomllib
import urllib.request
import zlib
from dataclasses import dataclass
from pathlib import Path

import griffe

PACKAGE = "slint"
ROOT = Path(__file__).parent
# repo_root/api/python/slint holds the `slint` package (and its `pyproject.toml`).
PACKAGE_ROOT = ROOT.parent.parent / "api" / "python" / "slint"
DOCS_ROOT = ROOT / "src" / "content" / "docs"
API_DIR = DOCS_ROOT / "api"
MANIFEST = ROOT / "src" / "api-manifest.json"
PYPROJECT = PACKAGE_ROOT / "pyproject.toml"
VERSION_FILE = ROOT / "src" / "version.json"

# Per-kind page directories (relative to DOCS_ROOT); each becomes a sidebar
# group. Submodules like `language` mirror the same structure one level down.
DIR_CLASSES = "api/classes"
DIR_ENUMS = "api/enumerations"
DIR_FUNCTIONS = "api/functions"
DIR_VARIABLES = "api/variables"

# CPython documentation cross-references, resolved from its Sphinx inventory
# (the same objects.inv that Sphinx cross-references consume). The doc version
# is the slint port's minimum Python (see python_docs_url) so links match the
# documented runtime floor.
# Type-like roles worth linking from a signature (skip py:module, py:method,
# std:* …). py:data covers typing aliases like Optional and Callable.
STDLIB_ROLES = {"py:class", "py:data", "py:exception", "py:function"}

# qualified name -> docs.python.org URL; populated once in main().
_STDLIB: dict[str, str] = {}


def python_doc_version(requires_python: str) -> str:
    """The `major.minor` floor of a `requires-python` spec (`">= 3.12"` -> `3.12`).
    Raises ValueError if no version is present."""
    match = re.search(r"(\d+)\.(\d+)", requires_python)
    if not match:
        raise ValueError(f"no Python version in requires-python: {requires_python!r}")
    return match.group(0)


def python_docs_url() -> str:
    """Base URL of the CPython docs for the slint port's minimum Python, e.g.
    `https://docs.python.org/3.12/`. The floor comes from the package's
    `requires-python`, so the linked docs and the objects.inv version track that
    single source of truth."""
    requires = tomllib.loads(PYPROJECT.read_text())["project"]["requires-python"]
    try:
        version = python_doc_version(requires)
    except ValueError as exc:
        raise SystemExit(f"error: {exc} in {PYPROJECT}") from exc
    return f"https://docs.python.org/{version}/"


def parse_inventory(raw: bytes, docs_url: str) -> dict[str, str]:
    """Parse a Sphinx v2 objects.inv into {qualified name: URL}, keeping only the
    type-like roles. Inventory v2 is four "#"-prefixed header lines, then a
    zlib-compressed body of "name domain:role priority uri display-name" records
    ($ in uri == name). A name appears under several roles (e.g. `list` is both
    py:class and a `comprehension`); only the py: type roles are linkable."""
    body = zlib.decompress(raw.split(b"\n", 4)[4]).decode()
    links: dict[str, str] = {}
    for line in body.splitlines():
        parts = line.split(" ", 4)
        if len(parts) < 4 or parts[1] not in STDLIB_ROLES:
            continue
        name, uri = parts[0], parts[3]
        links[name] = docs_url + uri.replace("$", name)
    return links


def load_stdlib_inventory(docs_url: str) -> dict[str, str]:
    """Fetch CPython's Sphinx inventory and parse it (see parse_inventory). A
    fetch failure aborts the build rather than silently dropping links."""
    inventory_url = docs_url + "objects.inv"
    try:
        raw = urllib.request.urlopen(inventory_url, timeout=30).read()  # noqa: S310
    except OSError as exc:
        raise SystemExit(f"error: could not fetch {inventory_url}: {exc}") from exc
    return parse_inventory(raw, docs_url)


def imports_for(dir_path: str) -> str:
    """The XRef/Signature import header for a page at
    `src/content/docs/<dir_path>/<page>.mdx`, with enough `../` to reach
    `src/components` regardless of how deeply the page is nested."""
    ups = "../" * (2 + len(dir_path.split("/")))
    return (
        f'import XRef from "{ups}components/XRef.astro";\n'
        f'import Signature from "{ups}components/Signature.astro";'
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
        if bare in _STDLIB:
            manifest.setdefault(bare, _STDLIB[bare])
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
        elif canonical in _STDLIB:
            # link the stdlib type to docs.python.org via the shared manifest
            manifest.setdefault(canonical, _STDLIB[canonical])
            parts.append(f'<XRef to="{canonical}" plain />')
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


def page_url(dir_path: str, name: str, member: str | None = None) -> str:
    base = f"/{dir_path}/{slug(name)}/"
    return f"{base}#{member.lower()}" if member else base


# ---- selection --------------------------------------------------------------


def reexport_target(attr: griffe.Attribute) -> griffe.Object | None:
    """The concrete package object a re-export attribute points to, or None.

    The package re-exports the native pyo3 classes with plain assignments such
    as `StyledText = native.StyledText` (see `slint/__init__.py`). griffe records
    those as attributes whose value is a name reference, not as import aliases, so
    they would otherwise be documented as bare variables instead of the classes
    they expose. Only a pure name reference (`ExprName`/`ExprAttribute`) that
    resolves to a class or function inside the package counts: a constructor call
    like `loader = SlintAutoLoader()` is a real instance and stays a variable."""
    value = attr.value
    if not isinstance(value, (griffe.ExprName, griffe.ExprAttribute)):
        return None
    canonical = getattr(value, "canonical_path", None)
    if not canonical:
        return None
    try:
        target: griffe.Object | griffe.Alias = attr.modules_collection[canonical]
    except Exception:
        return None
    seen: set[str] = set()
    while isinstance(target, griffe.Alias):
        if target.path in seen:
            return None
        seen.add(target.path)
        try:
            target = target.final_target
        except Exception:
            return None
    if not target.path.startswith(PACKAGE + "."):
        return None
    if isinstance(target, (griffe.Class, griffe.Function)):
        return target
    return None


def resolve(obj: griffe.Object | griffe.Alias) -> griffe.Object | None:
    """Resolve an alias to its concrete target, or return the object itself.
    Returns None for aliases that point outside the package (e.g. stdlib
    re-imports such as `os`, `asyncio`) which griffe cannot resolve statically."""
    if isinstance(obj, griffe.Attribute):
        return reexport_target(obj) or obj
    if not isinstance(obj, griffe.Alias):
        return obj
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


def base_is_internal(base: str | griffe.Expr) -> bool:
    """True if a base class is an implementation detail not worth listing: the
    native pyo3 base in `slint.slint` (e.g. `PyModelBase`), or `typing.NamedTuple`
    (the language structs are NamedTuples, but that is noise on every page)."""
    if isinstance(base, str):
        return False
    for tok in base.iterate(flat=True):
        if isinstance(tok, str):
            continue
        canonical = getattr(tok, "canonical_path", "") or ""
        if (
            canonical == "slint.slint"
            or canonical.startswith("slint.slint.")
            or canonical == "typing.NamedTuple"
        ):
            return True
    return False


def render_bases(cls: griffe.Class, manifest: dict[str, str]) -> str:
    """A `**Bases:**` line listing the base classes, linking the documented
    ones via <XRef>. Subscript brackets are escaped so a generic like `[T]` is
    not parsed as a Markdown link reference in the surrounding prose."""
    rendered: list[str] = []
    for base in cls.bases:
        if base_is_internal(base):
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


def render_class(
    name: str,
    cls: griffe.Class,
    manifest: dict[str, str],
    imports: str,
    import_line: str,
) -> str:
    lines = ["---", f'title: "{display_name(name, cls)}"', "---", imports, ""]
    lines += [import_line, ""]

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


def render_function(
    name: str,
    func: griffe.Function,
    manifest: dict[str, str],
    imports: str,
    import_line: str,
) -> str:
    lines = ["---", f'title: "{name}"', "---", imports, ""]
    lines += [import_line, ""]
    lines += [
        f'<Signature symbol="{func.path}">{render_signature(func, manifest)}</Signature>',
        "",
    ]
    if func.docstring:
        lines += [docstring_to_mdx(func.docstring.value, manifest), ""]
    return "\n".join(lines)


def render_variable(
    name: str,
    attr: griffe.Attribute,
    manifest: dict[str, str],
    imports: str,
    import_line: str,
) -> str:
    lines = ["---", f'title: "{name}"', "---", imports, ""]
    lines += [import_line, ""]
    children = text(name)
    if attr.annotation is not None:
        children += ": " + render_annotation(attr.annotation, manifest)
    lines += [f'<Signature symbol="{attr.path}">{children}</Signature>', ""]
    if attr.docstring:
        lines += [docstring_to_mdx(attr.docstring.value, manifest), ""]
    return "\n".join(lines)


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


# ---- page model -------------------------------------------------------------


@dataclass
class Page:
    """One generated page: its directory (a sidebar group), the symbol it
    documents (the renderer dispatches on its griffe type), and the dotted
    public name to register in the manifest."""

    dir_path: str  # relative to DOCS_ROOT, e.g. "api/classes"
    name: str  # binding name, also the URL slug source, e.g. "Model"
    obj: griffe.Object
    qualifier: str  # dotted public name, e.g. "slint.Model" or "language.KeyEvent"
    import_path: str  # importable module, e.g. "slint" or "slint.language"


def import_statement(import_path: str, name: str) -> str:
    """A copy-and-paste `from <module> import <name>` code fence showing how to
    import the symbol. The bare binding name is used (no PEP 695 type params)."""
    return f"```python\nfrom {import_path} import {name}\n```"


def class_dir(parent: str, cls: griffe.Class) -> str:
    """Enums go under enumerations/, everything else under classes/."""
    return f"{parent}/enumerations" if is_enum(cls) else f"{parent}/classes"


def collect_pages(mod: griffe.Module) -> list[Page]:
    """Group the package's public surface into per-kind pages. Classes, enums,
    functions and variables live directly under api/; a documented submodule
    (e.g. `language`) mirrors the same classes/ and enumerations/ split."""
    exported = exported_names(mod)
    pages: list[Page] = []
    for name, obj in public_named_members(mod):
        if exported is not None and name not in exported:
            continue
        qualifier = f"{PACKAGE}.{name}"
        if isinstance(obj, griffe.Class):
            pages.append(Page(class_dir("api", obj), name, obj, qualifier, PACKAGE))
        elif isinstance(obj, griffe.Function):
            pages.append(Page(DIR_FUNCTIONS, name, obj, qualifier, PACKAGE))
        elif isinstance(obj, griffe.Attribute):
            pages.append(Page(DIR_VARIABLES, name, obj, qualifier, PACKAGE))
        elif isinstance(obj, griffe.Module):
            parent = f"api/{slug(name)}"
            import_path = f"{PACKAGE}.{name}"
            for cls_name, cls in public_classes(obj):
                pages.append(
                    Page(
                        class_dir(parent, cls),
                        cls_name,
                        cls,
                        f"{name}.{cls_name}",
                        import_path,
                    )
                )
    return pages


# ---- manifest ---------------------------------------------------------------


def build_manifest(pages: list[Page]) -> dict[str, str]:
    manifest: dict[str, str] = {}
    for page in pages:
        url = page_url(page.dir_path, page.name)
        manifest[page.name] = url
        manifest[page.qualifier] = url
        manifest[page.obj.path] = url
        manifest[page.obj.name] = url
        if isinstance(page.obj, griffe.Class):
            _, _, members = class_members(page.obj)
            for m in members:
                member_url = page_url(page.dir_path, page.name, m.name)
                manifest[f"{page.name}.{m.name}"] = member_url
                manifest[m.path] = member_url
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


def render_page(page: Page, manifest: dict[str, str]) -> str:
    imports = imports_for(page.dir_path)
    import_line = import_statement(page.import_path, page.name)
    obj = page.obj
    if isinstance(obj, griffe.Function):
        return render_function(page.name, obj, manifest, imports, import_line)
    if isinstance(obj, griffe.Attribute):
        return render_variable(page.name, obj, manifest, imports, import_line)
    assert isinstance(obj, griffe.Class)
    return render_class(page.name, obj, manifest, imports, import_line)


def main() -> None:
    global _STDLIB
    _STDLIB = load_stdlib_inventory(python_docs_url())

    mod = griffe.load(PACKAGE, search_paths=[str(PACKAGE_ROOT)], resolve_aliases=True)
    assert isinstance(mod, griffe.Module)

    pages = collect_pages(mod)
    manifest = build_manifest(pages)

    if API_DIR.exists():
        shutil.rmtree(API_DIR)
    for page in pages:
        out_dir = DOCS_ROOT / page.dir_path
        out_dir.mkdir(parents=True, exist_ok=True)
        # render_page may add referenced stdlib symbols to `manifest`.
        (out_dir / f"{slug(page.name)}.mdx").write_text(render_page(page, manifest))

    MANIFEST.write_text(json.dumps(manifest, indent=2, sort_keys=True))
    print(f"Wrote {len(pages)} pages to {API_DIR}")
    print(f"Wrote manifest ({len(manifest)} symbols) to {MANIFEST}")

    version = tomllib.loads(PYPROJECT.read_text())["project"]["version"]
    VERSION_FILE.write_text(json.dumps({"package": PACKAGE, "version": version}))
    print(f"Wrote version ({PACKAGE} {version}) to {VERSION_FILE}")


if __name__ == "__main__":
    main()
