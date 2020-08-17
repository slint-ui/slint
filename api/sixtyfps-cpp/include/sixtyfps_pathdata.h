/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
#pragma once
#include <initializer_list>
#include <string_view>
#include "sixtyfps_pathdata_internal.h"

namespace sixtyfps {

using internal::types::PathArcTo;
using internal::types::PathElement;
using internal::types::PathEvent;
using internal::types::PathLineTo;
using internal::types::Point;

struct PathData
{
public:
    using Tag = internal::types::PathData::Tag;

    PathData() : data(Data::None()) { }
    PathData(const PathElement *firstElement, size_t count)
        : data(Data::Elements(elements_from_array(firstElement, count)))
    {
    }

    PathData(const PathEvent *firstEvent, size_t event_count, const Point *firstCoordinate,
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

    static internal::types::PathData events_from_array(const PathEvent *firstEvent,
                                                       size_t event_count,
                                                       const Point *firstCoordinate,
                                                       size_t coordinate_count)
    {
        SharedArray<PathEvent> events;
        SharedArray<Point> coordinates;
        sixtyfps_new_path_events(&events, &coordinates, firstEvent, event_count, firstCoordinate,
                                 coordinate_count);
        return Data::Events(events, coordinates);
    }

    using Data = internal::types::PathData;
    Data data;
};

}
