.. Copyright © SixtyFPS GmbH <info@slint.dev>
.. SPDX-License-Identifier: MIT

Introduction
===========================================

The Slint Language
------------------

`Slint <https://slint.dev>`_ comes with an easy to learn and use language for you to describe user
interfaces with. It is readable to both humans and machines.

This way, we have excellent tooling on one side, while also enabling
designers and developers to know exactly what happens, by reading the code
their machine uses to display the user interfaces.

This Slint language is either interpreted at run-time or compiled to native
code, which gets built into your application together with the code in the same
programming language providing the business logic. The Slint compiler can
optimize the user interface and any resources it uses at compile time, so
that user interfaces written in Slint use few resources, with regards to
performance and storage.

The Slint language enforces a separation of user interface from business logic,
using interfaces you can define for your project. This enables a fearless
cooperation between design-focused team members and those concentrating on the programming
side of the project.


The Slint design markup language describes extensible graphical user interfaces using the
`Slint framework <https://slint.dev>`_

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

It only describes the user interface and it is not a programming language. The business
logic is written in a different programming language using the Slint API.

Getting Started
---------------

To use `Slint <https://slint.dev>`_ you need to embed your slint files in a project written
in a supported programming language, like C++, Rust, or JavaScript.

There are three different pathways to get started with Slint:

1. `SlintPad <https://slint.dev/editor>`_ - Use this to get a feel of the Slint design markup language.
   This is a web browser-based tool where you can try Slint out.

2. As a UI Designer, working with Slint files locally, we recommend the following combination of software tools:

   - Visual Studio Code
   - The Slint for Visual Studio Code Extension

3. As a Software Developer, integrating Slint into a new or existing code base, choose one of these languages to
   get started:

   - `C++ <https://slint.dev/docs/cpp/>`_
   - `Rust <https://slint.dev/docs/rust/slint/>`_
   - `JavaScript <https://slint.dev/docs/node/>`_

.. toctree::
   :hidden:
   :maxdepth: 2
   :caption: Concepts

   src/concepts/file.md
   src/concepts/layouting.md
   src/concepts/container.md
   src/concepts/focus.md
   src/concepts/fonts.md
   src/concepts/purity.md
   src/concepts/translations.md

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
   src/builtins/globals.md
   src/builtins/namespaces.md
   src/builtins/structs.md
   src/builtins/widgets.md

.. toctree::
   :hidden:
   :maxdepth: 2
   :caption: Recipes & Examples

   src/recipes/recipes.md
