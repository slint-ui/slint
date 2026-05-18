---
title: File Structure
description: Top-level structure of a Slint source file and the basic form of component definitions.
---

## A Slint Source File

A `.slint` source file is a sequence of zero or more *top-level items*, separated only by whitespace and comments. {#sls_source_file}

A source file that contains only whitespace and comments is well-formed and defines no components. {#sls_source_file_empty}

## Top-Level Items

A *top-level item* is a *component definition*. {#sls_top_level_item}

## Component Definitions

A component definition has one of the following two forms: {#sls_component_definition_forms}

```slint
component Name { /* component body */ }
component Name inherits Base { /* component body */ }
```

The identifier following the `component` keyword is the *name* of the component. {#sls_component_name}

The first form defines a component with no explicit base; the second form defines a component that inherits from the element type named by `Base`.
The identifier `Base` shall resolve to a built-in element or to a component previously defined in the same source file. {#sls_component_inherits}

The braces `{` and `}` delimit the *component body*. {#sls_component_body_braces}

## Component Bodies

A component body is a sequence of zero or more *element instantiations*, separated only by whitespace and comments. {#sls_component_body}

A component body that contains no element instantiations is well-formed. {#sls_component_body_empty}

## Element Instantiations

An element instantiation has the form: {#sls_element_instantiation_form}

```slint
TypeName { /* element body */ }
```

The identifier `TypeName` shall be an element type name as defined in [Element Type Names](/language/lexical-structure/#element-type-names). {#sls_element_instantiation_typename}

The braces delimit the *element body*.
An element body is a sequence of zero or more nested element instantiations, separated only by whitespace and comments. {#sls_element_body}

A nested element instantiation is a *child* of the element instantiation in whose body it appears.
The child relationship forms a tree rooted at the elements that appear directly in a component body. {#sls_element_tree}

## Example

The following is a complete, well-formed `.slint` source file: {#sls_example_intro}

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

It defines one component, `Hello`, whose body contains two top-level `Rectangle` instantiations; the first of those has one nested `Rectangle` child. {#sls_example_description}
