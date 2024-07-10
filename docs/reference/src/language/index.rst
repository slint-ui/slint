.. Copyright © SixtyFPS GmbH <info@slint.dev>
.. SPDX-License-Identifier: MIT

Introduction
============

Slint is an easy to learn and use language to describe user
interfaces. It is readable to both humans and machines.

This way, we have excellent tooling on one side, while also enabling
designers and developers to know exactly what happens, by reading the code
their machine uses to display the user interfaces.

This Slint code is either interpreted at run-time or compiled to native
code, which gets built into your application together with the code in the
programming language providing the business logic. The Slint compiler can
optimize the user interface and any resources it uses at compile time, so
that user interfaces written in Slint use few resources, with regards to
performance and storage.

The Slint language enforces a separation of user interface from business logic,
using interfaces you can define for your project. This enables
cooperation between design-focused team members and those concentrating on the programming
side of the project.

The Slint language describes extensible graphical user interfaces using the
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

