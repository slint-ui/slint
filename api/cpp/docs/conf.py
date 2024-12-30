# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

# Configuration file for the Sphinx documentation builder.
#
# This file only contains a selection of the most common options. For a full
# list see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Path setup --------------------------------------------------------------

# If extensions (or modules to document with autodoc) are in another directory,
# add these directories to sys.path here. If the directory is relative to the
# documentation root, use os.path.abspath to make it absolute, like shown here.
#
# import sys
# sys.path.insert(0, os.path.abspath('.'))
import textwrap
import os
import json


# -- Project information -----------------------------------------------------

# The full version, including alpha/beta/rc tags
version = "1.9.2"

project = f'Slint {version} C++ API'
copyright = "SixtyFPS GmbH"
author = "Slint Developers <info@slint.dev>"

cpp_index_common_prefix = ["slint::", "slint::interpreter::"]

# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = ["breathe", "myst_parser", "exhale",
              "sphinx_markdown_tables", "sphinxcontrib.jquery"]

breathe_projects = {"Slint": "./docs/xml"}
breathe_default_project = "Slint"

exhale_args = {
    "containmentFolder": "./api",
    "rootFileName": "library_root.rst",
    "rootFileTitle": "C++ API Reference",
    "afterTitleDescription": textwrap.dedent(
        """
            The following sections present the C++ API Reference. All types are
            within the :ref:`slint<namespace_slint>` namespace and are accessible by including
            the :code:`slint.h` header file.

            If you choose to load :code:`.slint` files dynamically at run-time, then
            you can use the classes in :ref:`slint::interpreter<namespace_slint__interpreter>`, starting at
            :cpp:class:`slint::interpreter::ComponentCompiler`. You need to include
            the :code:`slint-interpreter.h` header file.
        """
    ),
    "doxygenStripFromPath": "..",
    "createTreeView": True,
    "kindsWithContentsDirectives": [],
    "exhaleExecutesDoxygen": True,
    "exhaleDoxygenStdin": """INPUT = ../../api/cpp/include generated_include
EXCLUDE_SYMBOLS = slint::cbindgen_private* slint::private_api* vtable* SLINT_DECL_ITEM
EXCLUDE = ../../api/cpp/include/vtable.h ../../api/cpp/include/slint_tests_helper.h ../../api/cpp/include/slint-stm32.h
ENABLE_PREPROCESSING = YES
PREDEFINED += DOXYGEN
INCLUDE_PATH = generated_include
WARN_AS_ERROR = YES""",
}

# Add any paths that contain templates here, relative to this directory.
templates_path = ["_templates"]

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = [
    "_build",
    "html/_static/collapsible-lists/LICENSE.md",
    "Thumbs.db",
    ".DS_Store",
    "markdown/tutorial",
    "markdown/building.md",
    "markdown/development.md",
    "markdown/install_qt.md",
    "markdown/README.md",
    "README.md",
]


# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#
html_theme = "furo"

html_theme_options = {"collapse_navigation": False}

# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named "default.css" will overwrite the builtin "default.css".
html_static_path = ["_static"]

html_show_sourcelink = False

html_logo = "https://slint.dev/logo/slint-logo-small-light.svg"

myst_enable_extensions = [
    "html_image", "colon_fence", "substitution"
]

# Annotate h1/h2 elements with anchors
myst_heading_anchors = 2

myst_url_schemes = {
    "slint-reference": f"https://slint.dev/releases/{version}/docs/slint/{{{{path}}}}",
    'http': None, 'https': None, 'mailto': None,
}

rst_epilog = ""

myst_substitutions = {}

with open(os.path.join(os.path.dirname(__file__), "..", "..", "internal", "core-macros", "link-data.json")) as link_data:
    links = json.load(link_data)

for key in links.keys():
    href = links[key]["href"]
    url = f"https://slint.dev/releases/{version}/docs/slint/{href}"
    myst_substitutions[f"slint_href_{key}"] = url
    rst_epilog += f".. |{key}| replace:: :code:`{key}`\n"
    rst_epilog += f".. _{key}: {url}\n"

def setup(app):
    app.add_css_file("theme_tweak.css")
