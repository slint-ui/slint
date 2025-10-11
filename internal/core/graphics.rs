// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
/*!
    Graphics Abstractions.

    This module contains the abstractions and convenience types used for rendering.
*/
extern crate alloc;
use crate::api::PlatformError;
use crate::lengths::LogicalLength;
use crate::Coord;
use crate::SharedString;
use alloc::boxed::Box;

pub use euclid;
/// 2D Rectangle
pub type Rect = euclid::default::Rect<Coord>;
/// 2D Rectangle with integer coordinates
pub type IntRect = euclid::default::Rect<i32>;
/// 2D Point
pub type Point = euclid::default::Point2D<Coord>;
/// 2D Size
pub type Size = euclid::default::Size2D<Coord>;
/// 2D Size in integer coordinates
pub type IntSize = euclid::default::Size2D<u32>;
/// 2D Transform
pub type Transform = euclid::default::Transform2D<Coord>;

pub(crate) mod color;
pub use color::*;

#[cfg(feature = "std")]
mod path;
#[cfg(feature = "std")]
pub use path::*;

mod brush;
pub use brush::*;

pub(crate) mod image;
pub use self::image::*;

pub(crate) mod bitmapfont;
pub use self::bitmapfont::*;

pub mod rendering_metrics_collector;

#[cfg(feature = "box-shadow-cache")]
pub mod boxshadowcache;

pub mod border_radius;
pub use border_radius::*;

#[cfg(feature = "unstable-wgpu-26")]
pub mod wgpu_26;
#[cfg(feature = "unstable-wgpu-27")]
pub mod wgpu_27;

/// CachedGraphicsData allows the graphics backend to store an arbitrary piece of data associated with
/// an item, which is typically computed by accessing properties. The dependency_tracker is used to allow
/// for a lazy computation. Typically, back ends store either compute intensive data or handles that refer to
/// data that's stored in GPU memory.
pub struct CachedGraphicsData<T> {
    /// The backend specific data.
    pub data: T,
    /// The property tracker that should be used to evaluate whether the primitive needs to be re-created
    /// or not.
    pub dependency_tracker: Option<core::pin::Pin<Box<crate::properties::PropertyTracker>>>,
}

impl<T> CachedGraphicsData<T> {
    /// Creates a new TrackingRenderingPrimitive by evaluating the provided update_fn once, storing the returned
    /// rendering primitive and initializing the dependency tracker.
    pub fn new(update_fn: impl FnOnce() -> T) -> Self {
        let dependency_tracker = Box::pin(crate::properties::PropertyTracker::default());
        let data = dependency_tracker.as_ref().evaluate(update_fn);
        Self { data, dependency_tracker: Some(dependency_tracker) }
    }
}

/// FontRequest collects all the developer-configurable properties for fonts, such as family, weight, etc.
/// It is submitted as a request to the platform font system (i.e. CoreText on macOS) and in exchange the
/// backend returns a `Box<dyn Font>`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FontRequest {
    /// The name of the font family to be used, such as "Helvetica". An empty family name means the system
    /// default font family should be used.
    pub family: Option<SharedString>,
    /// If the weight is None, the system default font weight should be used.
    pub weight: Option<i32>,
    /// If the pixel size is None, the system default font size should be used.
    pub pixel_size: Option<LogicalLength>,
    /// The additional spacing (or shrinking if negative) between glyphs. This is usually not submitted to
    /// the font-subsystem but collected here for API convenience
    pub letter_spacing: Option<LogicalLength>,
    /// Whether to select an italic face of the font family.
    pub italic: bool,
}

#[cfg(feature = "shared-fontique")]
impl FontRequest {
    /// Attempts to query the fontique font collection for a matching font.
    pub fn query_fontique(&self) -> Option<i_slint_common::sharedfontique::fontique::QueryFont> {
        use i_slint_common::sharedfontique::{self, fontique};

        let mut collection = sharedfontique::get_collection();

        let mut query = collection.query();
        query.set_families(
            self.family
                .as_ref()
                .map(|family| fontique::QueryFamily::from(family.as_str()))
                .into_iter()
                .chain(
                    [
                        fontique::QueryFamily::Generic(fontique::GenericFamily::SansSerif),
                        fontique::QueryFamily::Generic(fontique::GenericFamily::SystemUi),
                    ]
                    .into_iter(),
                ),
        );

        query.set_attributes(fontique::Attributes {
            weight: self
                .weight
                .as_ref()
                .map(|&weight| fontique::FontWeight::new(weight as f32))
                .unwrap_or_default(),
            style: if self.italic {
                fontique::FontStyle::Italic
            } else {
                fontique::FontStyle::Normal
            },
            ..Default::default()
        });

        let mut font = None;

        query.matches_with(|queried_font| {
            font = Some(queried_font.clone());
            fontique::QueryStatus::Stop
        });

        font
    }
}

/// Internal enum to specify which version of OpenGL to request
/// from the windowing system.
#[derive(Debug, Clone, PartialEq)]
pub enum RequestedOpenGLVersion {
    /// OpenGL
    OpenGL(Option<(u8, u8)>),
    /// OpenGL ES
    OpenGLES(Option<(u8, u8)>),
}

