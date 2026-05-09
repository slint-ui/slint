// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! WebView integration module for embedding Servo browser engine in Slint applications.
//!
//! This module provides a reusable `WebView` component that integrates the Servo browser engine
//! with Slint UI framework. It handles the complex bridging between Servo's rendering pipeline
//! and Slint's display system.
//!
//! # Architecture
//!
//! The module is organized into several key components:
//!
//! - **`WebView`**: Main public API for creating and managing a web browser instance
//! - **`adapter`**: Bridge between Slint UI and Servo engine, managing state and communication
//! - **`delegate`**: Servo callback handler for frame updates and rendering notifications
//! - **`rendering_context`**: Platform-specific rendering backends (GPU/software)
//! - **`waker`**: Event loop integration for async Servo operations
//! - **`webview_events`**: UI event handlers for user interactions (clicks, scrolls, etc.)
//!
//! # Platform Support
//!
//! - **Desktop (Linux, macOS)**: GPU-accelerated rendering via WGPU
//! - **Android**: Software rendering fallback
//!
//! # Threading Model
//!
//! The WebView runs Servo's event loop asynchronously using `slint::spawn_local()`.
//! All UI interactions are marshaled through async channels to maintain thread safety.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::cell::Cell;
//! use slint::ComponentHandle;
//! use crate::webview::WebView;
//!
//! pub fn main() {
//!     // Let Slint create the wgpu instance so that backend-specific
//!     // requirements (e.g. DRM display extensions on linuxkms) are honored.
//!     slint::BackendSelector::new()
//!         .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::Automatic(
//!             slint::wgpu_28::WGPUSettings::default(),
//!         ))
//!         .select()
//!         .unwrap();
//!
//!     let app = MyApp::new().unwrap();
//!
//!     let initialized = Cell::new(false);
//!     let app_weak = app.as_weak();
//!
//!     app.window().set_rendering_notifier(move |state, graphics_api| {
//!         if !matches!(state, slint::RenderingState::RenderingSetup) || initialized.get() {
//!             return;
//!         }
//!         let slint::GraphicsAPI::WGPU28 { device, queue, .. } = graphics_api else {
//!             return;
//!         };
//!         let app = app_weak.upgrade().unwrap();
//!         WebView::new(app, "https://example.com".into(), device.clone(), queue.clone());
//!         initialized.set(true);
//!     }).unwrap();
//!
//!     app.run().unwrap();
//! }
//! ```

mod adapter;
mod delegate;
mod events_utils;
mod rendering_context;
mod waker;
mod webview;
mod webview_events;

pub use adapter::SlintServoAdapter;
pub use delegate::AppDelegate;
pub use rendering_context::ServoRenderingAdapter;
pub use waker::Waker;
pub use webview::WebView;
pub use webview_events::WebViewEvents;
