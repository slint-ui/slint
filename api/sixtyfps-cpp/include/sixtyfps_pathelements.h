#pragma once
#include <initializer_list>
#include <string_view>
#include "sixtyfps_pathelements_internal.h"

namespace sixtyfps {

using internal::types::PathArcTo;
using internal::types::PathElement;
using internal::types::PathEvent;
using internal::types::PathEventBegin;
using internal::types::PathEventCubic;
using internal::types::PathEventEnd;
using internal::types::PathEventLine;
using internal::types::PathEventQuadratic;
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

    PathElements(const PathEvent *firstEvent, size_t count)
        : data(Data::PathEvents(events_from_array(firstEvent, count)))
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

    static SharedArray<PathEvent> events_from_array(const PathEvent *firstEvent, size_t count)
    {
        SharedArray<PathEvent> tmp;
        sixtyfps_new_path_events(&tmp, firstEvent, count);
        return tmp;
    }

    using Data = internal::types::PathElements;
    Data data;
};

}
