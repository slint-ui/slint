use super::{GLContext, Vertex};
use glow::HasContext;
use pathfinder_geometry::{rect::RectI, vector::Vector2I};

#[derive(Copy, Clone, PartialEq)]
pub struct GLTexture {
    texture_id: <GLContext as HasContext>::Texture,
}

impl GLTexture {
    fn new_with_size_and_data(
        gl: &glow::Context,
        width: i32,
        height: i32,
        data: Option<&[u8]>,
    ) -> Self {
        let texture_id = unsafe { gl.create_texture().unwrap() };

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_id));

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);

            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width,
                height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                data,
            )
        }

        Self { texture_id }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new_from_canvas(gl: &glow::Context, canvas: &web_sys::HtmlCanvasElement) -> Self {
        let texture_id = unsafe { gl.create_texture().unwrap() };

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_id));

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);

            gl.tex_image_2d_with_html_canvas(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                canvas,
            )
        }

        Self { texture_id }
    }

    fn set_sub_image(
        &mut self,
        gl: &glow::Context,
        x: i32,
        y: i32,
        image: image::ImageBuffer<image::Rgba<u8>, &[u8]>,
    ) {
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture_id));
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                x,
                y,
                image.width() as i32,
                image.height() as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(&image.into_raw()),
            );
        }
    }

    pub fn bind_to_location(
        &self,
        gl: &glow::Context,
        texture_location: &<glow::Context as glow::HasContext>::UniformLocation,
    ) {
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture_id));
            gl.uniform_1_i32(Some(&texture_location), 0);
        }
    }

    // TODO #3: make sure we release GL resources
    /*
    fn drop(&mut self, gl: &glow::Context) {
        unsafe {
            gl.delete_texture(self.texture_id);
        }
    }
    */
}

pub(crate) struct AtlasTextureAllocation {
    pub texture: GLTexture,
    pub texture_coordinates: RectI,
    pub normalized_coordinates: [Vertex; 6],
}

impl AtlasTextureAllocation {
    fn new(
        texture: GLTexture,
        allocator: &guillotiere::AtlasAllocator,
        allocation: guillotiere::Allocation,
    ) -> Self {
        let atlas_width = allocator.size().width as f32;
        let atlas_height = allocator.size().height as f32;
        let min = allocation.rectangle.min;
        let size = allocation.rectangle.max - allocation.rectangle.min;
        let origin = Vector2I::new(min.x, min.y);
        let size = Vector2I::new(size.x, size.y);
        let texture_coordinates = RectI::new(origin, size);

        let tex_left = (texture_coordinates.min_x() as f32) / atlas_width;
        let tex_top = (texture_coordinates.min_y() as f32) / atlas_height;
        let tex_right = (texture_coordinates.max_x() as f32) / atlas_width;
        let tex_bottom = (texture_coordinates.max_y() as f32) / atlas_height;

        let tex_vertex1 = Vertex { _pos: [tex_left, tex_top] };
        let tex_vertex2 = Vertex { _pos: [tex_right, tex_top] };
        let tex_vertex3 = Vertex { _pos: [tex_right, tex_bottom] };
        let tex_vertex4 = Vertex { _pos: [tex_left, tex_bottom] };

        AtlasTextureAllocation {
            texture,
            texture_coordinates,
            normalized_coordinates: [
                tex_vertex1,
                tex_vertex2,
                tex_vertex3,
                tex_vertex1,
                tex_vertex3,
                tex_vertex4,
            ],
        }
    }
}

struct GLAtlasTexture {
    index_in_atlases: usize,
    texture: GLTexture,
    allocator: guillotiere::AtlasAllocator,
}

pub struct AtlasAllocation {
    atlas_index: usize,
    pub(crate) sub_texture: AtlasTextureAllocation,
}

impl GLAtlasTexture {
    fn new(gl: &glow::Context, index_in_atlases: usize) -> Self {
        let allocator = guillotiere::AtlasAllocator::new(guillotiere::Size::new(2048, 2048));
        let texture = GLTexture::new_with_size_and_data(
            gl,
            allocator.size().width,
            allocator.size().height,
            None,
        );
        Self { index_in_atlases, texture, allocator }
    }

    fn allocate(&mut self, requested_width: i32, requested_height: i32) -> Option<AtlasAllocation> {
        self.allocator.allocate(guillotiere::Size::new(requested_width, requested_height)).map(
            |guillotiere_alloc| AtlasAllocation {
                atlas_index: self.index_in_atlases,
                sub_texture: AtlasTextureAllocation::new(
                    self.texture,
                    &self.allocator,
                    guillotiere_alloc,
                ),
            },
        )
    }
}

pub struct TextureAtlas {
    atlases: Vec<GLAtlasTexture>,
}

impl TextureAtlas {
    pub fn new() -> Self {
        Self { atlases: vec![] }
    }

    fn allocate_region(
        &mut self,
        gl: &glow::Context,
        requested_width: i32,
        requested_height: i32,
    ) -> AtlasAllocation {
        self.atlases
            .iter_mut()
            .find_map(|atlas| atlas.allocate(requested_width, requested_height))
            .unwrap_or_else(|| {
                let atlas_index = self.atlases.len();
                let mut new_atlas = GLAtlasTexture::new(&gl, atlas_index);
                let atlas_allocation =
                    new_atlas.allocate(requested_width, requested_height).unwrap();
                self.atlases.push(new_atlas);
                atlas_allocation
            })
    }

    pub fn allocate_image_in_atlas(
        &mut self,
        gl: &glow::Context,
        image: image::ImageBuffer<image::Rgba<u8>, &[u8]>,
    ) -> AtlasAllocation {
        let requested_width = image.width() as i32;
        let requested_height = image.height() as i32;

        let allocation = self.allocate_region(gl, requested_width, requested_height);

        let texture = &mut self.atlases[allocation.atlas_index].texture;
        texture.set_sub_image(
            gl,
            allocation.sub_texture.texture_coordinates.min_x(),
            allocation.sub_texture.texture_coordinates.min_y(),
            image,
        );

        allocation
    }
}
