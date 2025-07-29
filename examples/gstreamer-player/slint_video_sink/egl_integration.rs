// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::sync::{Arc, Mutex};

use futures::channel::mpsc::UnboundedSender;
use gst::prelude::*;
use gst_gl::prelude::*;

pub fn init<App: slint::ComponentHandle + 'static>(
    app: &App,
    pipeline: &gst::Pipeline,
    new_frame_callback: fn(App, slint::Image),
    bus_sender: UnboundedSender<gst::Message>,
) -> gst::Element {
    let mut slint_sink = SlintOpenGLSink::new();
    let sink_element = slint_sink.element();
    pipeline.set_property("video-sink", &sink_element);

    app.window()
        .set_rendering_notifier({
            let pipeline = pipeline.clone();
            let app_weak = app.as_weak();

            move |state, graphics_api| match state {
                slint::RenderingState::RenderingSetup => {
                    let app_weak = app_weak.clone();
                    slint_sink.connect(
                        graphics_api,
                        &pipeline.bus().unwrap(),
                        bus_sender.clone(),
                        Box::new(move || {
                            app_weak
                                .upgrade_in_event_loop(move |app| {
                                    app.window().request_redraw();
                                })
                                .ok();
                        }),
                    );
                    pipeline
                        .set_state(gst::State::Playing)
                        .expect("Unable to set the pipeline to the `Playing` state");
                }
                slint::RenderingState::RenderingTeardown => {
                    slint_sink.deactivate_and_pause();
                }
                slint::RenderingState::BeforeRendering => {
                    if let Some(next_frame) = slint_sink.fetch_next_frame() {
                        new_frame_callback(app_weak.unwrap(), next_frame)
                    }
                }
                _ => {}
            }
        })
        .unwrap();

    sink_element
}

pub struct SlintOpenGLSink {
    appsink: gst_app::AppSink,
    glsink: gst::Element,
    next_frame: Arc<Mutex<Option<(gst_video::VideoInfo, gst::Buffer)>>>,
    current_frame: Mutex<Option<gst_gl::GLVideoFrame<gst_gl::gl_video_frame::Readable>>>,
    gst_gl_context: Option<gst_gl::GLContext>,
}

impl SlintOpenGLSink {
    pub fn new() -> Self {
        let appsink = gst_app::AppSink::builder()
            .caps(
                &gst_video::VideoCapsBuilder::new()
                    .features([gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY])
                    .format(gst_video::VideoFormat::Rgba)
                    .field("texture-target", "2D")
                    .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
                    .build(),
            )
            .enable_last_sample(false)
            .max_buffers(1u32)
            .build();

        let glsink = gst::ElementFactory::make("glsinkbin")
            .property("sink", &appsink)
            .build()
            .expect("Fatal: Unable to create glsink");

        Self {
            appsink,
            glsink,
            next_frame: Default::default(),
            current_frame: Default::default(),
            gst_gl_context: None,
        }
    }

    pub fn element(&self) -> gst::Element {
        self.glsink.clone()
    }

