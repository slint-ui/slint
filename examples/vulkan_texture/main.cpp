// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Renders into a texture with plain Vulkan and shows it in a Slint scene.
//
// The C++ counterpart of main.rs next to it, drawing the same scene. The difference is who owns
// the texture: this one allocates the VkImage itself, the way an application with its own
// allocator would, and hands it to Slint with slint::vulkan::Texture::import().

#include "scene.h"

#include <slint-vulkan.h>

#include <chrono>
#include <cstdio>
#include <fstream>
#include <optional>
#include <vector>

// The one format Vulkan, Slint and the renderer all have to agree on.
static constexpr VkFormat vulkan_format = VK_FORMAT_R8G8B8A8_SRGB;
static constexpr auto slint_format = slint::vulkan::TextureFormat::Rgba8UnormSrgb;

// What the fragment shader reads out of the push constant block.
struct PushConstants
{
    float light_color_and_time[4];
};

static std::vector<uint32_t> read_spirv(const char *name)
{
    std::string path = std::string(SHADER_DIR) + "/" + name;
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    if (!file) {
        std::fprintf(stderr, "Cannot open %s\n", path.c_str());
        return {};
    }
    auto size = static_cast<size_t>(file.tellg());
    std::vector<uint32_t> code(size / sizeof(uint32_t));
    file.seekg(0);
    file.read(reinterpret_cast<char *>(code.data()), static_cast<std::streamsize>(size));
    return code;
}

static std::optional<uint32_t> find_memory_type(VkPhysicalDevice physical_device,
                                                uint32_t supported_types,
                                                VkMemoryPropertyFlags properties)
{
    VkPhysicalDeviceMemoryProperties memory_properties {};
    vkGetPhysicalDeviceMemoryProperties(physical_device, &memory_properties);
    for (uint32_t i = 0; i < memory_properties.memoryTypeCount; ++i) {
        const bool supported = (supported_types & (1u << i)) != 0;
        const auto flags = memory_properties.memoryTypes[i].propertyFlags;
        if (supported && (flags & properties) == properties)
            return i;
    }
    return {};
}

// The render target: the image this example allocates, the Vulkan objects that only make sense
// for that one image, and Slint's borrow of it.
struct Target
{
    uint32_t width = 0;
    uint32_t height = 0;
    std::optional<slint::vulkan::Texture> texture;

