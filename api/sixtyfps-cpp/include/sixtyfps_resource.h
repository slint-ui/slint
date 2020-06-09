#pragma once
#include <string_view>
#include "sixtyfps_resource_internal.h"
#include "sixtyfps_string.h"

namespace sixtyfps {

union ResourceData {
    SharedString absolute_file_path;

    ResourceData() { }
    ~ResourceData() { }
};

struct Resource
{
public:
    using Tag = internal::types::Resource::Tag;

    Resource() : tag(Tag::None) { }
    Resource(const SharedString &file_path) : tag(Tag::AbsoluteFilePath)
    {
        new (&data.absolute_file_path) SharedString(file_path);
    }
    Resource(const Resource &other) : tag(other.tag)
    {
        switch (tag) {
        case Tag::None:
            break;
        case Tag::AbsoluteFilePath:
            new (&data.absolute_file_path) SharedString(other.data.absolute_file_path);
        }
    }
    ~Resource() { destroy(); }
    Resource &operator=(const Resource &other)
    {
        if (this == &other)
            return *this;
        destroy();
        tag = other.tag;
        switch (tag) {
        case Tag::None:
            break;
        case Tag::AbsoluteFilePath:
            new (&data.absolute_file_path) SharedString(other.data.absolute_file_path);
        }
        return *this;
    }

private:
    void destroy()
    {
        switch (tag) {
        case Tag::None:
            break;
        case Tag::AbsoluteFilePath:
            data.absolute_file_path.~SharedString();
        }
    }

    Tag tag;
    ResourceData data;
};

}
