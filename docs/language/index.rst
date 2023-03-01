.. Copyright © SixtyFPS GmbH <info@slint-ui.com>
.. SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

Welcome to the Slint Language Documentation
===========================================

Introduction
------------

The Slint design markup language describes extensible graphical user interfaces.

- Place and compose a tree of visual elements in a window using a textual representation.
- Configure the appearance of elements via properties. For example a `Text` element has a `text`
  property, while a `Rectangle` element has a `background` color.
- Assign binding expressions to properties to automatically compute values that depend on other properties.
- Group binding expressions together with named states and conditions.
- Declare animations on properties and states to make the user interface feel alive.
- Build your own re-usable components and share them in `.slint` module files.
- Define data structures and models and access them from programming languages.
- Build highly customized user interfaces with the :ref:`builtin elements <Builtin Elements>`
  and pre-built :ref:`widgets <Widgets>` provided.

Architecture
------------

An application is composed of the business logic written in C++ and the `.slint` user interface design markup, which
is compiled to native code.

.. toctree::
   :hidden:
   :maxdepth: 2
   :caption: Concepts

   src/concepts/intro.md
   src/concepts/file.md
   src/concepts/layouting.md
   src/concepts/container.md
   src/concepts/focus.md
   src/concepts/fonts.md
   src/concepts/purity.md

.. toctree::
   :hidden:
   :maxdepth: 2
   :caption: Reference

   src/reference/comments.md
   src/reference/identifiers.md
   src/reference/types.md
   src/reference/properties.md
   src/reference/expressions.md
   src/reference/functions.md
   src/reference/callbacks.md
   src/reference/statements.md
   src/reference/repetitions.md
   src/reference/conditions.md
   src/reference/animations.md
   src/reference/states.md
   src/reference/globals.md
   src/reference/modules.md
   src/reference/legacy_syntax.md

.. toctree::
   :hidden:
   :maxdepth: 4
   :caption: Builtins

   src/builtins/callbacks.md
   src/builtins/elements.md
   src/builtins/enums.md
   src/builtins/functions.md
   src/builtins/namespaces.md
   src/builtins/structs.md
   src/builtins/widgets.md

.. toctree::
   :hidden:
   :maxdepth: 2
   :caption: Recipes & Examples

   src/recipes/recipes.md
