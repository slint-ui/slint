// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>

namespace {
struct TestVTable
{
    vtable::Layout (*drop_in_place)(vtable::VRefMut<TestVTable>);
    void (*dealloc)(const TestVTable *, uint8_t *, vtable::Layout);
};

void *dropped_instance = nullptr;
vtable::Layout freed_layout {};

struct alignas(32) OverAligned
{
    int value = 42;
    static const TestVTable static_vtable;
};

const TestVTable OverAligned::static_vtable {
    [](vtable::VRefMut<TestVTable> self) {
        dropped_instance = self.instance;
        return vtable::drop_in_place<TestVTable, OverAligned>(self);
    },
    [](const TestVTable *vt, uint8_t *ptr, vtable::Layout layout) {
        freed_layout = layout;
        vtable::dealloc(vt, ptr, layout);
    },
};
}

TEST_CASE("VRc with over-aligned data dropped through Dyn")
{
    using Inner = vtable::VRcInner<TestVTable, OverAligned>;
    std::optional rc = vtable::VRc<TestVTable, OverAligned>::make();
    void *data = &**rc;
    std::optional weak = vtable::VWeak<TestVTable, OverAligned>(*rc);
    std::optional dyn = vtable::VRc<TestVTable, vtable::Dyn>(rc->into_dyn());

    rc.reset();
    dyn.reset();
    REQUIRE(dropped_instance == data);
    REQUIRE(!weak->lock());

    weak.reset();
    REQUIRE(freed_layout.size == sizeof(Inner));
    REQUIRE(freed_layout.align == alignof(Inner));
}
