// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <initializer_list>
#include <string_view>
#include "slint_pathdata_internal.h"

namespace slint::private_api {
using cbindgen_private::PathEvent;
using cbindgen_private::types::PathArcTo;
using cbindgen_private::types::PathCubicTo;
using cbindgen_private::types::PathElement;
using cbindgen_private::types::PathLineTo;
using cbindgen_private::types::PathMoveTo;
using cbindgen_private::types::PathQuadraticTo;
using cbindgen_private::types::Point;

struct PathData
{
public:
    using Tag = cbindgen_private::types::PathData::Tag;

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

    PathData(const SharedString &commands)
        : data(cbindgen_private::types::PathData::Commands(commands))
    {
    }

    friend bool operator==(const PathData &a, const PathData &b) = default;

private:
    static SharedVector<PathElement> elements_from_array(const PathElement *firstElement,
                                                         size_t count)
    {
        SharedVector<PathElement> tmp;
        slint_new_path_elements(&tmp, firstElement, count);
        return tmp;
    }

    static cbindgen_private::types::PathData events_from_array(const PathEvent *firstEvent,
                                                               size_t event_count,
                                                               const Point *firstCoordinate,
                                                               size_t coordinate_count)
    {
        SharedVector<PathEvent> events;
        SharedVector<Point> coordinates;
        slint_new_path_events(&events, &coordinates, firstEvent, event_count, firstCoordinate,
                              coordinate_count);
        return Data::Events(events, coordinates);
    }

    using Data = cbindgen_private::types::PathData;
    Data data;
};

}
