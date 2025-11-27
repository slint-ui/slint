// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![deny(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use embedder_traits::RefreshDriver;
use euclid::Size2D;
use euclid::default::Size2D as UntypedSize2D;
use gleam::gl::{self, Gl};
use glow::NativeFramebuffer;
use image::RgbaImage;
use log::{debug, trace, warn};
use servo::webrender_api::units::DeviceIntRect;
pub use surfman::Error;
use surfman::chains::SwapChain;
use surfman::{
    Adapter, Connection, Context, ContextAttributeFlags, ContextAttributes, Device, GLApi,
    NativeContext, NativeWidget, Surface, SurfaceAccess, SurfaceInfo, SurfaceTexture, SurfaceType,
};
use winit::dpi::PhysicalSize;

/// A rendering context that uses the Surfman library to create and manage
/// the OpenGL context and surface. This struct provides the default implementation
/// of the `RenderingContext` trait, handling the creation, management, and destruction
/// of the rendering context and its associated resources.
///
/// The `SurfmanRenderingContext` struct encapsulates the necessary data and methods
/// to interact with the Surfman library, including creating surfaces, binding surfaces,
/// resizing surfaces, presenting rendered frames, and managing the OpenGL context state.
pub struct SurfmanRenderingContext {
    pub gleam_gl: Rc<dyn Gl>,
    pub glow_gl: Arc<glow::Context>,
    pub device: RefCell<Device>,
    pub context: RefCell<Context>,
    refresh_driver: Option<Rc<dyn RefreshDriver>>,
}

impl Drop for SurfmanRenderingContext {
    fn drop(&mut self) {
        let device = &mut self.device.borrow_mut();
        let context = &mut self.context.borrow_mut();
        let _ = device.destroy_context(context);
    }
}

impl SurfmanRenderingContext {
    pub fn new(
        connection: &Connection,
        adapter: &Adapter,
        refresh_driver: Option<Rc<dyn RefreshDriver>>,
    ) -> Result<Self, Error> {
        let device = connection.create_device(adapter)?;

        let flags = ContextAttributeFlags::ALPHA
            | ContextAttributeFlags::DEPTH
            | ContextAttributeFlags::STENCIL;
        let gl_api = connection.gl_api();
        let version = match &gl_api {
            GLApi::GLES => surfman::GLVersion { major: 3, minor: 0 },
            GLApi::GL => surfman::GLVersion { major: 3, minor: 2 },
        };
        let context_descriptor =
            device.create_context_descriptor(&ContextAttributes { flags, version })?;
        let context = device.create_context(&context_descriptor, None)?;

        #[expect(unsafe_code)]
        let gleam_gl = {
            match gl_api {
                GLApi::GL => unsafe {
                    gl::GlFns::load_with(|func_name| device.get_proc_address(&context, func_name))
                },
                GLApi::GLES => unsafe {
                    gl::GlesFns::load_with(|func_name| device.get_proc_address(&context, func_name))
                },
            }
        };

        #[expect(unsafe_code)]
        let glow_gl = unsafe {
            glow::Context::from_loader_function(|function_name| {
                device.get_proc_address(&context, function_name)
            })
        };

        Ok(SurfmanRenderingContext {
            gleam_gl,
            glow_gl: Arc::new(glow_gl),
            device: RefCell::new(device),
            context: RefCell::new(context),
            refresh_driver,
        })
    }

    pub fn create_surface(
        &self,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, Error> {
        let device = &mut self.device.borrow_mut();
        let context = &self.context.borrow();
        device.create_surface(context, SurfaceAccess::GPUOnly, surface_type)
    }

    pub fn bind_surface(&self, surface: Surface) -> Result<(), Error> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow_mut();
        device.bind_surface_to_context(context, surface).map_err(|(err, mut surface)| {
            let _ = device.destroy_surface(context, &mut surface);
            err
        })?;
        Ok(())
    }

    pub fn create_attached_swap_chain(&self) -> Result<SwapChain<Device>, Error> {
        let device = &mut self.device.borrow_mut();
        let context = &mut self.context.borrow_mut();
        SwapChain::create_attached(device, context, SurfaceAccess::GPUOnly)
    }

