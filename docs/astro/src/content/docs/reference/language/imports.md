---
title: Imports
description: Bringing components from other files into scope.
# Unpublished placeholder; drop `draft` and uncomment the sidebar entries in
# both docs/astro/astro.config.mjs and docs/safety/astro.config.mjs once
# imports come into scope.
draft: true
---

Imports are not yet covered by this revision of the specification.

<!--
Draft content; will be uncommented once imports come into scope.

## Placement

Import statements may appear at the top level of a `.slint` source file, interleaved with [component definitions](../file-structure/#sls.file.component.definition-forms). {#sls.import.placement}

## Form

An import statement has the form: {#sls.import.forms}

```slint no-test
import { Item, ... } from "path";
```

It brings named items from the file referenced by `"path"` into the importing file's namespace. {#sls.import.semantics}

## Import Lists

The brace-delimited list contains zero or more *import items* separated by commas.
A trailing comma is permitted. {#sls.import.list}

An import item is either: {#sls.import.item}

- a single [identifier](../lexical-structure/#sls.lex.identifier.classes) `Name`, or
- a renaming of the form `Name as Other`.

A bare identifier `Name` brings the corresponding exported name from the imported file into the current file's namespace under the same name. {#sls.import.same-name}

A renaming `Name as Other` brings the corresponding exported name into the current file's namespace under the name `Other` only.
The original name `Name` is not introduced. {#sls.import.rename}

The identifier on the left of `as` shall refer to a name exported by the imported file. {#sls.import.left-must-exist}

TODO: extend this rule when structs, enums, and globals come into scope of the specification.

An imported name shall refer to a component. {#sls.import.component-only}

## Import Paths

An import path is a double-quoted string literal.
Backslash escape sequences are recognized as in any other string literal. {#sls.import.path-literal}

If the path begins with the character `@`, it is a *library import*: the remainder of the path is resolved against the compiler's configured library paths. {#sls.import.path-library}

Otherwise, the path is resolved first relative to the directory of the importing source file, then against the compiler's configured include paths.
The first match in this search order is used. {#sls.import.path-relative}

The referenced file shall exist and shall be a Slint source file. {#sls.import.path-must-exist}

## Name Clashes

Two import items in the same source file shall not introduce the same name. {#sls.import.no-clash}
-->
