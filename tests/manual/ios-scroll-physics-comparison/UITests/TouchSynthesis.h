// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#import <Foundation/Foundation.h>

/// Synthesizes `pathCount` touches in one event record.
/// Path `p` uses `pointCounts[p]` consecutive entries of `times`, `xs`, and `ys`: the first is the
/// touch down, the others are moves. It lifts at `liftTimes[p]`. Times are seconds from the start
/// of the record; coordinates are screen points.
BOOL synthesizeTouchPaths(pid_t processID, int pathCount, const int *pointCounts,
                          const double *times, const double *xs, const double *ys,
                          const double *liftTimes);
