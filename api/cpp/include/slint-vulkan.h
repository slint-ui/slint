// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint.h"

#if !defined(SLINT_FEATURE_UNSTABLE_WGPU_30) && !defined(DOXYGEN)
#    error                                                                                         \
            "slint-vulkan.h needs Slint built with a Skia renderer (SLINT_FEATURE_RENDERER_SKIA)"
#endif

#if !__has_include(<vulkan/vulkan.h>) && !defined(DOXYGEN)
#    error                                                                                         \
            "slint-vulkan.h needs the Vulkan headers (install the Vulkan SDK, or libvulkan-dev)"
#endif

#include <functional>
#include <optional>
#include <utility>

#include <vulkan/vulkan.h>

#include "private/slint_platform_internal.h"

/// Use the types in this namespace to render with Vulkan into what Slint shows.
///
/// Slint renders through wgpu, and asks it for its Vulkan backend. Everything here refers to the
/// objects wgpu created: an application draws with the same device and submits on the same queue,
/// which is what lets Slint show the result without copying it, and what orders the drawing
/// against Slint's own without any semaphore of the application's.
///
/// *Note*: These types are behind a feature flag and may be removed or changed in future minor
///         releases, as new major WGPU releases become available.
namespace slint::vulkan {

/// The Vulkan objects the renderer is rendering with.
///
/// An instance of this is handed to the callback registered with
/// [set_rendering_notifier()](slint::vulkan::set_rendering_notifier()), and refers to objects
/// owned by the renderer. They stay valid from `RenderingState::RenderingSetup` until
/// `RenderingState::RenderingTeardown`; the `Api` object itself only for the duration of the call.
class Api
{
public:
    /// \private
    explicit Api(const cbindgen_private::VulkanApi &api) : inner(api) { }

    /// Returns the instance the renderer created.
    VkInstance instance() const { return reinterpret_cast<VkInstance>(inner.instance); }

    /// Returns the physical device the renderer picked.
    VkPhysicalDevice physical_device() const
    {
        return reinterpret_cast<VkPhysicalDevice>(inner.physical_device);
    }

    /// Returns the device the renderer created.
    ///
    /// Allocate what you hand back to Slint from this device; the renderer cannot use resources
    /// belonging to another one.
    VkDevice device() const { return reinterpret_cast<VkDevice>(inner.device); }

    /// Returns the queue the renderer submits on.
    ///
    /// Submitting your own command buffers on this same queue is what orders them against the
    /// renderer's work, without needing a semaphore of your own.
    VkQueue queue() const { return reinterpret_cast<VkQueue>(inner.queue); }

    /// Returns the index of the queue family queue() belongs to.
    uint32_t queue_family_index() const { return inner.queue_family_index; }

    /// Returns `vkGetInstanceProcAddr` of the loader the renderer is using.
    ///
    /// Resolve entry points through this to be sure of talking to the same driver as the
    /// renderer, rather than to whichever loader the application itself linked against.
    PFN_vkGetInstanceProcAddr get_instance_proc_addr() const
    {
        return reinterpret_cast<PFN_vkGetInstanceProcAddr>(inner.get_instance_proc_addr);
    }

private:
    friend class Texture;
    const cbindgen_private::VulkanApi &inner;
};

/// The pixel format of a texture handed to Slint.
///
/// Narrower than what Vulkan allows: these are the ones Slint can sample from.
enum class TextureFormat {
    /// `VK_FORMAT_R8G8B8A8_UNORM`
    Rgba8Unorm,
    /// `VK_FORMAT_R8G8B8A8_SRGB`
    Rgba8UnormSrgb,
};

/// Describes the `VkImage` an application hands to Slint, see Texture::import().
struct TextureImportInfo
{
    /// The image, allocated from Api::device() and still owned by the application.
    ///
    /// Create it with `VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT` and `VK_IMAGE_USAGE_SAMPLED_BIT`,
    /// one mip level, one sample and one array layer. These have to match, and a mismatch is not
    /// diagnosed: it produces barriers that don't describe the image.
    VkImage image = VK_NULL_HANDLE;
    /// The width in pixels passed to `vkCreateImage`.
    uint32_t width = 0;
    /// The height in pixels passed to `vkCreateImage`.
    uint32_t height = 0;
    /// The format passed to `vkCreateImage`.
    TextureFormat format = TextureFormat::Rgba8UnormSrgb;
    /// Invoked once Slint is done with the image, which is when destroying it becomes safe.
    ///
    /// Not before: an Image handed to the scene outlives the frame it was set in, and the GPU may
    /// still be reading from it. The call arrives on the thread running the event loop.
    std::function<void()> on_released;
};

/// A `VkImage` the application allocated, wrapped so that Slint can show it.
///
/// Import an image once and keep the Texture for as long as the image lives; importing per frame
/// would restart Slint's tracking of it each time. Each frame, call begin_render() before
/// submitting your commands, and set to_image() on an `Image` element afterwards.
///
/// The image itself stays owned by the application. Destroying this object ends Slint's borrow,
/// but the `VkImage` may still be in flight - wait for TextureImportInfo::on_released.
class Texture
{
public:
    /// Wraps \a info's image for use with Slint.
    ///
    /// Returns an empty optional if the image can't be wrapped, which is the case when \a api
    /// doesn't come from a Vulkan-backed renderer.
    [[nodiscard]] static std::optional<Texture> import(const Api &api, TextureImportInfo info)
    {
        cbindgen_private::VulkanTextureImportInfo raw {
            .image = reinterpret_cast<uint64_t>(info.image),
            .width = info.width,
            .height = info.height,
            .format = static_cast<cbindgen_private::VulkanTextureFormat>(info.format),
            .on_released = nullptr,
            .user_data = nullptr,
        };

        if (info.on_released) {
            raw.on_released = [](void *user_data) {
                auto *callback = reinterpret_cast<std::function<void()> *>(user_data);
                (*callback)();
                delete callback;
            };
            raw.user_data = new std::function<void()>(std::move(info.on_released));
        }

        auto *inner = cbindgen_private::slint_vulkan_texture_import(&api.inner, &raw);
        if (!inner) {
            // Slint never took ownership, so the trampoline will not run.
            delete reinterpret_cast<std::function<void()> *>(raw.user_data);
            return {};
        }
        return Texture(inner);
    }

