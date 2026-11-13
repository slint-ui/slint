# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

idf_component_register(
    SRCS "../../main.cpp"
    INCLUDE_DIRS "."
    REQUIRES slint
)

slint_target_sources(${COMPONENT_LIB} ../../../slint-project.json)
target_compile_definitions(${COMPONENT_LIB} PRIVATE EXAMPLE_TARGET_S3_BOX)