    pub fn connect(
        &mut self,
        graphics_api: &slint::GraphicsAPI<'_>,
        bus: &gst::Bus,
        bus_sender: UnboundedSender<gst::Message>,
        next_frame_available_notifier: Box<dyn Fn() + Send>,
    ) {
        let egl = match graphics_api {
            slint::GraphicsAPI::NativeOpenGL { get_proc_address } => {
                glutin_egl_sys::egl::Egl::load_with(|symbol| {
                    get_proc_address(&std::ffi::CString::new(symbol).unwrap())
                })
            }
            _ => panic!("unsupported graphics API"),
        };

        let (gst_gl_context, gst_gl_display) = unsafe {
            let platform = gst_gl::GLPlatform::EGL;

            let egl_display = egl.GetCurrentDisplay();
            let display = gst_gl_egl::GLDisplayEGL::with_egl_display(egl_display as usize).unwrap();
            let native_context = egl.GetCurrentContext();

            (
                gst_gl::GLContext::new_wrapped(
                    &display,
                    native_context as _,
                    platform,
                    gst_gl::GLContext::current_gl_api(platform).0,
                )
                .expect("unable to create wrapped GL context"),
                display,
            )
        };

        gst_gl_context.activate(true).expect("could not activate GStreamer GL context");
        gst_gl_context.fill_info().expect("failed to fill GL info for wrapped context");

        self.gst_gl_context = Some(gst_gl_context.clone());

        bus.set_sync_handler({
            let gst_gl_context = gst_gl_context.clone();
            move |_, msg| {
                match msg.view() {
                    gst::MessageView::NeedContext(ctx) => {
                        let ctx_type = ctx.context_type();
                        if ctx_type == *gst_gl::GL_DISPLAY_CONTEXT_TYPE {
                            if let Some(element) =
                                msg.src().and_then(|source| source.downcast_ref::<gst::Element>())
                            {
                                let gst_context = gst::Context::new(ctx_type, true);
                                gst_context.set_gl_display(&gst_gl_display);
                                element.set_context(&gst_context);
                            }
                        } else if ctx_type == "gst.gl.app_context" {
                            if let Some(element) =
                                msg.src().and_then(|source| source.downcast_ref::<gst::Element>())
                            {
                                let mut gst_context = gst::Context::new(ctx_type, true);
                                {
                                    let gst_context = gst_context.get_mut().unwrap();
                                    let structure = gst_context.structure_mut();
                                    structure.set("context", &gst_gl_context);
                                }
                                element.set_context(&gst_context);
                            }
                        }
                    }
                    _ => {
                        let _ = bus_sender.unbounded_send(msg.to_owned());
                    }
                }

                gst::BusSyncReply::Drop
            }
        });

        let next_frame_ref = self.next_frame.clone();

        self.appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Flushing)?;

                    let mut buffer = sample.buffer_owned().unwrap();
                    {
                        let context = match (buffer.n_memory() > 0)
                            .then(|| buffer.peek_memory(0))
                            .and_then(|m| m.downcast_memory_ref::<gst_gl::GLBaseMemory>())
                            .map(|m| m.context())
                        {
                            Some(context) => context.clone(),
                            None => {
                                eprintln!("Got non-GL memory");
                                return Err(gst::FlowError::Error);
                            }
                        };

                        // Sync point to ensure that the rendering in this context will be complete by the time the
                        // Slint created GL context needs to access the texture.
                        if let Some(meta) = buffer.meta::<gst_gl::GLSyncMeta>() {
                            meta.set_sync_point(&context);
                        } else {
                            let buffer = buffer.make_mut();
                            let meta = gst_gl::GLSyncMeta::add(buffer, &context);
                            meta.set_sync_point(&context);
                        }
                    }

                    let Some(info) =
                        sample.caps().and_then(|caps| gst_video::VideoInfo::from_caps(caps).ok())
                    else {
                        eprintln!("Got invalid caps");
                        return Err(gst::FlowError::NotNegotiated);
                    };

                    let next_frame_ref = next_frame_ref.clone();
                    *next_frame_ref.lock().unwrap() = Some((info, buffer));

                    next_frame_available_notifier();

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    pub fn fetch_next_frame(&self) -> Option<slint::Image> {
        if let Some((info, buffer)) = self.next_frame.lock().unwrap().take() {
            let sync_meta = buffer.meta::<gst_gl::GLSyncMeta>().unwrap();
            sync_meta.wait(self.gst_gl_context.as_ref().unwrap());

            if let Ok(frame) = gst_gl::GLVideoFrame::from_buffer_readable(buffer, &info) {
                *self.current_frame.lock().unwrap() = Some(frame);
            }
        }

        self.current_frame
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|frame| {
                frame
                    .texture_id(0)
                    .ok()
                    .and_then(|id| id.try_into().ok())
                    .map(|texture| (frame, texture))
            })
            .map(|(frame, texture)| unsafe {
                slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                    texture,
                    [frame.width(), frame.height()].into(),
                )
                .build()
            })
    }

    pub fn deactivate_and_pause(&self) {
        self.current_frame.lock().unwrap().take();
        self.next_frame.lock().unwrap().take();

        if let Some(context) = &self.gst_gl_context {
            context.activate(false).expect("could not activate GStreamer GL context");
        }
    }
}
