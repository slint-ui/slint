/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use super::{GLContext, Vertex};
use glow::HasContext;
use pathfinder_geometry::{rect::RectI, vector::Vector2I};
use std::{cell::RefCell, rc::Rc};

pub struct GLTexture {
    pub(crate) texture_id: <GLContext as HasContext>::Texture,
    context: Rc<glow::Context>,
    width: i32,
    height: i32,
}

impl PartialEq for GLTexture {
    fn eq(&self, other: &Self) -> bool {
        self.texture_id == other.texture_id && Rc::ptr_eq(&self.context, &other.context)
    }
}

pub trait UploadableAtlasImage {
    fn upload(&self, context: &Rc<glow::Context>, x: i32, y: i32);
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

#[cfg(target_arch = "wasm32")]
impl UploadableAtlasImage for &web_sys::HtmlImageElement {
    fn upload(&self, context: &Rc<GLContext>, x: i32, y: i32) {
        unsafe {
            context.tex_sub_image_2d_with_html_image(
                glow::TEXTURE_2D,
                0,
                x,
                y,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                self,
            );
        }
    }
    fn width(&self) -> u32 {
        (self as Self).width()
    }
    fn height(&self) -> u32 {
        (self as Self).height()
    }
}

impl<Container: core::ops::Deref<Target = [u8]>> UploadableAtlasImage
    for image::ImageBuffer<image::Rgba<u8>, Container>
{
    fn upload(&self, context: &Rc<GLContext>, x: i32, y: i32) {
        unsafe {
            context.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                x,
                y,
                self.width() as i32,
                self.height() as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(&self.as_raw()),
            );
        }
    }
    fn width(&self) -> u32 {
        self.width()
    }
    fn height(&self) -> u32 {
        self.height()
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

    fn set_sub_image(&self, x: i32, y: i32, image: impl UploadableAtlasImage) {
        unsafe {
            self.context.bind_texture(glow::TEXTURE_2D, Some(self.texture_id));
        }
        image.upload(&self.context, x, y);
    }

    pub fn bind_to_location(
        &self,
        texture_location: &<glow::Context as glow::HasContext>::UniformLocation,
    ) {
        unsafe {
            self.context.active_texture(glow::TEXTURE0);
            self.context.bind_texture(glow::TEXTURE_2D, Some(self.texture_id));
            self.context.uniform_1_i32(Some(&texture_location), 0);
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

pub(crate) struct GLAtlasTexture {
    pub(crate) texture: Rc<GLTexture>,
    allocator: RefCell<guillotiere::AtlasAllocator>,
}

pub struct AtlasAllocation {
    pub texture_coordinates: RectI,
    allocation_id: guillotiere::AllocId,
    pub(crate) atlas: Rc<GLAtlasTexture>,
}

impl Drop for AtlasAllocation {
    fn drop(&mut self) {
        self.atlas.allocator.borrow_mut().deallocate(self.allocation_id)
    }
}

impl AtlasAllocation {
    pub(crate) fn normalized_texture_coordinates(&self) -> [Vertex; 6] {
        let atlas_width = self.atlas.texture.width as f32;
        let atlas_height = self.atlas.texture.height as f32;
        let origin = self.texture_coordinates.origin();
        let size = self.texture_coordinates.size();
        let texture_coordinates = RectI::new(origin, size);

        let tex_left = ((texture_coordinates.min_x() as f32) + 0.5) / atlas_width;
        let tex_top = ((texture_coordinates.min_y() as f32) + 0.5) / atlas_height;
        let tex_right = ((texture_coordinates.max_x() as f32) - 0.5) / atlas_width;
        let tex_bottom = ((texture_coordinates.max_y() as f32) - 0.5) / atlas_height;

        let tex_vertex1 = Vertex { _pos: [tex_left, tex_top] };
        let tex_vertex2 = Vertex { _pos: [tex_right, tex_top] };
        let tex_vertex3 = Vertex { _pos: [tex_right, tex_bottom] };
        let tex_vertex4 = Vertex { _pos: [tex_left, tex_bottom] };

        [tex_vertex1, tex_vertex2, tex_vertex3, tex_vertex1, tex_vertex3, tex_vertex4]
    }
}

impl GLAtlasTexture {
    fn new(gl: &Rc<glow::Context>, width: u32, height: u32) -> Self {
        let allocator =
            guillotiere::AtlasAllocator::new(guillotiere::Size::new(width as _, height as _));
        let texture = Rc::new(GLTexture::new_with_size_and_data(
            gl,
            allocator.size().width,
            allocator.size().height,
            None,
        ));
        Self { texture, allocator: RefCell::new(allocator) }
    }

    fn allocate(
        self: Rc<Self>,
        requested_width: u32,
        requested_height: u32,
    ) -> Option<AtlasAllocation> {
        self.allocator
            .borrow_mut()
            .allocate(guillotiere::Size::new(requested_width as _, requested_height as _))
            .map(|guillotiere_alloc| {
                let min = guillotiere_alloc.rectangle.min;
                let size = guillotiere_alloc.rectangle.max - guillotiere_alloc.rectangle.min;
                let origin = Vector2I::new(min.x, min.y);
                let size = Vector2I::new(size.x, size.y);
                let texture_coordinates = RectI::new(origin, size);

                AtlasAllocation {
                    texture_coordinates,
                    allocation_id: guillotiere_alloc.id,
                    atlas: self.clone(),
                }
            })
    }
}

pub struct TextureAtlas {
    atlases: Vec<Rc<GLAtlasTexture>>,
}

impl TextureAtlas {
    pub fn new() -> Self {
        Self { atlases: vec![] }
    }

    pub fn allocate_region(
        &mut self,
        gl: &Rc<glow::Context>,
        requested_width: u32,
        requested_height: u32,
    ) -> AtlasAllocation {
        self.atlases
            .iter()
            .find_map(|atlas| atlas.clone().allocate(requested_width, requested_height))
            .unwrap_or_else(|| {
                let new_atlas = Rc::new(GLAtlasTexture::new(
                    &gl,
                    2048.max(requested_width),
                    2048.max(requested_height),
                ));
                let atlas_allocation =
                    new_atlas.clone().allocate(requested_width, requested_height).unwrap();
                self.atlases.push(new_atlas);
                atlas_allocation
            })
    }

    pub fn allocate_image_in_atlas(
        &mut self,
        gl: &Rc<glow::Context>,
        image: impl UploadableAtlasImage,
    ) -> AtlasAllocation {
        let allocation = self.allocate_region(gl, image.width(), image.height());

        allocation.atlas.texture.set_sub_image(
            allocation.texture_coordinates.origin_x(),
            allocation.texture_coordinates.origin_y(),
            image,
        );

        allocation
    }
}
