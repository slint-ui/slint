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
//! use slint::ComponentHandle;
//! use crate::webview::WebView;
//!
//! pub fn main() {
//! // Create Slint application
//! let app = MyApp::new().unwrap();
//!
//! // Initialize WGPU for GPU rendering (non-Android platforms)
//! let (device, queue) = setup_wgpu();
//!
//! // Create WebView instance
//! WebView::new(
//!     app.clone_strong(),
//!     "https://example.com".into(),
//!     device,
//!     queue,
//! );
//!
//! // Run the application
//! app.run().unwrap();
//! }
//!
//! fn setup_wgpu() -> (wgpu::Device, wgpu::Queue) {
//!     let backends = wgpu::Backends::from_env().unwrap_or_default();

//!     let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
//!         backends,
//!         flags: Default::default(),
//!         backend_options: Default::default(),
//!         memory_budget_thresholds: Default::default(),
//!     });
//!
//!     let adapter = spin_on::spin_on(async {
//!         instance
//!             .request_adapter(&Default::default())
//!             .await
//!             .unwrap()
//!     });
//!
//!     let (device, queue) = spin_on::spin_on(async {
//!         adapter.request_device(&Default::default()).await.unwrap()
//!     });
//!
//!     slint::BackendSelector::new()
//!         .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::Manual {
//!             instance,
//!             adapter,
//!             device: device.clone(),
//!             queue: queue.clone()
//!         })
//!         .select()
//!         .unwrap();
//!
//!     (device, queue)
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
