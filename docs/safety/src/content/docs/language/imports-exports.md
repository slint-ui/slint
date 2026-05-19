---
title: Imports and Exports
description: Bringing components from other files into scope and making components in a file available to other files.
---

## Placement

Import and export statements may appear at the top level of a `.slint` source file in any order, interleaved with [component definitions](../file-structure/#sls.file.component.definition-forms). {#sls.module.placement}

## Imports

An import statement has the form: {#sls.import.forms}

```slint no-test
import { Item, ... } from "path";
```

It brings named items from the file referenced by `"path"` into the importing file's namespace. {#sls.import.semantics}

### Import Lists

The brace-delimited list contains zero or more *import items* separated by commas.
A trailing comma is permitted. {#sls.import.list}

An import item is either: {#sls.import.item}

- a single [identifier](../lexical-structure/#sls.lex.identifier.classes) `Name`, or
- a renaming of the form `Name as Other`.

A bare identifier `Name` brings the corresponding exported name from the imported file into the current file's namespace under the same name. {#sls.import.same-name}

A renaming `Name as Other` brings the corresponding exported name into the current file's namespace under the name `Other` only.
The original name `Name` is not introduced. {#sls.import.rename}

The identifier on the left of `as` shall refer to a name exported by the imported file. {#sls.import.left-must-exist}

<!-- TODO: extend this rule when structs, enums, and globals come into scope of the specification. -->

An imported name shall refer to a component. {#sls.import.component-only}

### Import Paths

An import path is a double-quoted string literal.
Backslash escape sequences are recognized as in any other string literal. {#sls.import.path-literal}

If the path begins with the character `@`, it is a *library import*: the remainder of the path is resolved against the compiler's configured library paths. {#sls.import.path-library}

Otherwise, the path is resolved first relative to the directory of the importing source file, then against the compiler's configured include paths.
The first match in this search order is used. {#sls.import.path-relative}

The referenced file shall exist and shall be a Slint source file. {#sls.import.path-must-exist}

### Name Clashes

Two import items in the same source file shall not introduce the same name. {#sls.import.no-clash}

## Exports

Names defined in a `.slint` source file are private to that file by default.
A name is visible to importers only if it is explicitly exported. {#sls.export.default-private}

An export statement has one of the following forms: {#sls.export.forms}

```slint no-test
export component Name { /* ... */ }
export { Name, ... }
export { Name, ... } from "path";
export * from "path";
```

### Export at the Declaration Site

The form `export component Name { ... }` defines a component and exports it in a single statement.
The exported name is the component's own name. {#sls.export.declaration-site}

### Export Lists

The form `export { Name, ... }` exports one or more names already defined in the current file.
A trailing comma is permitted. {#sls.export.list}

An export list item is either a bare identifier `Name` or a renaming of the form `Name as Other`.
A renaming exports the locally-defined `Name` under the external name `Other`.
The local name `Name` remains defined in the current file. {#sls.export.rename}

Each identifier on the left of `as`, and each bare identifier, shall refer to a name defined in the current file. {#sls.export.left-must-exist}

### Re-exports

The form `export { Name, ... } from "path";` re-exports selected names from another file.
The path follows the same rules as an [import path](#sls.import.path-literal).
Each identifier on the left of `as`, and each bare identifier, shall refer to a name exported by the file at `"path"`. {#sls.export.re-export-selective}

The form `export * from "path";` re-exports every name exported by the file at `"path"`. {#sls.export.re-export-all}

A source file shall contain at most one `export * from "path";` statement. {#sls.export.re-export-all-once}

### Duplicate Exports

A source file shall not export the same external name more than once.
This includes the combination of any declaration-site export, export-list entry, and re-export. {#sls.export.no-duplicates}
