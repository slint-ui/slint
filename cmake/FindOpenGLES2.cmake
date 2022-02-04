# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

include(CheckCXXSourceCompiles)

find_path(OPENGLES2_INCLUDE_DIR NAMES "GLES2/gl2.h" "OpenGLES/ES2/gl.h" DOC "The path where the OpenGL ES 2.0 headers are located")
find_library(OPENGLES2_LIBRARY NAMES GLESv2 OpenGLES)

# Sometimes EGL linkage is required, so look it up and always use if available.
find_package(OpenGL COMPONENTS EGL)

# See if we can compile some example code with what we've found.
set(saved_libraries "${CMAKE_REQUIRED_LIBRARIES}")
set(saved_includes "${CMAKE_REQUIRED_INCLUDES}")

if (OPENGLES2_LIBRARY)
    list(APPEND CMAKE_REQUIRED_INCLUDES "${OPENGLES2_INCLUDE_DIR}")
    list(APPEND CMAKE_REQUIRED_LIBRARIES "${OPENGLES2_LIBRARY}")
endif()

if(OPENGL_egl_LIBRARY)
    list(APPEND CMAKE_REQUIRED_INCLUDES "${OPENGL_EGL_INCLUDE_DIRS}")
    list(APPEND CMAKE_REQUIRED_LIBRARIES "${OPENGL_egl_LIBRARY}")
endif()

check_cxx_source_compiles("
#include <GLES2/gl2.h>
#include <GLES2/gl2platform.h>

int main(int argc, char *argv[]) {
    glClear(GL_STENCIL_BUFFER_BIT);
    glUseProgram(0);
}" HAVE_OPENGLES2)

set(CMAKE_REQUIRED_INCLUDES "${saved_includes}")
set(CMAKE_REQUIRED_LIBRARIES "${saved_libraries}")

# Standard CMake package dance
set(package_args OPENGLES2_INCLUDE_DIR OPENGLES2_LIBRARY HAVE_OPENGLES2)
include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(OpenGLES2 DEFAULT_MSG ${package_args})
mark_as_advanced(${package_args})

# Create a convenience target for linkage
if (OPENGLES2_FOUND AND NOT TARGET OpenGLES2::OpenGLES2)
    add_library(OpenGLES2::OpenGLES2 UNKNOWN IMPORTED)
    set_property(TARGET OpenGLES2::OpenGLES2 PROPERTY INTERFACE_INCLUDE_DIRECTORIES "${OPENGLES2_INCLUDE_DIR}")
    set_property(TARGET OpenGLES2::OpenGLES2 PROPERTY IMPORTED_LOCATION "${OPENGLES2_LIBRARY}")
    if (TARGET OpenGL::EGL)
        target_link_libraries(OpenGLES2::OpenGLES2 INTERFACE OpenGL::EGL)
    endif()
endif()