    Texture(const Texture &) = delete;
    Texture &operator=(const Texture &) = delete;
    /// Moves the borrow out of \a other, which is left empty.
    Texture(Texture &&other) : inner(other.inner) { other.inner = nullptr; }
    /// Moves the borrow out of \a other, which is left empty.
    Texture &operator=(Texture &&other)
    {
        std::swap(inner, other.inner);
        return *this;
    }
    ~Texture()
    {
        if (inner)
            cbindgen_private::slint_vulkan_texture_drop(inner);
    }

    /// Tells Slint that you are about to render into the image.
    ///
    /// Slint can't see your command buffers, so without this it still believes the image holds
    /// what it last saw, and the barrier it later emits to sample the image names a source scope
    /// that doesn't cover your writes. Call this before submitting them, every frame you render.
    void begin_render() const { cbindgen_private::slint_vulkan_texture_begin_render(inner); }

    /// Returns the image, for use as the `source` of an `Image` element.
    ///
    /// Call this after submitting the commands that render into it. Leave the image as a colour
    /// attachment: that is the state begin_render() announced, and the one Slint transitions away
    /// from when it samples the image.
    [[nodiscard]] slint::Image to_image() const
    {
        cbindgen_private::types::Image img(cbindgen_private::types::Image::ImageInner_None());
        cbindgen_private::slint_vulkan_texture_to_image(inner, &img);
        return slint::Image(img);
    }

private:
    explicit Texture(cbindgen_private::VulkanTexture *inner) : inner(inner) { }
    cbindgen_private::VulkanTexture *inner;
};

/// Registers a callback that's invoked during the different phases of rendering, with the Vulkan
/// objects the renderer is using.
///
/// The callback is called with a `slint::RenderingState` and a pointer to an Api. That pointer is
/// null whenever the renderer isn't running on Vulkan, which is worth reporting rather than
/// ignoring: it means the application's own rendering can't take place.
///
/// On success the returned std::optional has no value. On error it holds the reason.
template<std::invocable<RenderingState, const Api *> F>
inline std::optional<SetRenderingNotifierError> set_rendering_notifier(const Window &window,
                                                                       F &&callback)
{
    private_api::assert_main_thread();

    using Callback = std::decay_t<F>;

    auto actual_cb = [](RenderingState state, const cbindgen_private::VulkanApi *raw_api,
                        void *user_data) {
        auto &f = *reinterpret_cast<Callback *>(user_data);
        if (raw_api) {
            Api api(*raw_api);
            f(state, &api);
        } else {
            f(state, nullptr);
        }
    };

    SetRenderingNotifierError err;
    if (cbindgen_private::slint_windowrc_set_vulkan_rendering_notifier(
                &window.window_handle().handle(), actual_cb,
                [](void *user_data) { delete reinterpret_cast<Callback *>(user_data); },
                new Callback(std::forward<F>(callback)), &err)) {
        return {};
    }
    return err;
}

}
