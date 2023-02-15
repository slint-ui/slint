.. Copyright © SixtyFPS GmbH <info@slint-ui.com>
.. SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

Welcome to the Slint Language Reference
========================================

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
- Build highly customized user interfaces with the [builtin elements](builtin_elements.md)
  and pre-built [widgets](widgets.md) provided.

Architecture
------------

An application is composed of the business logic written in C++ and the `.slint` user interface design markup, which
is compiled to native code.

.. toctree::
   :maxdepth: 2
   :caption: Slint Language Concepts

   src/concepts/intro.md
   src/concepts/purity.md
   src/concepts/layouting.md
   src/concepts/focus.md
   src/concepts/fonts.md

.. toctree::
   :maxdepth: 2
   :caption: Slint Language Reference

   src/comments.md
   src/identifiers.md
   src/types.md
   src/properties.md
   src/expressions.md
   src/functions.md
   src/callbacks.md
   src/statements.md
   src/repetitions.md
   src/conditions.md
   src/animations.md
   src/states.md
   src/globals.md
   src/modules.md
   src/legacy_syntax.md

.. toctree::
   :maxdepth: 4
   :caption: Slint Language Builtins

   src/builtin_callbacks.md
   src/builtin_elements.md
   src/builtin_enums.md
   src/builtin_functions.md
   src/builtin_namespaces.md
   src/builtin_structs.md
   src/widgets.md
