# CMake Reference
<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->


## `slint_target_sources`

```
slint_target_sources(<target> <files>.... [NAMESPACE namespace] [LIBRARY_PATHS name1=lib1 name2=lib2 ...])
```

Use this function to tell cmake about the .slint files of your application, similar to the builtin cmake [target_sources](https://cmake.org/cmake/help/latest/command/target_sources.html) function.
The function takes care of running the slint-compiler to convert `.slint` files to `.h` files in the build directory,
and extend  the include directories of your target so that the generated file is found when including it in your application.

The optional `NAMESPACE` argument will put the generated components in the given C++ namespace.

Use the `LIBRARY_PATHS` argument to specify the name and paths to [component libraries](slint-reference:src/language/syntax/modules#component-libraries),
separated by an equals sign (`=`).

Given a file called `the_window.slint`, the following example will create a file called `the_window.h` that can
be included from your .cpp file. Assuming the `the_window.slint` contains a component `TheWindow`, the output
C++ class will be put in the namespace `ui`, resulting to `ui::TheWindow`. Any import from `@mycomponentlib/` will
be redirected to the specified path.

```cmake
add_executable(my_application main.cpp)
target_link_libraries(my_application PRIVATE Slint::Slint)
slint_target_sources(my_application the_window.slint 
    NAMESPACE ui
    LIBRARY_PATHS mycomponentlib=/path/to/customcomponents
)
```


## Resource Embedding

By default, images from [`@image-url()`](slint-reference:src/language/syntax/types#images) or fonts that your Slint files reference are loaded from disk at run-time. This minimises build times, but requires that the directory structure with the files remains stable. If you want to build a program that runs anywhere, then you can configure the Slint compiler to embed such sources into the binary.

Set the `SLINT_EMBED_RESOURCES` target property on your CMake target to one of the following values:

* `embed-files`: The raw files are embedded in the application binary.
* `embed-for-software-renderer`: The files will be loaded by the Slint compiler, optimized for use with the software renderer and embedded in the application binary.
* `as-absolute-path`: The paths of files are made absolute and will be used at run-time to load the resources from the file system. This is the default.

This target property is initialised from the global `DEFAULT_SLINT_EMBED_RESOURCES` cache variable. Set it to configure the default for all CMake targets.

```cmake
# Example: when building my_application, specify that the compiler should embed the resources in the binary
set_property(TARGET my_application PROPERTY SLINT_EMBED_RESOURCES embed-files)
```

## Scale Factor for Microcontrollers

When targeting a Microcontroller, there exists no windowing system that provides a device pixel ratio to
map logical lengths in Slint (`px`) to physical pixels (`phx`). If desired, you can provide this ratio at
compile time by setting the `SLINT_SCALE_FACTOR` target property on your CMake target.

```cmake
# Example: when building my_application, specify that the scale factor shall be 2
set_property(TARGET my_application PROPERTY SLINT_SCALE_FACTOR 2.0)
```

A scale factor specified this way will also be used to pre-scale images and glyphs when used in combination
with [Resource Embedding](#resource-embedding).
