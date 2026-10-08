# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
# cSpell: ignore autoinit

cmake_minimum_required(VERSION 3.21)
project(vtk_cpp LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

find_package(VTK 9.4 REQUIRED COMPONENTS
    CommonComputationalGeometry
    FiltersSources
    InteractionStyle
    RenderingOpenGL2
)
# VTK builds with MPI support, such as Ubuntu's, link the requested modules privately to MPI::MPI_C,
# but VTK only finds MPI when the mpi component is requested.
if (TARGET VTK::mpi)
    enable_language(C)
    find_package(MPI REQUIRED COMPONENTS C)
endif()

add_executable(vtk_example main.cpp vtk_view.cpp)
target_compile_features(vtk_example PRIVATE cxx_std_20)
target_link_libraries(vtk_example PRIVATE Slint::Slint ${VTK_LIBRARIES})
slint_target_sources(vtk_example app.slint)
vtk_module_autoinit(TARGETS vtk_example MODULES ${VTK_LIBRARIES})
