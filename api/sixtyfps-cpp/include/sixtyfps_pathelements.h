#pragma once
#include <initializer_list>
#include <string_view>
#include "sixtyfps_pathelements_internal.h"

namespace sixtyfps {

using internal::types::PathArcTo;
using internal::types::PathElement;
using internal::types::PathLineTo;

struct PathElements
{
public:
    using Tag = internal::types::PathElements::Tag;

    PathElements() : data(Data::None()) { }
    PathElements(const PathElement *firstElement, size_t count)
        : data(Data::SharedElements(elements_from_array(firstElement, count)))
    {
    }

private:
    static SharedArray<PathElement> elements_from_array(const PathElement *firstElement,
                                                        size_t count)
    {
        SharedArray<PathElement> tmp;
        sixtyfps_new_path_elements(&tmp, firstElement, count);
        return tmp;
    }

    using Data = internal::types::PathElements;
    Data data;
};

}
