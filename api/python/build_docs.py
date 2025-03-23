# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint
import pdoc
import os


doc = pdoc.doc.Module(slint)

    model_cls = doc.get("Model")
    for method in model_cls.inherited_members[("builtins", "PyModelBase")]:
        method.is_inherited = False
        if not method.name.startswith("_") and method.name != "init_self":
            model_cls.own_members.append(method)

    out = pdoc.render.html_module(module=doc, all_modules={"foo": doc})

    os.makedirs("docs", exist_ok=True)
    with open("docs/index.html", "w") as f:
        f.write(out)
