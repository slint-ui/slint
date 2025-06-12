// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API,
in particular the `BackendSelector` type.
*/

use alloc::boxed::Box;
use alloc::{format, string::String};

use i_slint_core::api::PlatformError;
use i_slint_core::graphics::{RequestedGraphicsAPI, RequestedOpenGLVersion};

#[i_slint_core_macros::slint_doc]
/// Use the BackendSelector to configure one of Slint's built-in [backends with a renderer](slint:backends_and_renderers)
/// to accommodate specific needs of your application. This is a programmatic substitute for
/// the `SLINT_BACKEND` environment variable.
///
/// For example, to configure Slint to use a renderer that supports OpenGL ES 3.0, configure
/// the `BackendSelector` as follows:
/// ```rust,no_run
/// # use i_slint_backend_selector::api::BackendSelector;
/// let selector = BackendSelector::new().require_opengl_es_with_version(3, 0);
/// if let Err(err) = selector.select() {
///     eprintln!("Error selecting backend with OpenGL ES support: {err}");
/// }
/// ```
#[derive(Default)]
pub struct BackendSelector {
    requested_graphics_api: Option<RequestedGraphicsAPI>,
    backend: Option<String>,
    renderer: Option<String>,
    selected: bool,
    #[cfg(feature = "unstable-winit-030")]
    winit_window_attributes_hook: Option<
        Box<
            dyn Fn(
                i_slint_backend_winit::winit::window::WindowAttributes,
            ) -> i_slint_backend_winit::winit::window::WindowAttributes,
        >,
    >,
    #[cfg(feature = "unstable-winit-030")]
    winit_event_loop_builder: Option<i_slint_backend_winit::EventLoopBuilder>,
}

impl BackendSelector {
    /// Creates a new BackendSelector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the requirement to the selector that the backend must render with OpenGL ES
    /// and the specified major and minor version.
    #[must_use]
    pub fn require_opengl_es_with_version(mut self, major: u8, minor: u8) -> Self {
        self.requested_graphics_api =
            Some(RequestedOpenGLVersion::OpenGLES(Some((major, minor))).into());
        self
    }

    /// Adds the requirement to the selector that the backend must render with OpenGL ES.
    #[must_use]
    pub fn require_opengl_es(mut self) -> Self {
        self.requested_graphics_api = Some(RequestedOpenGLVersion::OpenGLES(None).into());
        self
    }

    /// Adds the requirement to the selector that the backend must render with OpenGL.
    #[must_use]
    pub fn require_opengl(mut self) -> Self {
        self.requested_graphics_api = Some(RequestedOpenGLVersion::OpenGL(None).into());
        self
    }

    /// Adds the requirement to the selector that the backend must render with OpenGL
    /// and the specified major and minor version.
    #[must_use]
    pub fn require_opengl_with_version(mut self, major: u8, minor: u8) -> Self {
        self.requested_graphics_api =
            Some(RequestedOpenGLVersion::OpenGL(Some((major, minor))).into());
        self
    }

    /// Adds the requirement to the selector that the backend must render with Apple's Metal framework.
    #[must_use]
    pub fn require_metal(mut self) -> Self {
        self.requested_graphics_api = Some(RequestedGraphicsAPI::Metal);
        self
    }

    /// Adds the requirement to the selector that the backend must render with Vulkan.
    #[must_use]
    pub fn require_vulkan(mut self) -> Self {
        self.requested_graphics_api = Some(RequestedGraphicsAPI::Vulkan);
        self
    }

    /// Adds the requirement to the selector that the backend must render with Direct 3D.
    #[must_use]
    pub fn require_d3d(mut self) -> Self {
        self.requested_graphics_api = Some(RequestedGraphicsAPI::Direct3D);
        self
    }

    #[i_slint_core_macros::slint_doc]
    /// Adds the requirement to the selector that the backend must render using [WGPU](http://wgpu.rs).
    /// Use this when you integrate other WGPU-based renderers with a Slint UI.
    ///
    /// *Note*: This function is behind the [`unstable-wgpu-24` feature flag](slint:rust:slint/docs/cargo_features/#backends)
    ///         and may be removed or changed in future minor releases, as new major WGPU releases become available.
    ///
    /// See also the [`slint::wgpu_24`](slint:rust:slint/wgpu_24) module.
    #[cfg(feature = "unstable-wgpu-24")]
    #[must_use]
    pub fn require_wgpu_24(
        mut self,
        configuration: i_slint_core::graphics::wgpu_24::WGPUConfiguration,
    ) -> Self {
        self.requested_graphics_api = Some(RequestedGraphicsAPI::WGPU24(configuration));
        self
    }

    #[i_slint_core_macros::slint_doc]
    /// Configures this builder to use the specified winit hook that will be called before a Window is created.
    ///
    /// It can be used to adjust settings of window that will be created.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let mut backend = slint::BackendSelector::new()
    ///     .with_winit_window_attributes_hook(|attributes| attributes.with_content_protected(true))
    ///     .select()
    ///     .unwrap();
    /// ```
    ///
    /// *Note*: This function is behind the [`unstable-winit-030` feature flag](slint:rust:slint/docs/cargo_features/#backends)
    ///         and may be removed or changed in future minor releases, as new major Winit releases become available.
    ///
    /// See also the [`slint::winit_030`](slint:rust:slint/winit_030) module
    #[must_use]
    #[cfg(feature = "unstable-winit-030")]
    pub fn with_winit_window_attributes_hook(
        mut self,
        hook: impl Fn(
                i_slint_backend_winit::winit::window::WindowAttributes,
            ) -> i_slint_backend_winit::winit::window::WindowAttributes
            + 'static,
    ) -> Self {
        self.winit_window_attributes_hook = Some(Box::new(hook));
        self
    }