    fn resize_surface(&self, size: PhysicalSize<u32>) -> Result<(), Error> {
        let size = Size2D::new(size.width as i32, size.height as i32);
        let device = &mut self.device.borrow_mut();
        let context = &mut self.context.borrow_mut();

        let mut surface = device.unbind_surface_from_context(context)?.unwrap();
        device.resize_surface(context, &mut surface, size)?;
        device.bind_surface_to_context(context, surface).map_err(|(err, mut surface)| {
            let _ = device.destroy_surface(context, &mut surface);
            err
        })
    }

    fn present_bound_surface(&self) -> Result<(), Error> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow_mut();

        let mut surface = device.unbind_surface_from_context(context)?.unwrap();
        device.present_surface(context, &mut surface)?;
        device.bind_surface_to_context(context, surface).map_err(|(err, mut surface)| {
            let _ = device.destroy_surface(context, &mut surface);
            err
        })
    }

    #[expect(dead_code)]
    fn native_context(&self) -> NativeContext {
        let device = &self.device.borrow();
        let context = &self.context.borrow();
        device.native_context(context)
    }

    fn framebuffer(&self) -> Option<NativeFramebuffer> {
        let device = &self.device.borrow();
        let context = &self.context.borrow();
        device
            .context_surface_info(context)
            .unwrap_or(None)
            .and_then(|info| info.framebuffer_object)
    }

    pub fn prepare_for_rendering(&self) {
        let framebuffer_id = self.framebuffer().map_or(0, |framebuffer| framebuffer.0.into());
        self.gleam_gl.bind_framebuffer(gleam::gl::FRAMEBUFFER, framebuffer_id);
    }

    pub fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        let framebuffer_id = self.framebuffer().map_or(0, |framebuffer| framebuffer.0.into());
        Framebuffer::read_framebuffer_to_image(&self.gleam_gl, framebuffer_id, source_rectangle)
    }

    pub fn make_current(&self) -> Result<(), Error> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow();
        device.make_context_current(context)
    }

    pub fn create_texture(
        &self,
        surface: Surface,
    ) -> Option<(SurfaceTexture, u32, UntypedSize2D<i32>)> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow_mut();
        let SurfaceInfo { id: front_buffer_id, size, .. } = device.surface_info(&surface);
        debug!("... getting texture for surface {:?}", front_buffer_id);
        let surface_texture = device.create_surface_texture(context, surface).unwrap();
        let gl_texture =
            device.surface_texture_object(&surface_texture).map(|tex| tex.0.get()).unwrap_or(0);
        Some((surface_texture, gl_texture, size))
    }

    pub fn destroy_texture(&self, surface_texture: SurfaceTexture) -> Option<Surface> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow_mut();
        device.destroy_surface_texture(context, surface_texture).map_err(|(error, _)| error).ok()
    }

    pub fn connection(&self) -> Option<Connection> {
        Some(self.device.borrow().connection())
    }

    fn refresh_driver(&self) -> Option<Rc<dyn RefreshDriver>> {
        self.refresh_driver.clone()
    }
}

struct Framebuffer {
    gl: Rc<dyn Gl>,
    framebuffer_id: gl::GLuint,
    renderbuffer_id: gl::GLuint,
    texture_id: gl::GLuint,
}

impl Framebuffer {
    fn bind(&self) {
        trace!("Binding FBO {}", self.framebuffer_id);
        self.gl.bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id)
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        self.gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
        self.gl.delete_textures(&[self.texture_id]);
        self.gl.delete_renderbuffers(&[self.renderbuffer_id]);
        self.gl.delete_framebuffers(&[self.framebuffer_id]);
    }
}

impl Framebuffer {
    fn new(gl: Rc<dyn Gl>, size: PhysicalSize<u32>) -> Self {
        let framebuffer_ids = gl.gen_framebuffers(1);
        gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer_ids[0]);