    // Builds an image, hands it to Slint, and leaves destroying it to the callback Slint invokes
    // once it is done with it. Nothing here is destroyed eagerly: an image set on the scene
    // outlives the frame, and the GPU may still be reading it.
    static std::optional<Target> create(const slint::vulkan::Api &api, VkRenderPass render_pass,
                                        uint32_t width, uint32_t height)
    {
        VkDevice device = api.device();

        VkImageCreateInfo image_info { .sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO };
        image_info.imageType = VK_IMAGE_TYPE_2D;
        image_info.format = vulkan_format;
        image_info.extent = { width, height, 1 };
        image_info.mipLevels = 1;
        image_info.arrayLayers = 1;
        image_info.samples = VK_SAMPLE_COUNT_1_BIT;
        image_info.tiling = VK_IMAGE_TILING_OPTIMAL;
        // Slint samples the image, this example renders into it. Both usages have to be here, and
        // have to match what slint::vulkan::TextureImportInfo is documented to expect.
        image_info.usage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_SAMPLED_BIT;
        image_info.sharingMode = VK_SHARING_MODE_EXCLUSIVE;
        image_info.initialLayout = VK_IMAGE_LAYOUT_UNDEFINED;

        VkImage image = VK_NULL_HANDLE;
        if (vkCreateImage(device, &image_info, nullptr, &image) != VK_SUCCESS)
            return {};

        VkMemoryRequirements requirements {};
        vkGetImageMemoryRequirements(device, image, &requirements);
        auto memory_type = find_memory_type(api.physical_device(), requirements.memoryTypeBits,
                                            VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
        if (!memory_type) {
            vkDestroyImage(device, image, nullptr);
            return {};
        }

        VkMemoryAllocateInfo allocate_info { .sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO };
        allocate_info.allocationSize = requirements.size;
        allocate_info.memoryTypeIndex = *memory_type;

        VkDeviceMemory memory = VK_NULL_HANDLE;
        if (vkAllocateMemory(device, &allocate_info, nullptr, &memory) != VK_SUCCESS) {
            vkDestroyImage(device, image, nullptr);
            return {};
        }
        vkBindImageMemory(device, image, memory, 0);

        VkImageViewCreateInfo view_info { .sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO };
        view_info.image = image;
        view_info.viewType = VK_IMAGE_VIEW_TYPE_2D;
        view_info.format = vulkan_format;
        view_info.subresourceRange = { VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1 };

        VkImageView view = VK_NULL_HANDLE;
        vkCreateImageView(device, &view_info, nullptr, &view);

        VkFramebufferCreateInfo framebuffer_info {
            .sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO
        };
        framebuffer_info.renderPass = render_pass;
        framebuffer_info.attachmentCount = 1;
        framebuffer_info.pAttachments = &view;
        framebuffer_info.width = width;
        framebuffer_info.height = height;
        framebuffer_info.layers = 1;

        VkFramebuffer framebuffer = VK_NULL_HANDLE;
        vkCreateFramebuffer(device, &framebuffer_info, nullptr, &framebuffer);

        Target target;
        target.width = width;
        target.height = height;
        target.image = image;
        target.framebuffer = framebuffer;
        target.texture = slint::vulkan::Texture::import(
                api,
                { .image = image,
                  .width = width,
                  .height = height,
                  .format = slint_format,
                  // Slint calls this once the image is out of use, on the event loop's thread.
                  .on_released = [device, image, memory, view, framebuffer] {
                      vkDestroyFramebuffer(device, framebuffer, nullptr);
                      vkDestroyImageView(device, view, nullptr);
                      vkDestroyImage(device, image, nullptr);
                      vkFreeMemory(device, memory, nullptr);
                  } });

        if (!target.texture) {
            vkDestroyFramebuffer(device, framebuffer, nullptr);
            vkDestroyImageView(device, view, nullptr);
            vkDestroyImage(device, image, nullptr);
            vkFreeMemory(device, memory, nullptr);
            return {};
        }

        return target;
    }

    VkImage image = VK_NULL_HANDLE;
    VkFramebuffer framebuffer = VK_NULL_HANDLE;
};

class VulkanRenderer
{
public:
    explicit VulkanRenderer(const slint::vulkan::Api &api)
        : device(api.device()), queue(api.queue()), start_time(std::chrono::steady_clock::now())
    {
        create_render_pass();
        create_pipeline();

        VkCommandPoolCreateInfo pool_info { .sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO };
        pool_info.queueFamilyIndex = api.queue_family_index();
        pool_info.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
        vkCreateCommandPool(device, &pool_info, nullptr, &command_pool);

        VkCommandBufferAllocateInfo buffer_info {
            .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO
        };
        buffer_info.commandPool = command_pool;
        buffer_info.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
        buffer_info.commandBufferCount = 1;
        vkAllocateCommandBuffers(device, &buffer_info, &command_buffer);

        VkFenceCreateInfo fence_info { .sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO };
        vkCreateFence(device, &fence_info, nullptr, &in_flight);
    }

    ~VulkanRenderer()
    {
        // Everything below may still be in use by the queue.
        vkDeviceWaitIdle(device);

        // Dropping the texture ends Slint's borrow; the image itself is destroyed by the callback
        // registered in Target::create, once Slint is really done with it.
        target.reset();

        vkDestroyFence(device, in_flight, nullptr);
        vkDestroyCommandPool(device, command_pool, nullptr);
        vkDestroyPipeline(device, pipeline, nullptr);
        vkDestroyPipelineLayout(device, pipeline_layout, nullptr);
        vkDestroyRenderPass(device, render_pass, nullptr);
    }

