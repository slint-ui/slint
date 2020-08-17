/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
#pragma once
#include <string_view>
#include "sixtyfps_resource_internal.h"
#include "sixtyfps_string.h"

namespace sixtyfps {

struct Resource
{
public:
    using Tag = internal::types::Resource::Tag;

    Resource() : data(Data::None()) { }
    Resource(const SharedString &file_path) : data(Data::AbsoluteFilePath(file_path)) { }

private:
    using Data = internal::types::Resource;
    Data data;
};

}
