# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT


idf_component_register(
    SRCS "main.cpp"
    INCLUDE_DIRS "."
    REQUIRES slint esp-box
)

slint_target_sources(${COMPONENT_LIB} ../../ui/printerdemo.slint)
target_link_options(${COMPONENT_LIB} PUBLIC -Wl,--allow-multiple-definition)
set_target_properties(${COMPONENT_LIB} PROPERTIES SLINT_EMBED_RESOURCES "embed-for-software-renderer")

# workaround false positive:
# ```
# In member function 'void slint::private_api::Repeater<...>::RepeaterInner::row_added(size_t, size_t):
# vtable.h:111:29: error: '... vtable::VRc<...>::inner' may be used uninitialized in this function [-Werror=maybe-uninitialized]
# 111 |     VRc(const VRc &other) : inner(other.inner) { inner->strong_ref++; }
# ```
target_compile_options(${COMPONENT_LIB} PRIVATE -Wno-maybe-uninitialized)
