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
import exhale_multiproject_monkeypatch

# -- Project information -----------------------------------------------------

project = 'SixtyFPS C++'
copyright = '2020, info@sixtyfps.io'
author = 'info@sixtyfps.io'

# The full version, including alpha/beta/rc tags
release = '0.0.5'


# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = ["breathe", "recommonmark", "exhale", "sphinx_markdown_tables"]

breathe_projects = {
        "SixtyFPS": "./docs/xml",
        "SixtyFPS Interpreter": "./interpreter-docs/xml",
}
breathe_default_project = "SixtyFPS"

exhale_args = {
        "doxygenStripFromPath": "..",
        "createTreeView": True,
        "exhaleExecutesDoxygen": True,
        "rootFileName": "library_root.rst",
}

exhale_projects_args = {
    "SixtyFPS": {
        "rootFileTitle": "SixtyFPS CPP Reference",
        "containmentFolder": "./api",
        "exhaleDoxygenStdin": '''INPUT = ../../api/sixtyfps-cpp/include
EXCLUDE_SYMBOLS = sixtyfps::cbindgen_private* sixtyfps::private_api* sixtyfps::interpreter*
EXCLUDE = ../../api/sixtyfps-cpp/include/vtable.h ../../api/sixtyfps-cpp/include/sixtyfps_testing.h
ENABLE_PREPROCESSING = YES
PREDEFINED = DOXYGEN'''
    },
    "SixtyFPS Interpreter": {
        "rootFileTitle": "SixtyFPS Interpreter CPP Reference",
        "containmentFolder": "./interpreter-api",
        "exhaleDoxygenStdin": '''INPUT = ../../api/sixtyfps-cpp/include/sixtyfps_interpreter.h
EXCLUDE_SYMBOLS = sixtyfps::cbindgen_private* sixtyfps::private_api*
EXCLUDE = ../../api/sixtyfps-cpp/include/vtable.h ../../api/sixtyfps-cpp/include/sixtyfps_testing.h
ENABLE_PREPROCESSING = YES
PREDEFINED = DOXYGEN
'''
    },
}


# Add any paths that contain templates here, relative to this directory.
templates_path = ['_templates']

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store']


# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#
html_theme = 'sphinx_rtd_theme'

# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named "default.css" will overwrite the builtin "default.css".
html_static_path = ['_static']

def setup(app):
    app.add_css_file('theme_tweak.css')
