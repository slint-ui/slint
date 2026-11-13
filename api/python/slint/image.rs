// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore asarray fromarray getbuffer itemsize matplotlib ndim releasebuffer ssize suboffsets
use pyo3::ffi;
use pyo3::prelude::*;
use slint_interpreter::{Rgba8Pixel, SharedPixelBuffer};
use std::ffi::{c_int, c_void};

/// Keeps the pixels and the shape arrays of an exported buffer alive until it's released.
struct ExportedPixels {
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    shape: [ffi::Py_ssize_t; 3],
    strides: [ffi::Py_ssize_t; 3],
}

/// Image objects can be set on Slint Image elements for display. Use `Image.load_from_path` to construct Image
/// objects from a path to an image file on disk.
///
/// Images implement the [Buffer Protocol](https://docs.python.org/3/c-api/buffer.html) to give read-only access
/// to their pixels, as an array of unsigned bytes with the shape (height, width, 4).
/// Each pixel is encoded as RGBA, without premultiplied alpha.
/// Use this to pass an image to third-party modules such as NumPy or Pillow.
/// A `BufferError` is raised if the pixels can't be obtained.
///
/// Example of saving an image with Pillow:
/// ```python
/// import slint
/// from PIL import Image
/// import numpy as np
///
/// img = slint.Image.load_from_path("hello.jpeg")
/// Image.fromarray(np.asarray(img)).save("hello.png")
/// ```
#[pyclass(unsendable, name = "Image")]
pub struct PyImage {
    pub image: slint_interpreter::Image,
}

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        Ok(Self { image: Default::default() })
    }

    /// The size of the image as a tuple of `width` and `height`.
    #[getter]
    fn size(&self) -> PyResult<(u32, u32)> {
        Ok(self.image.size().into())
    }

    /// The width of the image in pixels.
    #[getter]
    fn width(&self) -> PyResult<u32> {
        Ok(self.image.size().width)
    }

    /// The height of the image in pixels.
    #[getter]
    fn height(&self) -> PyResult<u32> {
        Ok(self.image.size().height)
    }

    /// The path of the image if it was loaded from disk, or `None`.
    #[getter]
    fn path(&self) -> PyResult<Option<std::path::PathBuf>> {
        Ok(self.image.path().map(|p| p.to_path_buf()))
    }

    /// Loads the image from the specified path. Raises `RuntimeError` if the image can't be loaded.
    #[staticmethod]
    fn load_from_path(path: std::path::PathBuf) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_path(&path)?;
        Ok(Self { image })
    }

    /// Creates a new image from bytes that describe the image in SVG format.
    #[staticmethod]
    fn load_from_svg_data(data: Vec<u8>) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_svg_data(&data)?;
        Ok(Self { image })
    }

    /// Creates a new image from an array-like object that implements the [Buffer Protocol](https://docs.python.org/3/c-api/buffer.html).
    /// Use this function to import images created by third-party modules such as matplotlib or Pillow.
    ///
    /// The array must satisfy certain constraints to represent an image:
    ///
    ///  - The buffer's format needs to be `B` (unsigned char)
    ///  - The shape must be a tuple of (height, width, bytes-per-pixel)
    ///  - If a stride is defined, the row stride must be equal to width * bytes-per-pixel, and the column stride must equal the bytes-per-pixel.
    ///  - A value of 3 for bytes-per-pixel is interpreted as an RGB image, a value of 4 means RGBA.
    ///
    /// The image is created by performing a deep copy of the array's data. Subsequent changes to the buffer aren't
    /// reflected in a previously created Image.
    ///
    /// Example of importing a Matplotlib figure into an image:
    /// ```python
    /// import slint
    /// import matplotlib
    ///
    /// from matplotlib.backends.backend_agg import FigureCanvasAgg
    /// from matplotlib.figure import Figure
    ///
    /// fig = Figure(figsize=(5, 4), dpi=100)
    /// canvas = FigureCanvasAgg(fig)
    /// ax = fig.add_subplot()
    /// ax.plot([1, 2, 3])
    /// canvas.draw()
    ///
    /// buffer = canvas.buffer_rgba()
    /// img = slint.Image.load_from_array(buffer)
    /// ```
    ///
    /// Example of loading an image with Pillow:
    /// ```python
    /// import slint
    /// from PIL import Image
    /// import numpy as np
    ///
    /// pil_img = Image.open("hello.jpeg")
    /// array = np.array(pil_img)
    /// img = slint.Image.load_from_array(array)
    /// ```
    #[staticmethod]
    fn load_from_array(array: &Bound<'_, PyAny>) -> PyResult<Self> {
        let buffer: pyo3::buffer::PyBuffer<u8> = pyo3::buffer::PyBuffer::get(array)?;

        let shape = buffer.shape();
        if shape.len() != 3 {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Arrays must have a shape of (height, width, bpp) for image conversion",
            ));
        }
        let bpp: u32 = shape[2]
            .try_into()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Image bpp exceeds u32"))?;
        let width = shape[1]
            .try_into()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Image width exceeds u32"))?;
        let height = shape[0]
            .try_into()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Image height exceeds u32"))?;

        if buffer.item_size() != 1 {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Item size {} is not valid. Arrays must contain bytes for image conversion",
                buffer.item_size(),
            )));
        }

        if buffer.format() != c"B" {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Unexpected buffer format {}, expected 'B' for unsigned char",
                buffer.format().to_str().unwrap_or_default(),
            )));
        }

        let strides = buffer.strides();
        if strides.len() > 0 {
            if strides.len() != 3 {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Unexpected strides size {}. Arrays must provides stride tuple of 3 for image conversion",
                    strides.len(),
                )));
            }

            let row_stride: u32 = strides[0].try_into().map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("Image row stride cannot be negative")
            })?;
            let column_stride: u32 = strides[1].try_into().map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("Image column stride cannot be negative")
            })?;
            let elem_stride: u32 = strides[2].try_into().map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("Image element stride cannot be negative")
            })?;

            if row_stride != width * bpp {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Unexpected row stride {}. Expected {}",
                    row_stride,
                    height * bpp,
                )));
            }

            if column_stride != bpp {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Unexpected column stride {}. Expected {}",
                    column_stride, bpp,
                )));
            }

            if elem_stride != 1 {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Unexpected element stride {}. Expected 1",
                    column_stride,
                )));
            }
        }

        Ok(Self {
            image: match bpp {
                3 => {
                    let mut pixel_buffer = SharedPixelBuffer::new(width, height);
                    buffer.copy_to_slice(array.py(), pixel_buffer.make_mut_bytes())?;
                    slint_interpreter::Image::from_rgb8(pixel_buffer)
                }
                4 => {
                    let mut pixel_buffer = SharedPixelBuffer::new(width, height);
                    buffer.copy_to_slice(array.py(), pixel_buffer.make_mut_bytes())?;
                    slint_interpreter::Image::from_rgba8(pixel_buffer)
                }
                _ => {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "Unexpected bits per pixel {}. Expected 3 or 4",
                        bpp,
                    )));
                }
            },
        })
    }

    unsafe fn __getbuffer__(
        slf: Bound<'_, Self>,
        view: *mut ffi::Py_buffer,
        flags: c_int,
    ) -> PyResult<()> {
        if flags & ffi::PyBUF_WRITABLE == ffi::PyBUF_WRITABLE {
            return Err(pyo3::exceptions::PyBufferError::new_err("Image pixels are read-only"));
        }
        let pixels = slf.borrow().image.to_rgba8().ok_or_else(|| {
            pyo3::exceptions::PyBufferError::new_err("The pixels of this image aren't available")
        })?;
        let (width, height) = (pixels.width() as ffi::Py_ssize_t, pixels.height() as _);
        let exported = Box::into_raw(Box::new(ExportedPixels {
            pixels,
            shape: [height, width, 4],
            strides: [width * 4, 4, 1],
        }));
        // Safety: the caller passes a valid `view`, and `exported` was allocated above.
        // The pointers stored in the view stay valid until `__releasebuffer__` frees `exported`.
        unsafe {
            let bytes = (*exported).pixels.as_bytes();
            (*view).buf = bytes.as_ptr() as *mut c_void;
            (*view).len = bytes.len() as _;
            (*view).readonly = 1;
            (*view).itemsize = 1;
            (*view).format = if flags & ffi::PyBUF_FORMAT == ffi::PyBUF_FORMAT {
                c"B".as_ptr() as *mut _
            } else {
                std::ptr::null_mut()
            };
            (*view).ndim = 3;
            (*view).shape = if flags & ffi::PyBUF_ND == ffi::PyBUF_ND {
                (*exported).shape.as_mut_ptr()
            } else {
                std::ptr::null_mut()
            };
            (*view).strides = if flags & ffi::PyBUF_STRIDES == ffi::PyBUF_STRIDES {
                (*exported).strides.as_mut_ptr()
            } else {
                std::ptr::null_mut()
            };
            (*view).suboffsets = std::ptr::null_mut();
            (*view).internal = exported as *mut c_void;
            (*view).obj = slf.into_ptr();
        }
        Ok(())
    }

    unsafe fn __releasebuffer__(&self, view: *mut ffi::Py_buffer) {
        // Safety: `internal` is the box that `__getbuffer__` leaked into this view.
        drop(unsafe { Box::from_raw((*view).internal as *mut ExportedPixels) });
    }
}

impl From<slint_interpreter::Image> for PyImage {
    fn from(image: slint_interpreter::Image) -> Self {
        Self { image: image }
    }
}
