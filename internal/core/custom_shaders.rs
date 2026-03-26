use wgpu_28 as wgpu;

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Renderer {
    fn new(device: &wgpu::Device, shader_code: &str, format: wgpu::TextureFormat) {

    }
}

struct Uniforms {
    resolution: [u32, 2],
    time: f32,
}
