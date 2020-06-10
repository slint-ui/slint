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