    #[i_slint_core_macros::slint_doc]
    /// Configures this builder to use the specified winit event loop builder when creating the event
    /// loop.
    ///
    /// *Note*: This function is behind the [`unstable-winit-030` feature flag](slint:rust:slint/docs/cargo_features/#backends)
    ///         and may be removed or changed in future minor releases, as new major Winit releases become available.
    ///
    /// See also the [`slint::winit_030`](slint:rust:slint/winit_030) module
    #[must_use]
    #[cfg(feature = "unstable-winit-030")]
    pub fn with_winit_event_loop_builder(
        mut self,
        event_loop_builder: i_slint_backend_winit::EventLoopBuilder,
    ) -> Self {
        self.winit_event_loop_builder = Some(event_loop_builder);
        self
    }

    /// Adds the requirement that the selected renderer must match the given name. This is
    /// equivalent to setting the `SLINT_BACKEND=name` environment variable and requires
    /// that the corresponding renderer feature is enabled. For example, to select the Skia renderer,
    /// enable the `renderer-skia` feature and call this function with `skia` as argument.
    #[must_use]
    pub fn renderer_name(mut self, name: String) -> Self {
        self.renderer = Some(name);
        self
    }

    /// Adds the requirement that the selected backend must match the given name. This is
    /// equivalent to setting the `SLINT_BACKEND=name` environment variable and requires
    /// that the corresponding backend feature is enabled. For example, to select the winit backend,
    /// enable the `backend-winit` feature and call this function with `winit` as argument.
    #[must_use]
    pub fn backend_name(mut self, name: String) -> Self {
        self.backend = Some(name);
        self
    }

    /// Completes the backend selection process and tries to combine with specified requirements
    /// with the different backends and renderers enabled at compile time. On success, the selected
    /// backend is automatically set to be active. Returns an error if the requirements could not be met.
    pub fn select(mut self) -> Result<(), PlatformError> {
        self.select_internal()
    }

    fn select_internal(&mut self) -> Result<(), PlatformError> {
        self.selected = true;

        #[cfg(any(
            feature = "i-slint-backend-qt",
            feature = "i-slint-backend-winit",
            feature = "i-slint-backend-linuxkms"
        ))]
        if self.backend.is_none() || self.renderer.is_none() {
            let backend_config = std::env::var("SLINT_BACKEND").unwrap_or_default();
            let backend_config = backend_config.to_lowercase();
            let (backend, renderer) = super::parse_backend_env_var(backend_config.as_str());
            if !backend.is_empty() {
                self.backend.get_or_insert_with(|| backend.to_owned());
            }
            if !renderer.is_empty() {
                self.renderer.get_or_insert_with(|| renderer.to_owned());
            }
        }

        let backend_name = self.backend.as_deref().unwrap_or_else(|| {
            // Only the winit backend supports graphics API requests right now, so prefer that over
            // aborting.
            #[cfg(feature = "i-slint-backend-winit")]
            if self.requested_graphics_api.is_some() {
                return "winit";
            }
            super::DEFAULT_BACKEND_NAME
        });

        let backend: Box<dyn i_slint_core::platform::Platform> = match backend_name {
            #[cfg(all(feature = "i-slint-backend-linuxkms", target_os = "linux"))]
            "linuxkms" => {
                if self.requested_graphics_api.is_some() {
                    return Err("The linuxkms backend does not implement renderer selection by graphics API".into());
                }

                Box::new(i_slint_backend_linuxkms::Backend::new_with_renderer_by_name(
                    self.renderer.as_deref(),
                )?)
            }
            #[cfg(feature = "i-slint-backend-winit")]
            "winit" => {
                let builder = i_slint_backend_winit::Backend::builder();

                let builder = match self.requested_graphics_api.as_ref() {
                    Some(api) => builder.request_graphics_api(api.clone()),
                    None => builder,
                };

                let builder = match self.renderer.as_ref() {
                    Some(name) => builder.with_renderer_name(name),
                    None => builder,
                };

                #[cfg(feature = "unstable-winit-030")]
                let builder = match self.winit_window_attributes_hook.take() {
                    Some(hook) => builder.with_window_attributes_hook(hook),
                    None => builder,
                };

                #[cfg(feature = "unstable-winit-030")]
                let builder = match self.winit_event_loop_builder.take() {
                    Some(event_loop_builder) => builder.with_event_loop_builder(event_loop_builder),
                    None => builder,
                };

                Box::new(builder.build()?)
            }
            #[cfg(feature = "i-slint-backend-qt")]
            "qt" => {
                if self.requested_graphics_api.is_some() {
                    return Err(
                        "The qt backend does not implement renderer selection by graphics API"
                            .into(),
                    );
                }
                if self.renderer.is_some() {
                    return Err(
                        "The qt backend does not implement renderer selection by name".into()
                    );
                }
                Box::new(i_slint_backend_qt::Backend::new())
            }
            requested_backend => {
                return Err(format!(
                    "{requested_backend} backend requested but it is not available"
                )
                .into());
            }
        };

        i_slint_core::platform::set_platform(backend).map_err(PlatformError::SetPlatformError)
    }
}

impl Drop for BackendSelector {
    fn drop(&mut self) {
        if !self.selected {
            self.select_internal().unwrap();
        }
    }
}
