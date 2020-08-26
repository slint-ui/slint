/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once
#include <string_view>
#include "sixtyfps_resource_internal.h"
#include "sixtyfps_string.h"

namespace sixtyfps {

struct Resource
{
public:
    using Tag = cbindgen_private::types::Resource::Tag;

    Resource() : data(Data::None()) { }
    Resource(const SharedString &file_path) : data(Data::AbsoluteFilePath(file_path)) { }

private:
    using Data = cbindgen_private::types::Resource;
    Data data;
};

}
