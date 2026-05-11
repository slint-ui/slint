# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint
import slint.slint as native
import pdoc
import pathlib
import subprocess
import typing


doc = pdoc.doc.Module(slint)
native_doc = pdoc.doc.Module(native)

model_cls = typing.cast(pdoc.doc.Class, doc.get("Model"))
assert model_cls is not None
for method in model_cls.inherited_members[("builtins", "PyModelBase")]:
    method.is_inherited = False
    if not method.name.startswith("_") and method.name != "init_self":
        model_cls.own_members.append(method)


# pdoc reads `slint.slint.pyi` for the native C extension, which gives full
# signatures and property types. But the user-facing classes are re-exported
# from `slint` (e.g. `slint.DataTransfer`), where pdoc loses that information
# because there's no top-level `.pyi` stub. Copy the typed members back into
# the `slint`-level classes so the rendered docs show parameter and return
# types, property types, etc.
for cls_name in (
    "DataTransfer",
    "Image",
    "Color",
    "Brush",
    "Keys",
    "Timer",
    "TimerMode",
):
    top_cls = doc.get(cls_name)
    native_cls = native_doc.get(cls_name)
    if not isinstance(top_cls, pdoc.doc.Class) or not isinstance(
        native_cls, pdoc.doc.Class
    ):
        continue
    typed = {m.name: m for m in native_cls.own_members}
    new_members: list[pdoc.doc.Doc] = []
    for member in top_cls.own_members:
        replacement = typed.get(member.name)
        if replacement is not None:
            replacement.is_inherited = False
            new_members.append(replacement)
        else:
            new_members.append(member)
    top_cls.own_members = new_members

all_modules: dict[str, pdoc.doc.Module] = {}


def add_modules(m: pdoc.doc.Module):
    all_modules[m.fullname] = m
    for submod in m.submodules:
        add_modules(submod)


add_modules(doc)

output_directory = pathlib.Path("docs")

for module in all_modules.values():
    out = pdoc.render.html_module(module, all_modules)
    outfile = output_directory / f"{module.fullname.replace('.', '/')}.html"
    outfile.parent.mkdir(parents=True, exist_ok=True)
    outfile.write_bytes(out.encode())

index = pdoc.render.html_index(all_modules)
(output_directory / "index.html").write_bytes(index.encode())

search = pdoc.render.search_index(all_modules)
(output_directory / "search.js").write_bytes(search.encode())

subprocess.call(
    "cargo about generate thirdparty.hbs -o docs/thirdparty.html", shell=True
)
