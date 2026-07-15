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
            id, from, to, &details, reinterpret_cast<void (*)(void *, const int32_t *)>(set_value),
            set_value_user_data, set_value_drop_user_data, on_finished, on_finished_user_data,
            on_finished_drop_user_data);
}
inline uintptr_t slint_animation_handle_restart_helper(
        uintptr_t id, int from, int to, const PropertyAnimation &details,
        void (*set_value)(void *, const int *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_handle_restart_int(
            id, from, to, &details, reinterpret_cast<void (*)(void *, const int32_t *)>(set_value),
            set_value_user_data, set_value_drop_user_data, on_finished, on_finished_user_data,
            on_finished_drop_user_data);
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

inline void *slint_animation_new_tween_helper(int from, int to, const PropertyAnimation &details,
                                              void (*set_value)(void *, const int *),
                                              void *set_value_user_data,
                                              void (*set_value_drop_user_data)(void *),
                                              void (*on_finished)(void *),
                                              void *on_finished_user_data,
                                              void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_new_tween_int(
            from, to, &details, reinterpret_cast<void (*)(void *, const int32_t *)>(set_value),
            set_value_user_data, set_value_drop_user_data, on_finished, on_finished_user_data,
            on_finished_drop_user_data);
}
inline void *slint_animation_new_tween_helper(
        float from, float to, const PropertyAnimation &details,
        void (*set_value)(void *, const float *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_new_tween_float(
            from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}
inline void *slint_animation_new_tween_helper(
        const Color &from, const Color &to, const PropertyAnimation &details,
        void (*set_value)(void *, const Color *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_new_tween_color(
            from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}
inline void *slint_animation_new_tween_helper(
        const Brush &from, const Brush &to, const PropertyAnimation &details,
        void (*set_value)(void *, const Brush *), void *set_value_user_data,
        void (*set_value_drop_user_data)(void *), void (*on_finished)(void *),
        void *on_finished_user_data, void (*on_finished_drop_user_data)(void *))
{
    return cbindgen_private::slint_animation_new_tween_brush(
            from, to, &details, set_value, set_value_user_data, set_value_drop_user_data,
            on_finished, on_finished_user_data, on_finished_drop_user_data);
}

/// Move-only owning handle to one node of a not-yet-started Delay/Parallel/Sequential/Tween
/// animation tree, built up via `new_tween`/`new_delay`/`new_parallel`/`new_sequential` and
/// `add_child` before being handed to `AnimationHandle::start`/`restart`.
class AnimationNode
{
public:
    AnimationNode() = default;
    explicit AnimationNode(void *ptr) : ptr(ptr) { }
    AnimationNode(const AnimationNode &) = delete;
    AnimationNode &operator=(const AnimationNode &) = delete;
    AnimationNode(AnimationNode &&other) noexcept : ptr(other.ptr) { other.ptr = nullptr; }
    AnimationNode &operator=(AnimationNode &&other) noexcept
    {
        if (this != &other) {
            reset();
            ptr = other.ptr;
            other.ptr = nullptr;
        }
        return *this;
    }
    ~AnimationNode() { reset(); }

    /// Build a leaf tween node animating `from` to `to`.
    template<typename T, typename SetValue, typename OnFinished>
    static AnimationNode new_tween(const T &from, const T &to, const PropertyAnimation &details,
                                   SetValue set_value, OnFinished on_finished)
    {
        struct SetValueData
        {
            SetValue set_value;
        };
        struct OnFinishedData
        {
            OnFinished on_finished;
        };
        return AnimationNode(slint_animation_new_tween_helper(
                from, to, details,
                [](void *data, const T *value) {
                    reinterpret_cast<SetValueData *>(data)->set_value(*value);
                },
                new SetValueData { set_value },
                [](void *data) { delete reinterpret_cast<SetValueData *>(data); },
                [](void *data) { reinterpret_cast<OnFinishedData *>(data)->on_finished(); },
                new OnFinishedData { on_finished },
                [](void *data) { delete reinterpret_cast<OnFinishedData *>(data); }));
    }

    /// Build a leaf spring node whose natural frequency/damping are derived from a
    /// `duration`/`bounce` pair. Unlike `new_tween`, a spring integrates in place: `get_value`
    /// reads the target's live value each frame (so external writes since the last frame are
    /// picked up) and `set_value` pushes the newly stepped value back. `initial_velocity` should
    /// be the outgoing animation's velocity (via `AnimationHandle::velocity()`) when retargeting a
    /// running spring, or `0.` when starting fresh. Register `on_finished` via `set_on_finished`
    /// (a spring doesn't take one at construction time, like Delay/Parallel/Sequential).
    template<typename GetValue, typename SetValue>
    static AnimationNode new_spring_duration_bounce(float start_value, float initial_velocity,
                                                    float target_value, float duration_secs,
                                                    float bounce, GetValue get_value,
                                                    SetValue set_value)
    {
        struct GetValueData
        {
            GetValue get_value;
        };
        struct SetValueData
        {
            SetValue set_value;
        };
        return AnimationNode(cbindgen_private::slint_animation_new_spring_duration_bounce(
                start_value, initial_velocity, target_value, duration_secs, bounce,
                [](void *data) -> float {
                    return reinterpret_cast<GetValueData *>(data)->get_value();
                },
                new GetValueData { get_value },
                [](void *data) { delete reinterpret_cast<GetValueData *>(data); },
                [](void *data, const float *value) {
                    reinterpret_cast<SetValueData *>(data)->set_value(*value);
                },
                new SetValueData { set_value },
                [](void *data) { delete reinterpret_cast<SetValueData *>(data); }));
    }

    /// Like [`new_spring_duration_bounce`], but with natural frequency/damping derived from a
    /// `mass`/`stiffness`/`damping` triple instead.
    template<typename GetValue, typename SetValue>
    static AnimationNode new_spring_physical(float start_value, float initial_velocity,
                                             float target_value, float mass, float stiffness,
                                             float damping, GetValue get_value, SetValue set_value)
    {
        struct GetValueData
        {
            GetValue get_value;
        };
        struct SetValueData
        {
            SetValue set_value;
        };
        return AnimationNode(cbindgen_private::slint_animation_new_spring_physical(
                start_value, initial_velocity, target_value, mass, stiffness, damping,
                [](void *data) -> float {
                    return reinterpret_cast<GetValueData *>(data)->get_value();
                },
                new GetValueData { get_value },
                [](void *data) { delete reinterpret_cast<GetValueData *>(data); },
                [](void *data, const float *value) {
                    reinterpret_cast<SetValueData *>(data)->set_value(*value);
                },
                new SetValueData { set_value },
                [](void *data) { delete reinterpret_cast<SetValueData *>(data); }));
    }

    /// Build a leaf delay node.
    static AnimationNode new_delay(uint64_t duration_ms)
    {
        return AnimationNode(cbindgen_private::slint_animation_new_delay(duration_ms));
    }

    /// Build an empty container node running its children in parallel.
    static AnimationNode new_parallel()
    {
        return AnimationNode(cbindgen_private::slint_animation_new_parallel());
    }

    /// Build an empty container node running its children one after another.
    static AnimationNode new_sequential()
    {
        return AnimationNode(cbindgen_private::slint_animation_new_sequential());
    }

    /// Set the number of whole passes this container node should repeat. No-op on a Tween/Delay
    /// leaf.
    void set_iteration_count(double iteration_count) const
    {
        cbindgen_private::slint_animation_container_set_iteration_count(ptr, iteration_count);
    }

    /// Move `child` into this container node. No-op (freeing `child`) if this is a Tween/Delay
    /// leaf.
    void add_child(AnimationNode &&child) const
    {
        cbindgen_private::slint_animation_container_add_child(ptr, child.release());
    }

    /// Register `on_finished` to run exactly once, the first time this node reports it's no
    /// longer running. Used to write `false` back to a `.slint` `running` property when a root
    /// Delay/Parallel/Sequential tree finishes on its own (a Tween already takes its own
    /// `on_finished` via `new_tween`).
    template<typename OnFinished>
    void set_on_finished(OnFinished on_finished) const
    {
        struct OnFinishedData
        {
            OnFinished on_finished;
        };
        cbindgen_private::slint_animation_node_set_on_finished(
                ptr, [](void *data) { reinterpret_cast<OnFinishedData *>(data)->on_finished(); },
                new OnFinishedData { on_finished },
                [](void *data) { delete reinterpret_cast<OnFinishedData *>(data); });
    }

    /// Relinquishes ownership of the underlying node to the caller (used to hand it to
    /// `AnimationHandle::start`/`restart`, or into a parent container's `add_child`).
    void *release()
    {
        void *p = ptr;
        ptr = nullptr;
        return p;
    }

private:
    void reset()
    {
        if (ptr) {
            cbindgen_private::slint_animation_box_drop(ptr);
            ptr = nullptr;
        }
    }

    void *ptr = nullptr;
};

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

    /// Start driving a Delay/Parallel/Sequential/Tween tree built via `AnimationNode`. Does
    /// nothing (freeing `node`) if this handle is already running something.
    void start(AnimationNode &&node) const
    {
        private_api::assert_main_thread();
        id = cbindgen_private::slint_animation_handle_start_box(id, node.release());
    }

    /// Force a Delay/Parallel/Sequential/Tween tree built via `AnimationNode` to (re)start from
    /// the beginning, even if this handle is already running something.
    void restart(AnimationNode &&node) const
    {
        private_api::assert_main_thread();
        id = cbindgen_private::slint_animation_handle_restart_box(id, node.release());
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

    /// Returns the current velocity of whatever is running on this handle, or 0.0 if nothing is
    /// running or the running animation doesn't track velocity (e.g. a Tween).
    float velocity() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_animation_handle_velocity(id);
    }

private:
    mutable uintptr_t id = 0;
};

} // namespace slint::private_api
