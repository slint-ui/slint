// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
/*!
    Graphics Abstractions.

    This module contains the abstractions and convenience types used for rendering.

    The run-time library also makes use of [RenderingCache] to store the rendering primitives
    created by the backend in a type-erased manner.
*/
extern crate alloc;
use crate::api::PlatformError;
use crate::lengths::LogicalLength;
use crate::Coord;
use crate::SharedString;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::format;

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

/// CachedGraphicsData allows the graphics backend to store an arbitrary piece of data associated with
/// an item, which is typically computed by accessing properties. The dependency_tracker is used to allow
/// for a lazy computation. Typically back ends store either compute intensive data or handles that refer to
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

/// The RenderingCache, in combination with CachedGraphicsData, allows back ends to store data that's either
/// intensive to compute or has bad CPU locality. Back ends typically keep a RenderingCache instance and use
/// the item's cached_rendering_data() integer as index in the vec_arena::Arena.
///
/// This is used only for the [`crate::item_rendering::PartialRenderingCache`]
pub struct RenderingCache<T> {
    slab: slab::Slab<CachedGraphicsData<T>>,
    generation: usize,
}

impl<T> Default for RenderingCache<T> {
    fn default() -> Self {
        Self { slab: Default::default(), generation: 1 }
    }
}

impl<T> RenderingCache<T> {
    /// Returns the generation of the cache. The generation starts at 1 and is increased
    /// whenever the cache is cleared, for example when the GL context is lost.
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// Retrieves a mutable reference to the cached graphics data at index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut CachedGraphicsData<T>> {
        self.slab.get_mut(index)
    }

    /// Returns true if a cache entry exists for the given index.
    pub fn contains(&self, index: usize) -> bool {
        self.slab.contains(index)
    }

    /// Inserts data into the cache and returns the index for retrieval later.
    pub fn insert(&mut self, data: CachedGraphicsData<T>) -> usize {
        self.slab.insert(data)
    }

    /// Retrieves an immutable reference to the cached graphics data at index.
    pub fn get(&self, index: usize) -> Option<&CachedGraphicsData<T>> {
        self.slab.get(index)
    }

    /// Removes the cached graphics data at the given index.
    pub fn remove(&mut self, index: usize) -> CachedGraphicsData<T> {
        self.slab.remove(index)
    }

    /// Removes all entries from the cache and increases the cache's generation count, so
    /// that stale index access can be avoided.
    pub fn clear(&mut self) {
        self.slab.clear();
        self.generation += 1;
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

#[cfg(feature = "shared-fontdb")]
impl FontRequest {
    /// Returns the relevant properties of this FontRequest propagated into a fontdb Query.
    pub fn to_fontdb_query(&self) -> i_slint_common::sharedfontdb::fontdb::Query<'_> {
        use i_slint_common::sharedfontdb::fontdb::{Query, Style, Weight};
        Query {
            style: if self.italic { Style::Italic } else { Style::Normal },
            weight: Weight(self.weight.unwrap_or(/* CSS normal*/ 400) as _),
            ..Default::default()
        }
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
#[derive(Debug, Clone, PartialEq)]
pub enum RequestedGraphicsAPI {
    /// OpenGL (ES)
    OpenGL(RequestedOpenGLVersion),
    /// Metal
    Metal,
    /// Vulkan
    Vulkan,
    /// Direct 3D
    Direct3D,
}

impl TryFrom<RequestedGraphicsAPI> for RequestedOpenGLVersion {
    type Error = PlatformError;

    fn try_from(requested_graphics_api: RequestedGraphicsAPI) -> Result<Self, Self::Error> {
        match requested_graphics_api {
            RequestedGraphicsAPI::OpenGL(requested_open_glversion) => Ok(requested_open_glversion),
            RequestedGraphicsAPI::Metal => {
                Err(format!("Metal rendering is not supported with an OpenGL renderer").into())
            }
            RequestedGraphicsAPI::Vulkan => {
                Err(format!("Vulkan rendering is not supported with an OpenGL renderer").into())
            }
            RequestedGraphicsAPI::Direct3D => {
                Err(format!("Direct3D rendering is not supported with an OpenGL renderer").into())
            }
        }
    }
}

impl From<RequestedOpenGLVersion> for RequestedGraphicsAPI {
    fn from(version: RequestedOpenGLVersion) -> Self {
        Self::OpenGL(version)
    }
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
