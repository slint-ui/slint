# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint
import pdoc
import pathlib
import subprocess


doc = pdoc.doc.Module(slint)

model_cls = doc.get("Model")
for method in model_cls.inherited_members[("builtins", "PyModelBase")]:
    method.is_inherited = False
    if not method.name.startswith("_") and method.name != "init_self":
        model_cls.own_members.append(method)

all_modules: dict[str, pdoc.doc.Module] = {"slint": doc}

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
