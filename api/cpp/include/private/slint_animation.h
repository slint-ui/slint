// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "private/slint_animation_internal.h"
#include "private/slint_properties_internal.h"
#include "private/slint_color.h"
#include "private/slint_brush.h"
#include "private/slint_timer.h"

namespace slint::private_api {

using cbindgen_private::PropertyAnimation;

inline uintptr_t slint_animation_handle_start_helper(
        uintptr_t id, int from, int to, const PropertyAnimation &details,
        void (*set_value)(void *, const int *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_start_int(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}
inline uintptr_t slint_animation_handle_restart_helper(
        uintptr_t id, int from, int to, const PropertyAnimation &details,
        void (*set_value)(void *, const int *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_restart_int(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}

inline uintptr_t slint_animation_handle_start_helper(
        uintptr_t id, float from, float to, const PropertyAnimation &details,
        void (*set_value)(void *, const float *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_start_float(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}
inline uintptr_t slint_animation_handle_restart_helper(
        uintptr_t id, float from, float to, const PropertyAnimation &details,
        void (*set_value)(void *, const float *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_restart_float(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}

inline uintptr_t slint_animation_handle_start_helper(
        uintptr_t id, const Color &from, const Color &to, const PropertyAnimation &details,
        void (*set_value)(void *, const Color *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_start_color(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}
inline uintptr_t slint_animation_handle_restart_helper(
        uintptr_t id, const Color &from, const Color &to, const PropertyAnimation &details,
        void (*set_value)(void *, const Color *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_restart_color(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}

inline uintptr_t slint_animation_handle_start_helper(
        uintptr_t id, const Brush &from, const Brush &to, const PropertyAnimation &details,
        void (*set_value)(void *, const Brush *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_start_brush(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}
inline uintptr_t slint_animation_handle_restart_helper(
        uintptr_t id, const Brush &from, const Brush &to, const PropertyAnimation &details,
        void (*set_value)(void *, const Brush *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_restart_brush(
            id, from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}

/// Handle to an animation
struct AnimationHandle
{
    AnimationHandle() = default;
    AnimationHandle(const AnimationHandle &) = delete;
    AnimationHandle &operator=(const AnimationHandle &) = delete;
    ~AnimationHandle()
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_animation_handle_drop(id);
    }

    // TODO fix this needs to be more generic to fit with the rest of animation types

    /// Start a tween from `from` to `to`. Does nothing if it is already running
    template<typename T, typename SetValue, typename OnFinished>
    void start(const T &from, const T &to, const PropertyAnimation &details, SetValue set_value,
               OnFinished on_finished) const
    {
        private_api::assert_main_thread();
        struct SetValueData
        {
            SetValue set_value;
        };
        struct OnFinishedData
        {
            OnFinished on_finished;
        };
        id = slint_animation_handle_start_helper(
                id, from, to, details,
                [](void *data, const T *value) {
                    reinterpret_cast<SetValueData *>(data)->set_value(*value);
                },
                new SetValueData { set_value },
                [](void *data) { delete reinterpret_cast<SetValueData *>(data); },
                [](void *data) { reinterpret_cast<OnFinishedData *>(data)->on_finished(); },
                new OnFinishedData { on_finished },
                [](void *data) { delete reinterpret_cast<OnFinishedData *>(data); });
    }

    /// Force a tween from `from` to `to` to (re)start from the beginning
    /// even if it's already running
    template<typename T, typename SetValue, typename OnFinished>
    void restart(const T &from, const T &to, const PropertyAnimation &details, SetValue set_value,
                 OnFinished on_finished) const
    {
        private_api::assert_main_thread();
        struct SetValueData
        {
            SetValue set_value;
        };
        struct OnFinishedData
        {
            OnFinished on_finished;
        };
        id = slint_animation_handle_restart_helper(
                id, from, to, details,
                [](void *data, const T *value) {
                    reinterpret_cast<SetValueData *>(data)->set_value(*value);
                },
                new SetValueData { set_value },
                [](void *data) { delete reinterpret_cast<SetValueData *>(data); },
                [](void *data) { reinterpret_cast<OnFinishedData *>(data)->on_finished(); },
                new OnFinishedData { on_finished },
                [](void *data) { delete reinterpret_cast<OnFinishedData *>(data); });
    }

    /// Stop and deregister the animation freezing the property
    void stop() const
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_animation_handle_stop(id);
    }

    /// Returns true the handle contains a running animation
    bool is_running() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_animation_handle_is_running(id);
    }

private:
    mutable uintptr_t id = 0;
};

} // namespace slint::private_api
