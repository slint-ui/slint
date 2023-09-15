# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

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
# import os
# import sys
# sys.path.insert(0, os.path.abspath('.'))
import textwrap


# -- Project information -----------------------------------------------------

# The full version, including alpha/beta/rc tags
version = "1.2.1"

project = f'Slint {version} C++ API'
copyright = "SixtyFPS GmbH"
author = "Slint Developers <info@slint.dev>"

cpp_index_common_prefix = ["slint::", "slint::interpreter::"]

# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = ["breathe", "myst_parser", "exhale", "sphinx_markdown_tables", "sphinxcontrib.jquery"]

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
EXCLUDE_SYMBOLS = slint::cbindgen_private* slint::private_api* vtable* slint::testing* SLINT_DECL_ITEM
EXCLUDE = ../../api/cpp/include/vtable.h ../../api/cpp/include/slint_testing.h
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
    "html_image",
]

# Annotate h1/h2 elements with anchors
myst_heading_anchors = 2

myst_url_schemes = {
    "slint-reference": f"https://slint.dev/releases/{version}/docs/slint/{{{{path}}}}",
    'http': None, 'https': None, 'mailto': None,
}

rst_epilog = """
.. |ListView| replace:: :code:`ListView`
.. _ListView: ../../slint/src/builtins/widgets.html#listview
.. |Repetition| replace:: :code:`for` - :code:`in`
.. _Repetition: ../../slint/src/reference/repetitions.html
"""


def setup(app):
    app.add_css_file("theme_tweak.css")
