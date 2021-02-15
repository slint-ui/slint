/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use std::cell::RefCell;
use std::rc::Rc;

use sixtyfps_corelib::{graphics::Size, slice::Slice, Property, Resource, SharedString};

use super::{CanvasRc, GLItemRenderer, GLRendererData, ItemGraphicsCacheEntry};

struct Texture {
    id: femtovg::ImageId,
    canvas: CanvasRc,
    /// If present, this boolean property indicates whether the image has been uploaded yet or
    /// if that operation is still pending. If not present, then the image *is* available. This is
    /// used for remote HTML image loading and the property will be used to correctly track dependencies
    /// to graphics items that query for the size.
    upload_pending: Option<core::pin::Pin<Box<Property<bool>>>>,
}

impl Texture {
    fn size(&self) -> Option<Size> {
        if self
            .upload_pending
            .as_ref()
            .map_or(false, |pending_property| pending_property.as_ref().get())
        {
            None
        } else {
            self.canvas
                .borrow()
                .image_info(self.id)
                .map(|info| [info.width() as f32, info.height() as f32].into())
                .ok()
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        self.canvas.borrow_mut().delete_image(self.id);
    }
}

#[derive(derive_more::From)]
enum ImageData {
    Texture(Texture),
    DecodedImage(image::DynamicImage),
    #[cfg(feature = "svg")]
    SVG(usvg::Tree),
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use image::GenericImageView;
        match self {
            ImageData::Texture(t) => {
                write!(f, "ImageData::Texture({:?})", t.id.0)
            }
            ImageData::DecodedImage(i) => {
                write!(f, "ImageData::DecodedImage({}x{})", i.width(), i.height())
            }
            ImageData::SVG(_) => {
                write!(f, "ImageData::SVG(...)")
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct CachedImage(RefCell<ImageData>);

impl CachedImage {
    fn new_on_cpu(decoded_image: image::DynamicImage) -> Self {
        Self(RefCell::new(ImageData::DecodedImage(decoded_image)))
    }

    pub fn new_on_gpu(
        canvas: &CanvasRc,
        image_id: femtovg::ImageId,
        upload_pending_notifier: Option<core::pin::Pin<Box<Property<bool>>>>,
    ) -> Self {
        Self(RefCell::new(
            Texture {
                id: image_id,
                canvas: canvas.clone(),
                upload_pending: upload_pending_notifier,
            }
            .into(),
        ))
    }

    #[cfg(feature = "svg")]
    fn new_on_cpu_svg(tree: usvg::Tree) -> Self {
        Self(RefCell::new(ImageData::SVG(tree)))
    }

    pub fn new_from_resource(resource: &Resource, renderer: &GLRendererData) -> Option<Rc<Self>> {
        match resource {
            Resource::None => None,
            Resource::AbsoluteFilePath(path) => Self::new_from_path(path, renderer),
            Resource::EmbeddedData(data) => Self::new_from_data(data).map(Rc::new),
            Resource::EmbeddedRgbaImage { .. } => todo!(),
        }
    }

    fn new_from_path(path: &SharedString, _renderer: &GLRendererData) -> Option<Rc<Self>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            #[cfg(feature = "svg")]
            if path.ends_with(".svg") {
                return Some(Rc::new(Self::new_on_cpu_svg(
                    super::svg::load_from_path(std::path::Path::new(&path.as_str())).map_or_else(
                        |err| {
                            eprintln!("Error loading SVG from {}: {}", &path, err);
                            None
                        },
                        |svg_tree| Some(svg_tree),
                    )?,
                )));
            }
            Some(Rc::new(Self::new_on_cpu(
                image::open(std::path::Path::new(&path.as_str())).map_or_else(
                    |decode_err| {
                        eprintln!("Error loading image from {}: {}", &path, decode_err);
                        None
                    },
                    |image| Some(image),
                )?,
            )))
        }
        #[cfg(target_arch = "wasm32")]
        Some(Self::load_html_image(&path, _renderer))
    }

    fn new_from_data(data: &Slice<u8>) -> Option<Self> {
        #[cfg(feature = "svg")]
        if data.starts_with(b"<svg") {
            return Some(CachedImage::new_on_cpu_svg(
                super::svg::load_from_data(data.as_slice()).map_or_else(
                    |svg_err| {
                        eprintln!("Error loading SVG: {}", svg_err);
                        None
                    },
                    |svg_tree| Some(svg_tree),
                )?,
            ));
        }
        Some(CachedImage::new_on_cpu(image::load_from_memory(data.as_slice()).map_or_else(
            |decode_err| {
                eprintln!("Error decoding image: {}", decode_err);
                None
            },
            |decoded_image| Some(decoded_image),
        )?))
    }

    #[cfg(target_arch = "wasm32")]
    fn load_html_image(url: &str, renderer: &GLRendererData) -> Rc<CachedImage> {
        let image_id = renderer
            .canvas
            .borrow_mut()
            .create_image_empty(1, 1, femtovg::PixelFormat::Rgba8, femtovg::ImageFlags::empty())
            .unwrap();

        let cached_image = Rc::new(Self::new_on_gpu(
            &renderer.canvas,
            image_id,
            Some(Box::pin(/*upload pending*/ Property::new(true))),
        ));

        let html_image = web_sys::HtmlImageElement::new().unwrap();
        html_image.set_cross_origin(Some("anonymous"));
        html_image.set_onload(Some(
            &wasm_bindgen::closure::Closure::once_into_js({
                let canvas_weak = Rc::downgrade(&renderer.canvas);
                let html_image = html_image.clone();
                let image_id = image_id.clone();
                let window_weak = Rc::downgrade(&renderer.window);
                let cached_image_weak = Rc::downgrade(&cached_image);
                let event_loop_proxy_weak = Rc::downgrade(&renderer.event_loop_proxy);
                move || {
                    let (canvas, window, event_loop_proxy, cached_image) = match (
                        canvas_weak.upgrade(),
                        window_weak.upgrade(),
                        event_loop_proxy_weak.upgrade(),
                        cached_image_weak.upgrade(),
                    ) {
                        (
                            Some(canvas),
                            Some(window),
                            Some(event_loop_proxy),
                            Some(cached_image),
                        ) => (canvas, window, event_loop_proxy, cached_image),
                        _ => return,
                    };
                    canvas
                        .borrow_mut()
                        .realloc_image(
                            image_id,
                            html_image.width() as usize,
                            html_image.height() as usize,
                            femtovg::PixelFormat::Rgba8,
                            femtovg::ImageFlags::empty(),
                        )
                        .unwrap();
                    canvas.borrow_mut().update_image(image_id, &html_image.into(), 0, 0).unwrap();

                    cached_image.notify_loaded();

                    // As you can paint on a HTML canvas at any point in time, request_redraw()
                    // on a winit window only queues an additional internal event, that'll be
                    // be dispatched as the next event. We are however not in an event loop
                    // call, so we also need to wake up the event loop.
                    window.request_redraw();
                    event_loop_proxy.send_event(crate::eventloop::CustomEvent::WakeUpAndPoll).ok();
                }
            })
            .into(),
        ));
        html_image.set_src(&url);

        cached_image
    }

    // Upload the image to the GPU? if that hasn't happened yet. This function could take just a canvas
    // as parameter, but since an upload requires a current context, this is "enforced" by taking
    // a renderer instead (which implies a current context).
    pub fn ensure_uploaded_to_gpu(&self, current_renderer: &GLItemRenderer) -> femtovg::ImageId {
        use std::convert::TryFrom;

        let canvas = &current_renderer.shared_data.canvas;

        let img = &mut *self.0.borrow_mut();
        if let ImageData::DecodedImage(decoded_image) = img {
            let image_id = match femtovg::ImageSource::try_from(&*decoded_image) {
                Ok(image_source) => {
                    canvas.borrow_mut().create_image(image_source, femtovg::ImageFlags::empty())
                }
                Err(_) => {
                    let converted = image::DynamicImage::ImageRgba8(decoded_image.to_rgba8());
                    let image_source = femtovg::ImageSource::try_from(&converted).unwrap();
                    canvas.borrow_mut().create_image(image_source, femtovg::ImageFlags::empty())
                }
            }
            .unwrap();

            *img = Texture { id: image_id, canvas: canvas.clone(), upload_pending: None }.into()
        };

        match &img {
            ImageData::Texture(Texture { id, .. }) => *id,
            _ => unreachable!(),
        }
    }

    pub fn size(&self) -> Option<Size> {
        use image::GenericImageView;

        match &*self.0.borrow() {
            ImageData::Texture(texture) => texture.size(),
            ImageData::DecodedImage(decoded_image) => {
                let (width, height) = decoded_image.dimensions();
                Some([width as f32, height as f32].into())
            }

            #[cfg(feature = "svg")]
            ImageData::SVG(tree) => {
                let size = tree.svg_node().size.to_screen_size();
                Some([size.width() as f32, size.height() as f32].into())
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn notify_loaded(&self) {
        if let ImageData::Texture(Texture { upload_pending, .. }) = &*self.0.borrow() {
            upload_pending.as_ref().map(|pending_property| {
                pending_property.as_ref().set(false);
            });
        }
    }

    pub fn as_renderable(
        self: Rc<Self>,
        target_size: euclid::default::Size2D<u32>,
    ) -> Option<ItemGraphicsCacheEntry> {
        Some(match &*self.0.borrow() {
            ImageData::Texture { .. } => ItemGraphicsCacheEntry::Image(self.clone()),
            ImageData::DecodedImage(_) => ItemGraphicsCacheEntry::Image(self.clone()),
            #[cfg(feature = "svg")]
            ImageData::SVG(svg_tree) => match super::svg::render(&svg_tree, target_size) {
                Ok(rendered_svg_image) => ItemGraphicsCacheEntry::ScalableImage {
                    scalable_source: self.clone(),
                    scaled_image: Rc::new(Self::new_on_cpu(rendered_svg_image)),
                },
                Err(err) => {
                    eprintln!("Error rendering SVG: {}", err);
                    return None;
                }
            },
        })
    }
}
