// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};
use slint_interpreter::SharedPixelBuffer;

/// Image objects can be set on Slint Image elements for display. Use `Image.load_from_path` to construct Image
/// objects from a path to an image file on disk.
#[gen_stub_pyclass]
#[pyclass(unsendable, name = "Image")]
pub struct PyImage {
    pub image: slint_interpreter::Image,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        Ok(Self { image: Default::default() })
    }

    /// The size of the image as tuple of `width` and `height`.
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

    /// The path of the image if it was loaded from disk, or None.
    #[getter]
    fn path(&self) -> PyResult<Option<std::path::PathBuf>> {
        Ok(self.image.path().map(|p| p.to_path_buf()))
    }

    /// Loads the image from the specified path. Returns None if the image can't be loaded.
    #[staticmethod]
    fn load_from_path(path: std::path::PathBuf) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_path(&path)?;
        Ok(Self { image })
    }

    /// Creates a new image from a string that describes the image in SVG format.
    #[staticmethod]
    fn load_from_svg_data(data: Vec<u8>) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_svg_data(&data)?;
        Ok(Self { image })
    }

    /// Creates a new image from an array-like object that implements the [Buffer Protocol](https://docs.python.org/3/c-api/buffer.html).
    /// Use this function to import images created by third-party modules such as matplotlib or Pillow.
    ///
    /// The array must satisfy certain contraints to represent an image:
    ///
    ///  - The buffer's format needs to be `B` (unsigned char)
    ///  - The shape must be a tuple of (height, width, bytes-per-pixel)
    ///  - If a stride is defined, the row stride must be equal to width * bytes-per-pixel, and the column stride must equal the bytes-per-pixel.
    ///  - A value of 3 for bytes-per-pixel is interpreted as RGB image, a value of 4 means RGBA.
    ///
    /// The image is created by performing a deep copy of the array's data. Subsequent changes to the buffer are not automatically
    /// reflected in a previously created Image.
    ///
    /// Example of importing a matplot figure into an image:
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
                    )))
                }
            },
        })
    }
}

impl From<slint_interpreter::Image> for PyImage {
    fn from(image: slint_interpreter::Image) -> Self {
        Self { image: image }
    }
}
