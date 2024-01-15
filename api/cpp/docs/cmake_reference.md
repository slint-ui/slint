# CMake Reference

## `slint_target_sources`

```
slint_target_sources(<target> <files>....)
```

Similar to the builtin cmake [target_sources](https://cmake.org/cmake/help/latest/command/target_sources.html) macro,
this can be used to tell cmake about the .slint files to use.
The macro will take care of running the slint-compiler to convert .slint file to a .h file in the build directory,
while making sure that the include path are approprietly set so that you can include it.


In the following example, given a file called `the_window.slint`, it will create a file call `the_window.h` that can
be included from your .cpp file.

```cmake
add_executable(my_application main.cpp)
target_link_libraries(my_application PRIVATE Slint::Slint)
slint_target_sources(my_application the_window.slint)
```

## Resource Embedding

By default, images from `@image-url` or fonts that your Slint files reference are loaded from disk at run-time. This minimises build times, but requires that the directory structure with the files remains stable. If you want to build a program that runs anywhere, then you can configure the Slint compiler to embed such sources into the binary.

Set the `SLINT_EMBED_RESOURCES` target property on your CMake target to one of the following values:

* `embed-files`: The raw files are embedded in the application binary.
* `embed-for-software-renderer`: The files will be loaded by the Slint compiler, optimized for use with the software renderer and embedded in the application binary.
* `as-absolute-path`: The paths of files are made absolute and will be used at run-time to load the resources from the file system. This is the default.

This target property is initialised from the global `DEFAULT_SLINT_EMBED_RESOURCES` cache variable. Set it to configure the default for all CMake targets.

```cmake
# Example: given a when building my_app specify that the compiler should embed the resorces in the binary
set_property(TARGET my_application PROPERTY SLINT_EMBED_RESOURCES embed-files)
```
