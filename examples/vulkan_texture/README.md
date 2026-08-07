<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Vulkan Texture Import Example

This example draws with plain Vulkan into a texture and shows that texture in a Slint scene:

1. A ray marched cube is rendered with Vulkan into a texture.
2. The texture is imported into a `slint::Image` and set on an `Image` element.
3. Slint renders the scene, with the texture shown in the `Image`.

It's the same effect as the `opengl_texture` and `wgpu_texture` examples next door, so the three
can be compared side by side. There are two versions of it, `main.rs` and `main.cpp`, sharing the
scene and the shaders. They differ in who owns the texture, which is worth comparing too: the Rust
one lets wgpu allocate it, while the C++ one allocates the `VkImage` itself, the way an
application with its own allocator would.

## Where the Vulkan handles come from

The example never creates a Vulkan instance, device or queue of its own. Slint renders with Skia
on top of wgpu here, wgpu is asked for its Vulkan backend, and the example borrows the handles
wgpu already made:

```rust
let hal_device = wgpu_device.as_hal::<wgpu::wgc::api::Vulkan>()?;
let hal_queue = wgpu_queue.as_hal::<wgpu::wgc::api::Vulkan>()?;
(hal_device.raw_device().clone(), hal_queue.as_raw(), hal_device.queue_family_index())
```

`wgpu_device` and `wgpu_queue` come from `slint::GraphicsAPI::WGPU30`, which
`slint::Window::set_rendering_notifier()` hands to its callback. Sharing one device and one queue
is what lets Slint import the result without copying it, and it's why the drawing can be ordered
against Slint's own rendering with nothing but submission order.

The render target is allocated by wgpu rather than by this example, because wgpu owns the memory
and the lifetime and because `slint::Image::try_from` takes a `wgpu::Texture`. Only the `VkImage`
underneath it is borrowed, via `texture.as_hal::<Vulkan>()?.raw_handle()`.

## Handing the texture back and forth

Two things need care when raw Vulkan and wgpu share a texture.

**wgpu can't see the drawing.** wgpu tracks the state of every resource it knows about, and this
example's command buffer is invisible to that tracking. Before submitting, the example tells wgpu
what is about to happen:

```rust
encoder.transition_resources(std::iter::empty(), std::iter::once(wgpu::TextureTransition {
    texture: &target.texture,
    selector: None,
    state: wgpu::TextureUses::COLOR_TARGET,
}));
```

Without it wgpu still believes the texture is untouched, and the barrier Slint later emits to hand
the texture to Skia for sampling names a source scope that doesn't cover these writes. The Vulkan
synchronization validation layer reports that as a `WRITE_AFTER_WRITE` hazard.

**The image layout has to match on both sides.** The render pass hands the image back in
`COLOR_ATTACHMENT_OPTIMAL`, matching the `COLOR_TARGET` state wgpu was just told about. It starts
from `UNDEFINED`, because the whole image is redrawn every frame and that saves tracking whatever
layout Skia and wgpu left it in.

The render pass also spells out its dependency on `VK_SUBPASS_EXTERNAL`. Both this example's
previous frame and Skia's sampling of it ran on the same queue, and the implicit dependency starts
at `TOP_OF_PIPE`, which would order the drawing after neither.

## The C++ version

`main.cpp` builds with CMake, along with the rest of the examples:

```sh
cmake -B build -DSLINT_BUILD_EXAMPLES=ON -DSLINT_FEATURE_RENDERER_SKIA=ON
cmake --build build --target vulkan_texture
```

It uses `slint::vulkan` from `slint-vulkan.h`, which needs the Vulkan headers. Slint finds them
where you build your application, and only that one header needs them - nothing else in Slint
does, and Slint never links the Vulkan loader.

The two halves of the handover are the same as in the Rust version, just spelled differently:

```cpp
// once per image
texture = slint::vulkan::Texture::import(*api, {
    .image = image, .width = w, .height = h,
    .format = slint::vulkan::TextureFormat::Rgba8UnormSrgb,
    .on_released = [=] { vkDestroyImage(device, image, nullptr); ... },
});

// every frame
texture->begin_render();
// ... record and submit, on api->queue() ...
app->set_texture(texture->to_image());
```

`begin_render()` is what tells Slint about writes it can't see, in place of the
`transition_resources` call the Rust version makes directly. `on_released` is what makes the
image safe to destroy: an image set on the scene outlives the frame it was set in, so a resize
can't free the old one straight away. It arrives on the event loop's thread.

Import once per image and keep the `Texture`. Importing per frame would restart Slint's tracking
of the image each time, which is what the barriers depend on.

## Running it

The example asks for `wgpu::Backends::VULKAN`, so no backend selection is needed:

```sh
cargo run -p vulkan_texture
```

On macOS and iOS Vulkan runs on MoltenVK, through wgpu's `vulkan-portability` feature, which the
`slint/wgpu-30-vulkan-portability` feature in `Cargo.toml` turns on. It needs the
[Vulkan SDK](https://vulkan.lunarg.com/) installed, and the loader has to be findable: `ash` opens
a bare `libvulkan.dylib`, which dyld doesn't look for in `/usr/local/lib` on its own.

```sh
env DYLD_FALLBACK_LIBRARY_PATH=/usr/local/lib cargo run -p vulkan_texture
```

Use `env`, not an exported variable: macOS strips `DYLD_*` from the environment of protected
binaries such as the shell itself. Prefer `DYLD_FALLBACK_LIBRARY_PATH` over `DYLD_LIBRARY_PATH`:
the latter takes precedence over an executable's own rpath, so if a copy of Slint is installed in
`/usr/local/lib` it gets loaded instead of the one you just built.

## Checking it against the validation layers

```sh
env DYLD_FALLBACK_LIBRARY_PATH=/usr/local/lib \
    VK_LOADER_LAYERS_ENABLE=VK_LAYER_KHRONOS_validation \
    VK_LAYER_SETTINGS_PATH=vk_layer_settings.txt \
    cargo run -p vulkan_texture
```

with a `vk_layer_settings.txt` of:

```
khronos_validation.validate_sync = true
khronos_validation.debug_action = VK_DBG_LAYER_ACTION_LOG_MSG
khronos_validation.log_filename = stdout
khronos_validation.report_flags = error,warn,perf
```

Routing the layer's output to `stdout` matters: the example installs no `log` logger, so nothing
reaching wgpu's own debug messenger would be printed.

One error is expected and comes from wgpu itself, not from this example:
`VUID-vkAcquireNextImageKHR-fence-10066`, once per frame. wgpu 30.0.0 passes a real fence to
`vkAcquireNextImageKHR` but only waits on and resets it on Windows. It's fixed on wgpu trunk.

## Regenerating the shaders

`shader.vert.spv` and `shader.frag.spv` are checked in so that building the example doesn't need a
shader compiler. After editing the GLSL, rebuild them with `glslangValidator` from the Vulkan SDK:

```sh
glslangValidator -V shader.vert -o shader.vert.spv
glslangValidator -V shader.frag -o shader.frag.spv
```