/// Internal enum specify which graphics API should be used, when
/// the backend selector requests that from a built-in backend.
#[derive(Debug, Clone)]
pub enum RequestedGraphicsAPI {
    /// OpenGL (ES)
    OpenGL(RequestedOpenGLVersion),
    /// Metal
    Metal,
    /// Vulkan
    Vulkan,
    /// Direct 3D
    Direct3D,
    #[cfg(feature = "unstable-wgpu-26")]
    /// WGPU 26.x
    WGPU26(wgpu_26::api::WGPUConfiguration),
    #[cfg(feature = "unstable-wgpu-27")]
    /// WGPU 27.x
    WGPU27(wgpu_27::api::WGPUConfiguration),
}

impl TryFrom<&RequestedGraphicsAPI> for RequestedOpenGLVersion {
    type Error = PlatformError;

    fn try_from(requested_graphics_api: &RequestedGraphicsAPI) -> Result<Self, Self::Error> {
        match requested_graphics_api {
            RequestedGraphicsAPI::OpenGL(requested_open_glversion) => {
                Ok(requested_open_glversion.clone())
            }
            RequestedGraphicsAPI::Metal => {
                Err("Metal rendering is not supported with an OpenGL renderer".into())
            }
            RequestedGraphicsAPI::Vulkan => {
                Err("Vulkan rendering is not supported with an OpenGL renderer".into())
            }
            RequestedGraphicsAPI::Direct3D => {
                Err("Direct3D rendering is not supported with an OpenGL renderer".into())
            }
            #[cfg(feature = "unstable-wgpu-26")]
            RequestedGraphicsAPI::WGPU26(..) => {
                Err("WGPU 26.x rendering is not supported with an OpenGL renderer".into())
            }
            #[cfg(feature = "unstable-wgpu-27")]
            RequestedGraphicsAPI::WGPU27(..) => {
                Err("WGPU 27.x rendering is not supported with an OpenGL renderer".into())
            }
        }
    }
}

impl From<RequestedOpenGLVersion> for RequestedGraphicsAPI {
    fn from(version: RequestedOpenGLVersion) -> Self {
        Self::OpenGL(version)
    }
}

/// Private API exposed to just the renderers to create GraphicsAPI instance with
/// non-exhaustive enum variant.
#[cfg(feature = "unstable-wgpu-26")]
pub fn create_graphics_api_wgpu_26(
    instance: wgpu_26::wgpu::Instance,
    device: wgpu_26::wgpu::Device,
    queue: wgpu_26::wgpu::Queue,
) -> crate::api::GraphicsAPI<'static> {
    crate::api::GraphicsAPI::WGPU26 { instance, device, queue }
}

/// Private API exposed to just the renderers to create GraphicsAPI instance with
/// non-exhaustive enum variant.
#[cfg(feature = "unstable-wgpu-27")]
pub fn create_graphics_api_wgpu_27(
    instance: wgpu_27::wgpu::Instance,
    device: wgpu_27::wgpu::Device,
    queue: wgpu_27::wgpu::Queue,
) -> crate::api::GraphicsAPI<'static> {
    crate::api::GraphicsAPI::WGPU27 { instance, device, queue }
}

/// Internal module for use by cbindgen and the C++ platform API layer.
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]

    /// Expand Rect so that cbindgen can see it. ( is in fact euclid::default::Rect<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    /// Expand IntRect so that cbindgen can see it. ( is in fact euclid::default::Rect<i32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct IntRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    /// Expand Point so that cbindgen can see it. ( is in fact euclid::default::Point2D<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Point {
        x: f32,
        y: f32,
    }

    /// Expand Box2D so that cbindgen can see it.
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Box2D<T, U> {
        min: euclid::Point2D<T>,
        max: euclid::Point2D<T>,
        _unit: std::marker::PhantomData<U>,
    }

    #[cfg(feature = "std")]
    pub use super::path::ffi::*;

    /// Conversion function used by C++ platform API layer to
    /// convert the PhysicalSize used in the Rust WindowAdapter API
    /// to the ffi.
    pub fn physical_size_from_api(
        size: crate::api::PhysicalSize,
    ) -> crate::graphics::euclid::default::Size2D<u32> {
        size.to_euclid()
    }

    /// Conversion function used by C++ platform API layer to
    /// convert the PhysicalPosition used in the Rust WindowAdapter API
    /// to the ffi.
    pub fn physical_position_from_api(
        position: crate::api::PhysicalPosition,
    ) -> crate::graphics::euclid::default::Point2D<i32> {
        position.to_euclid()
    }

    /// Conversion function used by C++ platform API layer to
    /// convert from the ffi to PhysicalPosition.
    pub fn physical_position_to_api(
        position: crate::graphics::euclid::default::Point2D<i32>,
    ) -> crate::api::PhysicalPosition {
        crate::api::PhysicalPosition::from_euclid(position)
    }
}
