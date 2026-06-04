// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#![allow(dead_code)]
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub type ResourceData = Vec<u8>;

#[derive(Debug)]
pub enum ResourceError {
    NotFound(PathBuf),
    IoError(std::io::Error),
    Custom(String),
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceError::NotFound(p) => write!(f, "Resource not found: {}", p.display()),
            ResourceError::IoError(e) => write!(f, "IO error: {e}"),
            ResourceError::Custom(s) => write!(f, "Resource error: {s}"),
        }
    }
}

impl From<std::io::Error> for ResourceError {
    fn from(e: std::io::Error) -> Self {
        ResourceError::IoError(e)
    }
}

pub trait ResourceProvider: Send + Sync {
    fn load_image(&self, path: &Path) -> Result<ResourceData, ResourceError>;
    fn load_font(&self, path: &Path) -> Result<ResourceData, ResourceError>;
}

static RESOURCE_PROVIDER: OnceLock<Box<dyn ResourceProvider>> = OnceLock::new();

/// Register the application's resource provider.
/// Must be called once before any Slint window is shown.
pub fn set_resource_provider(provider: Box<dyn ResourceProvider>) {
    if RESOURCE_PROVIDER.set(provider).is_err() {
        panic!("set_resource_provider() called more than once");
    }
}

pub fn load_image(path: &Path) -> Option<Result<ResourceData, ResourceError>> {
    RESOURCE_PROVIDER.get().map(|p| p.load_image(path))
}

pub fn load_font(path: &Path) -> Option<Result<ResourceData, ResourceError>> {
    RESOURCE_PROVIDER.get().map(|p| p.load_font(path))
}

// ---------------------------------------------------------------------------
// FileSystemProvider
// ---------------------------------------------------------------------------

pub struct FileSystemProvider {
    root_dir: PathBuf,
}

impl FileSystemProvider {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self { root_dir: root_dir.into() }
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root_dir.join(path)
        }
    }

    fn read(&self, path: &Path) -> Result<ResourceData, ResourceError> {
        let full = self.resolve(path);
        std::fs::read(&full).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ResourceError::NotFound(full)
            } else {
                ResourceError::IoError(e)
            }
        })
    }
}

impl ResourceProvider for FileSystemProvider {
    fn load_image(&self, path: &Path) -> Result<ResourceData, ResourceError> {
        println!("[ResourceLoader] Loading image: {}", path.display());
        self.read(path)
    }

    fn load_font(&self, path: &Path) -> Result<ResourceData, ResourceError> {
        println!("[ResourceLoader] Loading font: {}", path.display());
        self.read(path)
    }
}

// ---------------------------------------------------------------------------
// InMemoryProvider
// ---------------------------------------------------------------------------

/// Serves resources from an in-memory map.
/// Keys are path strings as they appear in .slint files.
#[derive(Default)] // <-- fixes the clippy warning
pub struct InMemoryProvider {
    data: HashMap<String, ResourceData>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    pub fn add(&mut self, slint_path: impl Into<String>, bytes: impl Into<ResourceData>) {
        self.data.insert(slint_path.into(), bytes.into());
    }
}

impl ResourceProvider for InMemoryProvider {
    fn load_image(&self, path: &Path) -> Result<ResourceData, ResourceError> {
        let key = path.to_string_lossy().to_string();
        println!("[ResourceLoader] Loading image from memory: {key}");
        self.data.get(&key).cloned().ok_or_else(|| ResourceError::NotFound(path.to_path_buf()))
    }

    fn load_font(&self, path: &Path) -> Result<ResourceData, ResourceError> {
        let key = path.to_string_lossy().to_string();
        println!("[ResourceLoader] Loading font from memory: {key}");
        self.data.get(&key).cloned().ok_or_else(|| ResourceError::NotFound(path.to_path_buf()))
    }
}

// ---------------------------------------------------------------------------
// Slint integration helper
// ---------------------------------------------------------------------------

pub fn load_slint_image(path: impl AsRef<Path>) -> Result<slint::Image, ResourceError> {
    let path = path.as_ref();
    let bytes = load_image(path)
        .unwrap_or_else(|| Err(ResourceError::Custom("No provider registered".into())))?;

    let img = image::load_from_memory(&bytes)
        .map_err(|e| ResourceError::Custom(format!("Image decode error: {e}")))?
        .into_rgba8();

    let (width, height) = img.dimensions();
    let mut pixel_buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
    pixel_buf.make_mut_bytes().copy_from_slice(img.as_raw().as_slice());

    Ok(slint::Image::from_rgba8(pixel_buf))
}
