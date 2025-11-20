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
//! # Example
//!
//! ```rust,no_run
//! use slint::ComponentHandle;
//! use crate::webview::WebView;
//!
//! // Create Slint application
//! let app = MyApp::new().unwrap();
//!
//! // Initialize WGPU for GPU rendering (non-Android platforms)
//! # #[cfg(not(target_os = "android"))]
//! let (device, queue) = setup_wgpu();
//!
//! // Create WebView instance
//! # #[cfg(not(target_os = "android"))]
//! WebView::new(
//!     app.clone_strong(),
//!     "https://example.com".into(),
//!     device,
//!     queue,
//! );
//!
//! // Run the application
//! app.run().unwrap();
//! ```
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

mod adapter;
mod delegate;
mod rendering_context;
mod waker;
mod webview;
mod webview_events;

pub use waker::Waker;
pub use webview::WebView;
