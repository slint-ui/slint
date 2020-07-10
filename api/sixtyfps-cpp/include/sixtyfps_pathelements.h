#pragma once
#include <initializer_list>
#include <string_view>
#include "sixtyfps_pathelements_internal.h"

namespace sixtyfps {

using internal::types::PathArcTo;
using internal::types::PathElement;
using internal::types::PathEvent;
using internal::types::PathLineTo;
using internal::types::Point;

struct PathElements
{
public:
    using Tag = internal::types::PathElements::Tag;

    PathElements() : data(Data::None()) { }
    PathElements(const PathElement *firstElement, size_t count)
        : data(Data::SharedElements(elements_from_array(firstElement, count)))
    {
    }

    PathElements(const PathEvent *firstEvent, size_t event_count, const Point *firstCoordinate,
                 size_t coordinate_count)
        : data(events_from_array(firstEvent, event_count, firstCoordinate, coordinate_count))
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

    static internal::types::PathElements events_from_array(const PathEvent *firstEvent,
                                                           size_t event_count,
                                                           const Point *firstCoordinate,
                                                           size_t coordinate_count)
    {
        SharedArray<PathEvent> events;
        SharedArray<Point> coordinates;
        sixtyfps_new_path_events(&events, &coordinates, firstEvent, event_count, firstCoordinate,
                                 coordinate_count);
        return Data::PathEvents(events, coordinates);
    }

    using Data = internal::types::PathElements;
    Data data;
};

}
