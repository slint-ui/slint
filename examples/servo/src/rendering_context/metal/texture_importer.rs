/// Servo texture importer that manages WGPU resources for Metal IOSurface textures.
///
/// This struct holds all the necessary WGPU rendering resources (shaders, pipeline, sampler, etc.)
/// and is tied to a specific WGPU device, avoiding the issues with global static caches.
pub struct ServoTextureImporter {
    pub sampler: wgpu::Sampler,
    pub render_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl ServoTextureImporter {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Metal Texture Flip Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(r#"
                @vertex
                fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
                    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
                    return vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
                    
                }
            "#.into()),
        });

        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Metal Texture Flip Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                @group(0) @binding(0) var source_texture: texture_2d<f32>;
                @group(0) @binding(1) var source_sampler: sampler;

                @fragment
                fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
                    let size = textureDimensions(source_texture);
                    let uv = position.xy / vec2<f32>(f32(size.x), f32(size.y));
                    // Flip vertically by inverting the V coordinate
                    let flipped_uv = vec2<f32>(uv.x, 1.0 - uv.y);
                    let color = textureSample(source_texture, source_sampler, flipped_uv);
                    return color;
                }
            "#
                .into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Metal Texture Flip Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Metal Texture Sampler"),
            compare: None,
            border_color: None,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            anisotropy_clamp: 1,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Metal Texture Flip Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Metal Texture Flip Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            bind_group_layout,
            render_pipeline,
            sampler,
        }
    }
}
