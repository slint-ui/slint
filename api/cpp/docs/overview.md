
# Overview

The following sections explain how to integrate your `.slint` designs into your
C++ application. The entry point is the `.slint` file containing the primary
component you need to instantiate from C++.

Slint is a very flexible system and allows for different integration options.

First you can compile your Slint designs ahead of time into C++ code. This
code is then built into your application. This allows for the smallest
possible memory footprint and the best possible performance.

The second approach is to load your Slint designs at run-time, interpreting
them as needed. This enables even more dynamic user interfaces that can be
changed at run-time, but comes at the price of having less opportunity to apply
optimizations.

Either way, once your user interface is shown, you interact with it from C++,
for example by setting properties, populating data models or setting up and
handling callbacks to react to events triggered by the user.

## Compiled `.slint` Designs

The provided CMake integration makes it easy to compile your Slint sources:
The `slint_target_sources` CMake command makes the translation automatic. The
[generated code](generated_code.md) has an API to set and get property values,
etc. This API uses types from the {ref}`slint <namespace_slint>` namespace, for
example {cpp:class}`slint::SharedString` or {cpp:class}`slint::Color`.

## Run-Time Interpreted `.slint` Designs

Instead of compiling `.slint` designs to C++, you can dynamically load `.slint`
files at run-time. This is slower than compiling them ahead of time and requires
more memory, however it provides more flexibility in your application design.

The entry point to loading a `.slint` file is the
{cpp:class}`slint::interpreter::ComponentCompiler` class in the
{ref}`slint::interpreter <namespace_slint__interpreter>` namespace.

With the help of {cpp:class}`slint::interpreter::ComponentCompiler` you create
a {cpp:class}`slint::interpreter::ComponentDefinition`, which provides
information on properties and callbacks common to all instances. The
{cpp:func}`slint::interpreter::ComponentDefinition::create()` function creates
new instances, wrapped in a {cpp:class}`slint::ComponentHandle`. This is a smart
pointer that owns the actual instance and keeps it alive as long as at least one
{cpp:class}`slint::ComponentHandle` is in scope, similar to
`std::shared_ptr<T>`.

All property values in `.slint` are mapped to
{cpp:class}`slint::interpreter::Value` in C++. This is a polymorphic data type
that can hold different kinds of values, such as numbers, strings or even data
models.

More complex user interfaces commonly consume data in the form of an abstract
data model, that is used with [`for` - `in`](markdown/langref.md#repetition)
repetitions or [`ListView`](markdown/widgets.md#listview) elements in the
`.slint` language. All models in C++ with the interpreter API are sub-classes
of the {cpp:class}`slint::Model` where the template parameter is
{cpp:class}`slint::interpreter::Value`. To provide your own data model, you can
subclass `slint::Model<slint::interpreter::Value>`.

It's possible to declare [singletons that are globally available](markdown/langref.md#global-singletons)
in `.slint` files. You can access them from to your C++ code by exporting them
and using the getter and setter functions on
{cpp:class}`slint::interpreter::ComponentInstance` to change properties and
callbacks:

1. {cpp:func}`slint::interpreter::ComponentInstance::set_global_property()`
1. {cpp:func}`slint::interpreter::ComponentInstance::get_global_property()`
1. {cpp:func}`slint::interpreter::ComponentInstance::set_global_callback()`
1. {cpp:func}`slint::interpreter::ComponentInstance::invoke_global_callback()`
