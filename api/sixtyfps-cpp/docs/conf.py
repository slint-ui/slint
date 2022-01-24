# Copyright © 2021 SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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

project = "SixtyFPS C++"
copyright = "2021, info@sixtyfps.io"
author = "info@sixtyfps.io"

# The full version, including alpha/beta/rc tags
version = "0.2.0"

cpp_index_common_prefix = ["sixtyfps::", "sixtyfps::interpreter::"]

# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = ["breathe", "myst_parser", "exhale", "sphinx_markdown_tables"]

breathe_projects = {"SixtyFPS": "./docs/xml"}
breathe_default_project = "SixtyFPS"

exhale_args = {
    "containmentFolder": "./api",
    "rootFileName": "library_root.rst",
    "rootFileTitle": "C++ API Reference",
    "afterTitleDescription": textwrap.dedent(
        """
            The following sections present the C++ API Reference. All types are
            within the :ref:`sixtyfps<namespace_sixtyfps>` namespace and are accessible by including
            the :code:`sixtyfps.h` header file.

            If you choose to load :code:`.60` files dynamically at run-time, then
            you can use the classes in :ref:`sixtyfps::interpreter<namespace_sixtyfps__interpreter>`, starting at
            :cpp:class:`sixtyfps::interpreter::ComponentCompiler`. You need to include
            the :code:`sixtyfps_interpreter.h` header file.
        """
    ),
    "doxygenStripFromPath": "..",
    "createTreeView": True,
    "exhaleExecutesDoxygen": True,
    "exhaleDoxygenStdin": """INPUT = ../../api/sixtyfps-cpp/include generated_include
EXCLUDE_SYMBOLS = sixtyfps::cbindgen_private* sixtyfps::private_api* vtable* SIXTYFPS_DECL_ITEM
EXCLUDE = ../../api/sixtyfps-cpp/include/vtable.h ../../api/sixtyfps-cpp/include/sixtyfps_testing.h
ENABLE_PREPROCESSING = YES
PREDEFINED += DOXYGEN
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
html_theme = "sphinx_rtd_theme"

html_theme_options = {"collapse_navigation": False}

# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named "default.css" will overwrite the builtin "default.css".
html_static_path = ["_static"]

html_show_sourcelink = False

html_logo = "logo.drawio.svg"

myst_enable_extensions = [
    "html_image",
]

# Annotate h1/h2 elements with anchors
myst_heading_anchors = 2

rst_epilog = """
.. |ListView| replace:: :code:`ListView`
.. _ListView: ../markdown/widgets.html#listview
.. |Repetition| replace:: :code:`for` - :code:`in`
.. _Repetition: ../markdown/langref.html#repetition
"""


def setup(app):
    app.add_css_file("theme_tweak.css")