    VulkanRenderer(const VulkanRenderer &) = delete;
    VulkanRenderer &operator=(const VulkanRenderer &) = delete;

    std::optional<slint::Image> render(const slint::vulkan::Api &api, float light_color[3],
                                       uint32_t width, uint32_t height)
    {
        width = std::max(width, 1u);
        height = std::max(height, 1u);

        // Re-recording the command buffer, and retiring a target, both need the previous frame to
        // be off the GPU.
        if (submitted)
            vkWaitForFences(device, 1, &in_flight, VK_TRUE, UINT64_MAX);

        if (!target || target->width != width || target->height != height) {
            target = Target::create(api, render_pass, width, height);
            if (!target)
                return {};
        }

        // Slint cannot see the command buffer below, so tell it the image is about to be written
        // as a colour attachment. Without this the barrier Slint later emits to sample the image
        // names a source scope that doesn't cover these writes.
        target->texture->begin_render();

        vkResetFences(device, 1, &in_flight);
        vkResetCommandBuffer(command_buffer, 0);

        VkCommandBufferBeginInfo begin_info { .sType =
                                                      VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO };
        begin_info.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
        vkBeginCommandBuffer(command_buffer, &begin_info);

        VkClearValue clear {};
        clear.color = { { 0.f, 0.f, 0.f, 1.f } };

        VkRenderPassBeginInfo pass_info { .sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO };
        pass_info.renderPass = render_pass;
        pass_info.framebuffer = target->framebuffer;
        pass_info.renderArea.extent = { width, height };
        pass_info.clearValueCount = 1;
        pass_info.pClearValues = &clear;
        vkCmdBeginRenderPass(command_buffer, &pass_info, VK_SUBPASS_CONTENTS_INLINE);

        VkViewport viewport { 0.f, 0.f, float(width), float(height), 0.f, 1.f };
        vkCmdSetViewport(command_buffer, 0, 1, &viewport);
        VkRect2D scissor { { 0, 0 }, { width, height } };
        vkCmdSetScissor(command_buffer, 0, 1, &scissor);
        vkCmdBindPipeline(command_buffer, VK_PIPELINE_BIND_POINT_GRAPHICS, pipeline);

        const float elapsed =
                std::chrono::duration<float>(std::chrono::steady_clock::now() - start_time).count()
                * 2.f;
        PushConstants push_constants { { light_color[0], light_color[1], light_color[2],
                                         elapsed } };
        vkCmdPushConstants(command_buffer, pipeline_layout, VK_SHADER_STAGE_FRAGMENT_BIT, 0,
                           sizeof(push_constants), &push_constants);

        vkCmdDraw(command_buffer, 3, 1, 0, 0);
        vkCmdEndRenderPass(command_buffer);
        vkEndCommandBuffer(command_buffer);

        // The same queue Slint submits on, so submission order alone puts this drawing ahead of
        // the work that samples the texture. No semaphores needed.
        VkSubmitInfo submit_info { .sType = VK_STRUCTURE_TYPE_SUBMIT_INFO };
        submit_info.commandBufferCount = 1;
        submit_info.pCommandBuffers = &command_buffer;
        vkQueueSubmit(queue, 1, &submit_info, in_flight);
        submitted = true;

        return target->texture->to_image();
    }

private:
    // A single-attachment render pass that hands the image back in the layout Slint's tracking of
    // it expects, which is what begin_render() announced.
    void create_render_pass()
    {
        VkAttachmentDescription attachment {};
        attachment.format = vulkan_format;
        attachment.samples = VK_SAMPLE_COUNT_1_BIT;
        attachment.loadOp = VK_ATTACHMENT_LOAD_OP_CLEAR;
        attachment.storeOp = VK_ATTACHMENT_STORE_OP_STORE;
        attachment.stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE;
        attachment.stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE;
        // The whole image is redrawn every frame, so there is nothing to preserve. Starting from
        // UNDEFINED also saves tracking whatever layout Slint left it in.
        attachment.initialLayout = VK_IMAGE_LAYOUT_UNDEFINED;
        attachment.finalLayout = VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL;

        VkAttachmentReference color_reference { 0, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL };

        VkSubpassDescription subpass {};
        subpass.pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS;
        subpass.colorAttachmentCount = 1;
        subpass.pColorAttachments = &color_reference;

        // Two things touched this image before we get here, both on this same queue: our own
        // render pass last frame, and Slint sampling the result. The UNDEFINED initial layout
        // counts as a write, so the dependency has to cover both. The implicit external
        // dependency starts at TOP_OF_PIPE and would order us after neither.
        VkSubpassDependency dependency {};
        dependency.srcSubpass = VK_SUBPASS_EXTERNAL;
        dependency.dstSubpass = 0;
        dependency.srcStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT
                | VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT;
        dependency.srcAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT | VK_ACCESS_SHADER_READ_BIT;
        dependency.dstStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
        dependency.dstAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;

        VkRenderPassCreateInfo info { .sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO };
        info.attachmentCount = 1;
        info.pAttachments = &attachment;
        info.subpassCount = 1;
        info.pSubpasses = &subpass;
        info.dependencyCount = 1;
        info.pDependencies = &dependency;
        vkCreateRenderPass(device, &info, nullptr, &render_pass);
    }

