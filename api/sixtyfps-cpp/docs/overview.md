
# Overview

The following two sections explain how you can integrate your `.60` designs into your
C++ application. The entry point is a `.60` file that contains your primary component
that you instantiate from C++.

There are two ways in that you can instantiate your `.60` designs in your C++ application,
either by compiling them ahead of time or by dynamically loading them at run-time.

Once instantiated you feed data into it, for example by setting properties, populating
data models or setting up callbacks that are invoked when the user activates certain elements.


## Compiled `.60` designs

You can choose to compile a `.60` file to C++, which provides the best performance
and lowest memory consumption.

The `sixtyfps_target_60_sources` cmake command makes the translation automatic
and [generated code](generated_code.md) has an API that allows setting and getting
property values, etc. That API will use types from the {ref}`sixtyfps <namespace_sixtyfps>`
namespace, for example {cpp:class}`sixtyfps::SharedString` or {cpp:class}`sixtyfps::Color`.

## Run-time interpreted `.60` designs

Instead of compiling `.60` designs to C++, you can also choose to dynamically load `.60`
files at run-time. This is slower than compiling them ahead of time and requires more memory,
however it provides more flexibility in your application design.

The entry point to loading a `.60` file is the {cpp:class}`sixtyfps::interpreter::ComponentCompiler`
class in the {ref}`sixtyfps::interpreter <namespace_sixtyfps__interpreter>` namespace.

With the help of {cpp:class}`sixtyfps::interpreter::ComponentCompiler` you create a {cpp:class}`sixtyfps::interpreter::ComponentDefinition`,
which provides you with information about properties and callbacks that are common to all instances. The
{cpp:func}`sixtyfps::interpreter::ComponentDefinition::create()` function creates new instances, which
are wrapped in {cpp:class}`sixtyfps::ComponentHandle`. This is a smart pointer that owns the actual instance
and keeps it alive as long as at least one {cpp:class}`sixtyfps::ComponentHandle` is in scope, similar to `std::shared_ptr<T>`.

All property values in `.60` are mapped to {cpp:class}`sixtyfps::interpreter::Value` in C++. This is a
polymorphic data type that can hold different kinds of values, such as numbers, strings or even data models.

For more complex UIs it is common to supply data in the form of an abstract data model, that is used with
[`for` - `in`](markdown/langref.md#repetition) repetitions or [`ListView`](markdown/widgets.md#listview) elements in the `.60` language.
All models in C++ with the interpreter API are sub-classes of the {cpp:class}`sixtyfps::Model` where the template
parameter is {cpp:class}`sixtyfps::interpreter::Value`. Therefore to provide your own data model, you can subclass
`sixtyfps::Model<sixtyfps::interpreter::Value>`.