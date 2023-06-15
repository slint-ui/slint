.. Copyright © SixtyFPS GmbH <info@slint-ui.com>
.. SPDX-License-Identifier: MIT

.. Slint C++ documentation master file

Welcome to Slint C++'s documentation!
========================================

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Getting Started

   cmake.md

   First Steps <getting_started.md>

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: C++ / .slint Integration

   Overview <overview.md>

   Type Mapping to C++ <types.md>

   Example Generated Code <generated_code.md>

.. toctree::
   :maxdepth: 2
   :hidden:
   :caption: Reference

   api/library_root

   genindex

   markdown/debugging_techniques.md

.. image:: https://github.com/slint-ui/slint/workflows/CI/badge.svg
   :target: https://github.com/slint-ui/slint/actions
   :alt: GitHub CI Build Status

.. image:: https://img.shields.io/github/discussions/slint-ui/slint
   :target: https://github.com/slint-ui/slint/discussions
   :alt: GitHub Discussions

`Slint <https://slint-ui.com/>`_ is a toolkit to efficiently develop fluid graphical user interfaces for any display: embedded devices and desktop applications.
Slint C++ is the C++ API to interact with a Slint UI from C++.

The .slint Markup Language
=======================

Slint comes with a markup language that is specifically designed for user interfaces. This language provides a
powerful way to describe graphical elements, their placement, and the flow of data through the different states. It is a familiar syntax to describe the hierarchy
of elements and property bindings. Here's the obligatory "Hello World":

.. code-block:: slint,ignore

    export component HelloWorld inherits Window {
        width: 400px;
        height: 400px;

        Text {
           y: parent.width / 2;
           x: parent.x + 200px;
           text: "Hello, world";
           color: blue;
        }
    }

Check out the `Slint Language Documentation <../slint>`_ for more details.

Architecture
============

An application is composed of the business logic written in C++ and the `.slint` user interface design markup, which
is compiled to native code.

.. image:: https://slint-ui.com/resources/architecture.drawio.svg
  :alt: Architecture Overview

Developing
==========

You can create and edit `.slint` files using our `Slint Visual Studio Code Extension <https://marketplace.visualstudio.com/items?itemName=Slint.slint>`_,
which features syntax highlighting and live design preview.

For a quick edit and preview cycle, you can also use the :code:`slint-viewer` command line tool, which can be installed using :code:`cargo install slint-viewer`,
if you have `Cargo <https://marketplace.visualstudio.com/items?itemName=Slint.slint>`_ installed.

In the next section you will learn how to install the Slint C++ library and the CMake build system integration.
