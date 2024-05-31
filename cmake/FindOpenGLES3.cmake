# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

include(CheckCXXSourceCompiles)

find_path(OPENGLES3_INCLUDE_DIR NAMES "GLES3/gl3.h" "OpenGLES/ES3/gl.h" DOC "The path where the OpenGL ES 3.0 headers are located")
# GLESv3 entry points are in v2 lib
find_library(OPENGLES3_LIBRARY NAMES GLESv2 OpenGLES)

# Sometimes EGL linkage is required, so look it up and always use if available.
find_package(OpenGL COMPONENTS EGL)

# See if we can compile some example code with what we've found.
set(saved_libraries "${CMAKE_REQUIRED_LIBRARIES}")
set(saved_includes "${CMAKE_REQUIRED_INCLUDES}")

if (OPENGLES3_LIBRARY AND OPENGLES3_INCLUDE_DIR)
    list(APPEND CMAKE_REQUIRED_INCLUDES "${OPENGLES3_INCLUDE_DIR}")
    list(APPEND CMAKE_REQUIRED_LIBRARIES "${OPENGLES3_LIBRARY}")
endif()

if(OPENGL_egl_LIBRARY)
    list(APPEND CMAKE_REQUIRED_INCLUDES "${OPENGL_EGL_INCLUDE_DIRS}")
    list(APPEND CMAKE_REQUIRED_LIBRARIES "${OPENGL_egl_LIBRARY}")
endif()

check_cxx_source_compiles("
#include <GLES3/gl3.h>
#include <GLES3/gl3platform.h>

int main(int argc, char *argv[]) {
    glClear(GL_STENCIL_BUFFER_BIT);
    glUseProgram(0);
}" HAVE_OPENGLES3)

set(CMAKE_REQUIRED_INCLUDES "${saved_includes}")
set(CMAKE_REQUIRED_LIBRARIES "${saved_libraries}")

# Standard CMake package dance
set(package_args OPENGLES3_INCLUDE_DIR OPENGLES3_LIBRARY HAVE_OPENGLES3)
include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(OpenGLES3 DEFAULT_MSG ${package_args})
mark_as_advanced(${package_args})

# Create a convenience target for linkage
if (OPENGLES3_FOUND AND NOT TARGET OpenGLES3::OpenGLES3)
    add_library(OpenGLES3::OpenGLES3 UNKNOWN IMPORTED)
    set_property(TARGET OpenGLES3::OpenGLES3 PROPERTY INTERFACE_INCLUDE_DIRECTORIES "${OPENGLES3_INCLUDE_DIR}")
    set_property(TARGET OpenGLES3::OpenGLES3 PROPERTY IMPORTED_LOCATION "${OPENGLES3_LIBRARY}")
    if (TARGET OpenGL::EGL)
        target_link_libraries(OpenGLES3::OpenGLES3 INTERFACE OpenGL::EGL)
    endif()
endif()
