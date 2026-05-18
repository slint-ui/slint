---
title: File Structure
description: Top-level structure of a Slint source file and the basic form of component definitions.
---

## A Slint Source File

A `.slint` source file is a sequence of zero or more *top-level items*, separated only by whitespace and comments.

A source file that contains only whitespace and comments is well-formed and defines no components.

## Top-Level Items

A *top-level item* is a *component definition*.

## Component Definitions

A component definition has one of the following two forms:

```slint
component Name { /* component body */ }
component Name inherits Base { /* component body */ }
```

The identifier following the `component` keyword is the *name* of the component.

The first form defines a component with no explicit base; the second form defines a component that inherits from the element type named by `Base`.
The identifier `Base` shall resolve to a built-in element or to a component previously defined in the same source file.

The braces `{` and `}` delimit the *component body*.

## Component Bodies

A component body is a sequence of zero or more *element instantiations*, separated only by whitespace and comments.

A component body that contains no element instantiations is well-formed.

## Element Instantiations

An element instantiation has the form:

```slint
TypeName { /* element body */ }
```

The identifier `TypeName` shall be an element type name as defined in [Element Type Names](/language/lexical-structure/#element-type-names).

The braces delimit the *element body*.
An element body is a sequence of zero or more nested element instantiations, separated only by whitespace and comments.

A nested element instantiation is a *child* of the element instantiation in whose body it appears.
The child relationship forms a tree rooted at the elements that appear directly in a component body.

## Example

The following is a complete, well-formed `.slint` source file:

```slint
component Hello {
    Rectangle {
        Rectangle {
        }
    }
    Rectangle {
    }
}
```

It defines one component, `Hello`, whose body contains two top-level `Rectangle` instantiations; the first of those has one nested `Rectangle` child.
