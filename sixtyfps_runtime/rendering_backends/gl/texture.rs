use super::{GLContext, Vertex};
use glow::HasContext;
use pathfinder_geometry::{rect::RectI, vector::Vector2I};
use std::rc::Rc;

pub struct GLTexture {
    texture_id: <GLContext as HasContext>::Texture,
    context: Rc<glow::Context>,
    width: i32,
    height: i32,
}

impl PartialEq for GLTexture {
    fn eq(&self, other: &Self) -> bool {
        self.texture_id == other.texture_id && Rc::ptr_eq(&self.context, &other.context)
    }
}

impl GLTexture {
    fn new_with_size_and_data(
        gl: &Rc<glow::Context>,
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

        Self { texture_id, context: gl.clone(), width, height }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new_from_canvas(gl: &Rc<glow::Context>, canvas: &web_sys::HtmlCanvasElement) -> Self {
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

        Self {
            texture_id,
            context: gl.clone(),
            width: canvas.width() as _,
            height: canvas.height() as _,
        }
    }

    fn set_sub_image<Container: core::ops::Deref<Target = [u8]>>(
        &self,
        gl: &glow::Context,
        x: i32,
        y: i32,
        image: image::ImageBuffer<image::Rgba<u8>, Container>,
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
}

impl Drop for GLTexture {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_texture(self.texture_id);
        }
    }
}

pub(crate) struct AtlasTextureAllocation {
    pub texture: Rc<GLTexture>,
    pub texture_coordinates: RectI, // excludes padding
}

impl AtlasTextureAllocation {
    fn new(texture: Rc<GLTexture>, allocation: guillotiere::Allocation) -> Self {
        let min = allocation.rectangle.min;
        let size = allocation.rectangle.max - allocation.rectangle.min;
        let origin = Vector2I::new(min.x, min.y);
        let size = Vector2I::new(size.x, size.y);
        let texture_coordinates = RectI::new(origin, size);

        AtlasTextureAllocation { texture, texture_coordinates }
    }

    pub fn normalized_texture_coordinates(&self) -> [Vertex; 6] {
        let atlas_width = self.texture.width as f32;
        let atlas_height = self.texture.height as f32;
        let origin = self.texture_coordinates.origin();
        let size = self.texture_coordinates.size();
        let texture_coordinates = RectI::new(origin, size);

        let tex_left = (texture_coordinates.min_x() as f32) / atlas_width;
        let tex_top = (texture_coordinates.min_y() as f32) / atlas_height;
        let tex_right = (texture_coordinates.max_x() as f32) / atlas_width;
        let tex_bottom = (texture_coordinates.max_y() as f32) / atlas_height;

        let tex_vertex1 = Vertex { _pos: [tex_left, tex_top] };
        let tex_vertex2 = Vertex { _pos: [tex_right, tex_top] };
        let tex_vertex3 = Vertex { _pos: [tex_right, tex_bottom] };
        let tex_vertex4 = Vertex { _pos: [tex_left, tex_bottom] };

        [tex_vertex1, tex_vertex2, tex_vertex3, tex_vertex1, tex_vertex3, tex_vertex4]
    }
}

struct GLAtlasTexture {
    index_in_atlases: usize,
    texture: Rc<GLTexture>,
    allocator: guillotiere::AtlasAllocator,
}

pub struct AtlasAllocation {
    atlas_index: usize,
    pub(crate) sub_texture: AtlasTextureAllocation,
}

impl GLAtlasTexture {
    fn new(gl: &Rc<glow::Context>, index_in_atlases: usize) -> Self {
        let allocator = guillotiere::AtlasAllocator::new(guillotiere::Size::new(2048, 2048));
        let texture = Rc::new(GLTexture::new_with_size_and_data(
            gl,
            allocator.size().width,
            allocator.size().height,
            None,
        ));
        Self { index_in_atlases, texture, allocator }
    }

    fn allocate(&mut self, requested_width: i32, requested_height: i32) -> Option<AtlasAllocation> {
        self.allocator.allocate(guillotiere::Size::new(requested_width, requested_height)).map(
            |guillotiere_alloc| AtlasAllocation {
                atlas_index: self.index_in_atlases,
                sub_texture: AtlasTextureAllocation::new(self.texture.clone(), guillotiere_alloc),
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
        gl: &Rc<glow::Context>,
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
        gl: &Rc<glow::Context>,
        image: image::ImageBuffer<image::Rgba<u8>, &[u8]>,
    ) -> AtlasAllocation {
        use image::GenericImage;
        use image::GenericImageView;

        // To avoid pixels leaking from adjacent textures in the atlas when scaling, add a one-pixel
        // padding.

        let requested_width = image.width() + 2;
        let requested_height = image.height() + 2;

        let mut padded_image = image::ImageBuffer::new(requested_width, requested_height);

        let mut blit = |target_x, target_y, source_x, source_y, source_width, source_height| {
            padded_image
                .copy_from(
                    &image.view(source_x, source_y, source_width, source_height),
                    target_x,
                    target_y,
                )
                .ok()
                .unwrap();
        };

        // First the main image itself
        blit(1, 1, 0, 0, image.width(), image.height());

        // duplicate the top edge
        blit(1, 0, 0, 0, image.width(), 1);

        // duplicate the bottom edge
        blit(1, requested_height - 1, 0, image.height() - 1, image.width(), 1);

        // duplicate the left edge
        blit(0, 1, 0, 0, 1, image.height());

        // duplicate the right edge
        blit(requested_width - 1, 1, image.width() - 1, 0, 1, image.height());

        // duplicate the top-left corner pixel
        blit(0, 0, 0, 0, 1, 1);

        // duplicate the bottom-left corner pixel
        blit(0, requested_height - 1, 0, image.height() - 1, 1, 1);

        // duplicate the top-right corner pixel
        blit(requested_width - 1, 0, image.width() - 1, 0, 1, 1);

        // duplicate the bottom-right corner pixel
        blit(
            requested_width - 1,
            requested_height - 1,
            image.width() - 1,
            image.height() - 1,
            1,
            1,
        );

        let mut allocation = self.allocate_region(gl, requested_width as _, requested_height as _);

        let texture = &mut self.atlases[allocation.atlas_index].texture;
        texture.set_sub_image(
            gl,
            allocation.sub_texture.texture_coordinates.origin_x(),
            allocation.sub_texture.texture_coordinates.origin_y(),
            padded_image,
        );

        // Remove the padding from the coordinates we use for sampling
        allocation.sub_texture.texture_coordinates =
            allocation.sub_texture.texture_coordinates.contract(Vector2I::new(1, 1));

        allocation
    }
}
