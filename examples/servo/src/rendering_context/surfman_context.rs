// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::{cell::RefCell, rc::Rc, sync::Arc};

use euclid::default::Size2D;
use gleam::gl::{self, Gl};
use glow::NativeFramebuffer;
use image::RgbaImage;
use servo::webrender_api::units::DeviceIntRect;

use surfman::{
    Adapter, Connection, Context, ContextAttributeFlags, ContextAttributes, Device, Error, GLApi,
    NativeWidget, Surface, SurfaceAccess, SurfaceInfo, SurfaceTexture, SurfaceType,
    chains::SwapChain,
};

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
}

impl Drop for SurfmanRenderingContext {
    fn drop(&mut self) {
        let device = &mut self.device.borrow_mut();
        let context = &mut self.context.borrow_mut();
        let _ = device.destroy_context(context);
    }
}

impl SurfmanRenderingContext {
    pub fn new(connection: &Connection, adapter: &Adapter) -> Result<Self, Error> {
        let mut device = connection.create_device(adapter)?;

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

        #[allow(unsafe_code)]
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

        #[allow(unsafe_code)]
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
        device
            .bind_surface_to_context(context, surface)
            .map_err(|(err, mut surface)| {
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

    fn framebuffer(&self) -> Option<NativeFramebuffer> {
        let device = &self.device.borrow();
        let context = &self.context.borrow();
        device
            .context_surface_info(context)
            .unwrap_or(None)
            .and_then(|info| info.framebuffer_object)
    }

    pub fn prepare_for_rendering(&self) {
        let framebuffer_id = self
            .framebuffer()
            .map_or(0, |framebuffer| framebuffer.0.into());
        self.gleam_gl
            .bind_framebuffer(gleam::gl::FRAMEBUFFER, framebuffer_id);
    }

    pub fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        let framebuffer_id = self
            .framebuffer()
            .map_or(0, |framebuffer| framebuffer.0.into());
        Self::read_framebuffer_to_image(&self.gleam_gl, framebuffer_id, source_rectangle)
    }

    pub fn make_current(&self) -> Result<(), Error> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow();
        device.make_context_current(context)
    }

    pub fn create_texture(&self, surface: Surface) -> Option<(SurfaceTexture, u32, Size2D<i32>)> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow_mut();

        let SurfaceInfo {
            id: _front_buffer_id,
            size,
            ..
        } = device.surface_info(&surface);
        // debug!("... getting texture for surface {:?}", front_buffer_id);
        let surface_texture = device.create_surface_texture(context, surface).ok()?;

        let gl_texture = device
            .surface_texture_object(&surface_texture)
            .map(|tex| tex.0.get())
            .unwrap_or(0);

        Some((surface_texture, gl_texture, size))
    }

    pub fn destroy_texture(&self, surface_texture: SurfaceTexture) -> Option<Surface> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow_mut();

        device
            .destroy_surface_texture(context, surface_texture)
            .map_err(|(error, _)| error)
            .ok()
    }

    pub fn connection(&self) -> Option<Connection> {
        Some(self.device.borrow().connection())
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
            // warn!("GL error code 0x{gl_error:x} set after read_pixels");
        }

        // flip image vertically (texture is upside down)
        let source_rectangle = source_rectangle.to_usize();
        let orig_pixels = pixels.clone();
        let stride = source_rectangle.width() * 4;
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
