#pragma once
#include <string_view>
#include "sixtyfps_resource_internal.h"
#include "sixtyfps_string.h"

namespace sixtyfps {

struct Resource
{
public:
    using Tag = internal::types::Resource::Tag;

    Resource() : data(create()) { }
    Resource(const SharedString &file_path) : data(create())
    {
        ::new (&data.embedded_data)
                internal::types::Resource::AbsoluteFilePath_Body { Tag::AbsoluteFilePath,
                                                                   file_path };
    }

private:
    internal::types::Resource data;

    static internal::types::Resource create()
    {
        union U {
            U() { data.tag = Tag::None; }
            internal::types::Resource data;
            ~U() { }
        } u;
        return u.data;
    }
};

}