        let texture_ids = gl.gen_textures(1);
        gl.bind_texture(gl::TEXTURE_2D, texture_ids[0]);
        gl.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as gl::GLint,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            None,
        );
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as gl::GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as gl::GLint);

        gl.framebuffer_texture_2d(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_2D,
            texture_ids[0],
            0,
        );

        gl.bind_texture(gl::TEXTURE_2D, 0);

        let renderbuffer_ids = gl.gen_renderbuffers(1);
        let depth_rb = renderbuffer_ids[0];
        gl.bind_renderbuffer(gl::RENDERBUFFER, depth_rb);
        gl.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::DEPTH_COMPONENT24,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
        );
        gl.framebuffer_renderbuffer(
            gl::FRAMEBUFFER,
            gl::DEPTH_ATTACHMENT,
            gl::RENDERBUFFER,
            depth_rb,
        );

        Self {
            gl,
            framebuffer_id: *framebuffer_ids.first().expect("Guaranteed by GL operations"),
            renderbuffer_id: *renderbuffer_ids.first().expect("Guaranteed by GL operations"),
            texture_id: *texture_ids.first().expect("Guaranteed by GL operations"),
        }
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        Self::read_framebuffer_to_image(&self.gl, self.framebuffer_id, source_rectangle)
    }

    fn read_framebuffer_to_image(
        gl: &Rc<dyn Gl>,
        framebuffer_id: u32,
        source_rectangle: DeviceIntRect,
    ) -> Option<RgbaImage> {
        gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer_id);

        // For some reason, OSMesa fails to render on the 3rd
        // attempt in headless mode, under some conditions.
        // I think this can only be some kind of synchronization
        // bug in OSMesa, but explicitly un-binding any vertex
        // array here seems to work around that bug.
        // See https://github.com/servo/servo/issues/18606.
        gl.bind_vertex_array(0);

        let mut pixels = gl.read_pixels(
            source_rectangle.min.x,
            source_rectangle.min.y,
            source_rectangle.width(),
            source_rectangle.height(),
            gl::RGBA,
            gl::UNSIGNED_BYTE,
        );
        let gl_error = gl.get_error();
        if gl_error != gl::NO_ERROR {
            warn!("GL error code 0x{gl_error:x} set after read_pixels");
        }

        // Flip image vertically (OpenGL textures are upside down)
        let source_rectangle = source_rectangle.to_usize();
        let orig_pixels = pixels.clone();
        let stride = source_rectangle.width() * 4; // 4 bytes per RGBA pixel
        for y in 0..source_rectangle.height() {
            let dst_start = y * stride;
            let src_start = (source_rectangle.height() - y - 1) * stride;
            let src_slice = &orig_pixels[src_start..src_start + stride];
            pixels[dst_start..dst_start + stride].clone_from_slice(&src_slice[..stride]);
        }

        RgbaImage::from_raw(
            source_rectangle.width() as u32,
            source_rectangle.height() as u32,
            pixels,
        )
    }
}

#[cfg(test)]
mod test {
    use euclid::{Box2D, Point2D, Size2D};
    use gleam::gl;
    use image::Rgba;
    use surfman::{Connection, ContextAttributeFlags, ContextAttributes, Error, GLApi, GLVersion};
    use winit::dpi::PhysicalSize;

    use super::Framebuffer;

    #[test]
    #[expect(unsafe_code)]
    fn test_read_pixels() -> Result<(), Error> {
        let connection = Connection::new()?;
        let adapter = connection.create_software_adapter()?;
        let device = connection.create_device(&adapter)?;
        let context_descriptor = device.create_context_descriptor(&ContextAttributes {
            version: GLVersion::new(3, 0),
            flags: ContextAttributeFlags::empty(),
        })?;
        let mut context = device.create_context(&context_descriptor, None)?;

        let gl = match connection.gl_api() {
            GLApi::GL => unsafe { gl::GlFns::load_with(|s| device.get_proc_address(&context, s)) },
            GLApi::GLES => unsafe {
                gl::GlesFns::load_with(|s| device.get_proc_address(&context, s))
            },
        };

        device.make_context_current(&context)?;

        {
            const SIZE: u32 = 16;
            let framebuffer = Framebuffer::new(gl, PhysicalSize::new(SIZE, SIZE));
            framebuffer.bind();
            framebuffer.gl.clear_color(12.0 / 255.0, 34.0 / 255.0, 56.0 / 255.0, 78.0 / 255.0);
            framebuffer.gl.clear(gl::COLOR_BUFFER_BIT);

            let rect = Box2D::from_origin_and_size(Point2D::zero(), Size2D::new(SIZE, SIZE));
            let img = framebuffer
                .read_to_image(rect.to_i32())
                .expect("Should have been able to read back image.");
            assert_eq!(img.width(), SIZE);
            assert_eq!(img.height(), SIZE);

            let expected_pixel: Rgba<u8> = Rgba([12, 34, 56, 78]);
            assert!(img.pixels().all(|&p| p == expected_pixel));
        }

        device.destroy_context(&mut context)?;

        Ok(())
    }
}
