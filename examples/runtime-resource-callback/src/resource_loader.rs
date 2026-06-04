
// Implements the runtime resource loading callback system proposed in #10970.
//
//   1. ResourceProvider trait — application code implements this
//   2. Global registry — stores the user-supplied provider
//   3. FileSystemProvider — loads from disk
//   4. InMemoryProvider — serves from an in-memory HashMap
//   5. load_slint_image() — decodes bytes into a slint::Image

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Raw bytes returned by a resource load.
pub type ResourceData = Vec<u8>;

/// Errors that a provider can return.
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

/// Trait that an application implements to supply resources to Slint at runtime.
///
/// The path received is the *relative* path as written in the .slint file.
/// Implementations may load from disk, compressed archives, Qt resources,
/// a database, or anywhere else.
pub trait ResourceProvider: Send + Sync {
    /// Load an image. Returns raw bytes (PNG, JPEG, etc.).
    fn load_image(&self, path: &Path) -> Result<ResourceData, ResourceError>;

    /// Load a font. Returns raw TTF/OTF bytes.
    fn load_font(&self, path: &Path) -> Result<ResourceData, ResourceError>;
}

// ---------------------------------------------------------------------------
// Global registry
// ---------------------------------------------------------------------------

static RESOURCE_PROVIDER: OnceLock<Box<dyn ResourceProvider>> = OnceLock::new();

/// Register the application's resource provider.
///
/// Must be called once before any Slint window is shown.
/// Panics if called more than once.
pub fn set_resource_provider(provider: Box<dyn ResourceProvider>) {
    RESOURCE_PROVIDER
        .set(provider)
        .expect("set_resource_provider() called more than once");
}

/// Load an image through the registered provider.
pub fn load_image(path: &Path) -> Option<Result<ResourceData, ResourceError>> {
    RESOURCE_PROVIDER.get().map(|p| p.load_image(path))
}

/// Load a font through the registered provider.
pub fn load_font(path: &Path) -> Option<Result<ResourceData, ResourceError>> {
    RESOURCE_PROVIDER.get().map(|p| p.load_font(path))
}

// ---------------------------------------------------------------------------
// FileSystemProvider
// ---------------------------------------------------------------------------

/// Loads resources from the file system.
/// `root_dir` is prepended to all relative paths.
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

/// Serves resources from an in-memory HashMap.
/// Keys are the path strings as they appear in .slint files.
/// Useful for bundled/compressed resources or Qt's resource system.
pub struct InMemoryProvider {
    data: HashMap<String, ResourceData>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    /// Register a resource by its .slint path and raw bytes.
    pub fn add(&mut self, slint_path: impl Into<String>, bytes: impl Into<ResourceData>) {
        self.data.insert(slint_path.into(), bytes.into());
    }
}

impl Default for InMemoryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceProvider for InMemoryProvider {
    fn load_image(&self, path: &Path) -> Result<ResourceData, ResourceError> {
        let key = path.to_string_lossy().to_string();
        println!("[ResourceLoader] Loading image from memory: {key}");
        self.data
            .get(&key)
            .cloned()
            .ok_or_else(|| ResourceError::NotFound(path.to_path_buf()))
    }

    fn load_font(&self, path: &Path) -> Result<ResourceData, ResourceError> {
        let key = path.to_string_lossy().to_string();
        println!("[ResourceLoader] Loading font from memory: {key}");
        self.data
            .get(&key)
            .cloned()
            .ok_or_else(|| ResourceError::NotFound(path.to_path_buf()))
    }
}

// ---------------------------------------------------------------------------
// Slint integration helper
// ---------------------------------------------------------------------------

/// Load an image via the registered provider and decode it into a slint::Image.
///
/// This is what the Slint runtime would call internally once this feature is
/// merged. For now, application code calls it and sets the property manually.
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