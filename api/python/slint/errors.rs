// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::PyErr;

pub struct PyGetPropertyError(pub slint_interpreter::GetPropertyError);

impl From<PyGetPropertyError> for PyErr {
    fn from(err: PyGetPropertyError) -> Self {
        pyo3::exceptions::PyValueError::new_err(err.0.to_string())
    }
}

impl From<slint_interpreter::GetPropertyError> for PyGetPropertyError {
    fn from(err: slint_interpreter::GetPropertyError) -> Self {
        Self(err)
    }
}

pub struct PySetPropertyError(pub slint_interpreter::SetPropertyError);

impl From<PySetPropertyError> for PyErr {
    fn from(err: PySetPropertyError) -> Self {
        pyo3::exceptions::PyValueError::new_err(err.0.to_string())
    }
}

impl From<slint_interpreter::SetPropertyError> for PySetPropertyError {
    fn from(err: slint_interpreter::SetPropertyError) -> Self {
        Self(err)
    }
}

pub struct PyPlatformError(pub slint_interpreter::PlatformError);

impl From<PyPlatformError> for PyErr {
    fn from(err: PyPlatformError) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(err.0.to_string())
    }
}

impl From<slint_interpreter::PlatformError> for PyPlatformError {
    fn from(err: slint_interpreter::PlatformError) -> Self {
        Self(err)
    }
}

pub struct PyEventLoopError(pub slint_interpreter::EventLoopError);

impl From<PyEventLoopError> for PyErr {
    fn from(err: PyEventLoopError) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(err.0.to_string())
    }
}

impl From<slint_interpreter::EventLoopError> for PyEventLoopError {
    fn from(err: slint_interpreter::EventLoopError) -> Self {
        Self(err)
    }
}

pub struct PyInvokeError(pub slint_interpreter::InvokeError);

impl From<PyInvokeError> for PyErr {
    fn from(err: PyInvokeError) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(err.0.to_string())
    }
}

impl From<slint_interpreter::InvokeError> for PyInvokeError {
    fn from(err: slint_interpreter::InvokeError) -> Self {
        Self(err)
    }
}

pub struct PySetCallbackError(pub slint_interpreter::SetCallbackError);

impl From<PySetCallbackError> for PyErr {
    fn from(err: PySetCallbackError) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(err.0.to_string())
    }
}

impl From<slint_interpreter::SetCallbackError> for PySetCallbackError {
    fn from(err: slint_interpreter::SetCallbackError) -> Self {
        Self(err)
    }
}

pub struct PyLoadImageError(pub slint_interpreter::LoadImageError);

impl From<PyLoadImageError> for PyErr {
    fn from(err: PyLoadImageError) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(err.0.to_string())
    }
}

impl From<slint_interpreter::LoadImageError> for PyLoadImageError {
    fn from(err: slint_interpreter::LoadImageError) -> Self {
        Self(err)
    }
}

pub struct PyColorParseError(pub css_color_parser2::ColorParseError);

impl From<PyColorParseError> for PyErr {
    fn from(err: PyColorParseError) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(err.0.to_string())
    }
}

impl From<css_color_parser2::ColorParseError> for PyColorParseError {
    fn from(err: css_color_parser2::ColorParseError) -> Self {
        Self(err)
    }
}
