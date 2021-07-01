# Usage via CMake

A typical example looks like this:

```cmake
cmake_minimum_required(VERSION 3.16)
project(my_application LANGUAGES CXX)

# Note: Use find_package(SixtyFPS) instead of the following three commands, if you prefer the package
# approach.
include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.1.0
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)

add_executable(my_application main.cpp)
target_link_libraries(my_application PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(my_application my_application_ui.60)
```

The `sixtyfps_target_60_sources` cmake command allows you to add .60 files to your build. Finally it is
necessary to link your executable or library against the `SixtyFPS::SixtyFPS` target.