    VkShaderModule create_shader_module(const char *name)
    {
        auto code = read_spirv(name);
        VkShaderModuleCreateInfo info { .sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO };
        info.codeSize = code.size() * sizeof(uint32_t);
        info.pCode = code.data();
        VkShaderModule module = VK_NULL_HANDLE;
        vkCreateShaderModule(device, &info, nullptr, &module);
        return module;
    }

    // The pipeline for the one draw call this example makes: a full-target triangle with the ray
    // marcher in its fragment shader.
    void create_pipeline()
    {
        VkShaderModule vertex_module = create_shader_module("shader.vert.spv");
        VkShaderModule fragment_module = create_shader_module("shader.frag.spv");

        VkPushConstantRange push_constant_range { VK_SHADER_STAGE_FRAGMENT_BIT, 0,
                                                  sizeof(PushConstants) };

        VkPipelineLayoutCreateInfo layout_info {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO
        };
        layout_info.pushConstantRangeCount = 1;
        layout_info.pPushConstantRanges = &push_constant_range;
        vkCreatePipelineLayout(device, &layout_info, nullptr, &pipeline_layout);

        VkPipelineShaderStageCreateInfo stages[2] {};
        stages[0].sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO;
        stages[0].stage = VK_SHADER_STAGE_VERTEX_BIT;
        stages[0].module = vertex_module;
        stages[0].pName = "main";
        stages[1].sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO;
        stages[1].stage = VK_SHADER_STAGE_FRAGMENT_BIT;
        stages[1].module = fragment_module;
        stages[1].pName = "main";

        VkPipelineVertexInputStateCreateInfo vertex_input {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO
        };
        VkPipelineInputAssemblyStateCreateInfo input_assembly {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO
        };
        input_assembly.topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST;

        // The target is resized with the window, so both are set at record time instead of
        // rebuilding the pipeline for every size.
        VkDynamicState dynamic_states[] = { VK_DYNAMIC_STATE_VIEWPORT, VK_DYNAMIC_STATE_SCISSOR };
        VkPipelineDynamicStateCreateInfo dynamic_state {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO
        };
        dynamic_state.dynamicStateCount = 2;
        dynamic_state.pDynamicStates = dynamic_states;

        VkPipelineViewportStateCreateInfo viewport_state {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO
        };
        viewport_state.viewportCount = 1;
        viewport_state.scissorCount = 1;

        VkPipelineRasterizationStateCreateInfo rasterization {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO
        };
        rasterization.polygonMode = VK_POLYGON_MODE_FILL;
        rasterization.cullMode = VK_CULL_MODE_NONE;
        rasterization.frontFace = VK_FRONT_FACE_COUNTER_CLOCKWISE;
        rasterization.lineWidth = 1.f;

        VkPipelineMultisampleStateCreateInfo multisample {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO
        };
        multisample.rasterizationSamples = VK_SAMPLE_COUNT_1_BIT;

        VkPipelineColorBlendAttachmentState blend_attachment {};
        blend_attachment.colorWriteMask = VK_COLOR_COMPONENT_R_BIT | VK_COLOR_COMPONENT_G_BIT
                | VK_COLOR_COMPONENT_B_BIT | VK_COLOR_COMPONENT_A_BIT;

        VkPipelineColorBlendStateCreateInfo color_blend {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO
        };
        color_blend.attachmentCount = 1;
        color_blend.pAttachments = &blend_attachment;

        VkGraphicsPipelineCreateInfo info {
            .sType = VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO
        };
        info.stageCount = 2;
        info.pStages = stages;
        info.pVertexInputState = &vertex_input;
        info.pInputAssemblyState = &input_assembly;
        info.pViewportState = &viewport_state;
        info.pRasterizationState = &rasterization;
        info.pMultisampleState = &multisample;
        info.pColorBlendState = &color_blend;
        info.pDynamicState = &dynamic_state;
        info.layout = pipeline_layout;
        info.renderPass = render_pass;
        info.subpass = 0;
        vkCreateGraphicsPipelines(device, VK_NULL_HANDLE, 1, &info, nullptr, &pipeline);

        // The modules are only needed while the pipeline is being built.
        vkDestroyShaderModule(device, vertex_module, nullptr);
        vkDestroyShaderModule(device, fragment_module, nullptr);
    }

    VkDevice device;
    VkQueue queue;
    VkRenderPass render_pass = VK_NULL_HANDLE;
    VkPipelineLayout pipeline_layout = VK_NULL_HANDLE;
    VkPipeline pipeline = VK_NULL_HANDLE;
    VkCommandPool command_pool = VK_NULL_HANDLE;
    VkCommandBuffer command_buffer = VK_NULL_HANDLE;
    // Signalled once command_buffer has run, so we don't re-record it while it's in flight.
    VkFence in_flight = VK_NULL_HANDLE;
    bool submitted = false;
    std::optional<Target> target;
    std::chrono::steady_clock::time_point start_time;
};

int main()
{
    auto app = App::create();

    auto weak = slint::ComponentWeakHandle(app);
    auto error = slint::vulkan::set_rendering_notifier(
            app->window(),
            [weak, renderer = std::unique_ptr<VulkanRenderer>()](
                    slint::RenderingState state, const slint::vulkan::Api *api) mutable {
                if (!api) {
                    if (state == slint::RenderingState::RenderingSetup)
                        std::fprintf(stderr,
                                     "This example needs the renderer to be running on Vulkan.\n");
                    return;
                }

                switch (state) {
                case slint::RenderingState::RenderingSetup:
                    renderer = std::make_unique<VulkanRenderer>(*api);
                    break;
                case slint::RenderingState::BeforeRendering:
                    if (auto app = weak.lock(); app && renderer) {
                        float light_color[3] = { (*app)->get_selected_red(),
                                                 (*app)->get_selected_green(),
                                                 (*app)->get_selected_blue() };
                        if (auto image = renderer->render(
                                    *api, light_color,
                                    uint32_t((*app)->get_requested_texture_width()),
                                    uint32_t((*app)->get_requested_texture_height()))) {
                            (*app)->set_texture(*image);
                        }
                        // The effect animates, so keep frames coming.
                        (*app)->window().request_redraw();
                    }
                    break;
                case slint::RenderingState::RenderingTeardown:
                    renderer.reset();
                    break;
                default:
                    break;
                }
            });

    if (error) {
        std::fprintf(stderr, "Cannot install a rendering notifier on this renderer.\n");
        return 1;
    }

    app->run();
}
