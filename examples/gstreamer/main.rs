// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use gstreamer::prelude::*;
use gstreamer_gl::prelude::*;

slint::include_modules!();

struct Player<C: slint::ComponentHandle + 'static> {
    app: slint::Weak<C>,
    //pipeline: gstreamer::Pipeline,
    pipeline: gstreamer::Element,
    appsink: gstreamer_app::AppSink,
    current_sample: std::sync::Arc<std::sync::Mutex<Option<gstreamer::Sample>>>,
    gst_gl_context: Option<gstreamer_gl::GLContext>,
}

impl<C: slint::ComponentHandle + 'static> Player<C> {
    fn new(app: slint::Weak<C>) -> Result<Self, anyhow::Error> {
        gstreamer::init()?;

        let source = gstreamer::ElementFactory::make("playbin", None)?;
        source.set_property("uri", "https://www.freedesktop.org/software/gstreamer-sdk/data/media/sintel_trailer-480p.webm");

        let appsink = gstreamer::ElementFactory::make("appsink", None)?
            .dynamic_cast::<gstreamer_app::AppSink>()
            .unwrap();

        appsink.set_property("enable-last-sample", false);
        appsink.set_property("emit-signals", false);
        appsink.set_property("max-buffers", 1u32);

        let caps = gstreamer::Caps::builder("video/x-raw")
            .features(&[&gstreamer_gl::CAPS_FEATURE_MEMORY_GL_MEMORY])
            .field("format", gstreamer_video::VideoFormat::Rgba.to_str())
            .field("texture-target", "2D")
            .build();
        appsink.set_caps(Some(&caps));

        let glsink = gstreamer::ElementFactory::make("glsinkbin", None)?;
        glsink.set_property("sink", &appsink);

        source.set_property("video-sink", &glsink);

        Ok(Self {
            app,
            pipeline: source,
            appsink,
            current_sample: std::sync::Arc::new(std::sync::Mutex::new(None)),
            gst_gl_context: None,
        })
    }

    fn setup_graphics(&mut self, graphics_api: &slint::GraphicsAPI) {
        let egl = match graphics_api {
            slint::GraphicsAPI::NativeOpenGL { get_proc_address } => {
                glutin_egl_sys::egl::Egl::load_with(|symbol| {
                    get_proc_address(&std::ffi::CString::new(symbol).unwrap())
                })
            }
            _ => panic!("unsupported graphics API"),
        };

        let (gst_gl_context, gst_gl_display) = unsafe {
            let platform = gstreamer_gl::GLPlatform::EGL;

            let egl_display = egl.GetCurrentDisplay();
            let display =
                gstreamer_gl_egl::GLDisplayEGL::with_egl_display(egl_display as usize).unwrap();
            let native_context = egl.GetCurrentContext();

            (
                gstreamer_gl::GLContext::new_wrapped(
                    &display,
                    native_context as _,
                    platform,
                    gstreamer_gl::GLContext::current_gl_api(platform).0,
                )
                .expect("unable to create wrapped GL context"),
                display,
            )
        };

        gst_gl_context.activate(true).expect("could not activate GSL GL context");
        gst_gl_context.fill_info().expect("failed to fill GL info for wrapped context");

        self.gst_gl_context = Some(gst_gl_context.clone());

        let bus = self.pipeline.bus().unwrap();
        bus.set_sync_handler({
            let gst_gl_context = gst_gl_context.clone();
            move |_, msg| {
                match msg.view() {
                    gstreamer::MessageView::NeedContext(ctx) => {
                        let ctx_type = ctx.context_type();
                        if ctx_type == *gstreamer_gl::GL_DISPLAY_CONTEXT_TYPE {
                            if let Some(element) = msg
                                .src()
                                .map(|source| source.downcast::<gstreamer::Element>().unwrap())
                            {
                                let gst_context = gstreamer::Context::new(ctx_type, true);
                                gst_context.set_gl_display(&gst_gl_display);
                                element.set_context(&gst_context);
                            }
                        } else if ctx_type == "gst.gl.app_context" {
                            if let Some(element) = msg
                                .src()
                                .map(|source| source.downcast::<gstreamer::Element>().unwrap())
                            {
                                let mut gst_context = gstreamer::Context::new(ctx_type, true);
                                {
                                    let gst_context = gst_context.get_mut().unwrap();
                                    let structure = gst_context.structure_mut();
                                    structure.set("context", &gst_gl_context);
                                }
                                element.set_context(&gst_context);
                            }
                        }
                    }
                    _ => (),
                }
                // forward event

                gstreamer::BusSyncReply::Pass
            }
        });

        self.pipeline.set_state(gstreamer::State::Playing).unwrap();

        let app_weak = self.app.clone();

        let current_sample_ref = self.current_sample.clone();

        self.appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().unwrap();

                    {
                        //let buffer = sample.buffer().unwrap();
                        let _info = sample
                            .caps()
                            .map(|caps| gstreamer_video::VideoInfo::from_caps(caps).unwrap())
                            .unwrap();
                    }

                    let current_sample_ref = current_sample_ref.clone();

                    app_weak.upgrade_in_event_loop(move |app| {
                        *current_sample_ref.lock().unwrap() = Some(sample);

                        app.window().request_redraw();
                    }).ok();
                    Ok(gstreamer::FlowSuccess::Ok)
                })
                .build(),
        )
    }
}

impl<C: slint::ComponentHandle + 'static> Drop for Player<C> {
    fn drop(&mut self) {
        self.pipeline.send_event(gstreamer::event::Eos::new());
        self.pipeline.set_state(gstreamer::State::Null).unwrap();
        eprintln!("Player drop");
    }
}

pub fn main() -> Result<(), anyhow::Error> {
    let main_window = MainWindow::new()?;

    let mut player = Player::new(main_window.as_weak())?;

    let mw_weak = main_window.as_weak();

    if let Err(error) =
        main_window.window().set_rendering_notifier(move |state, graphics_api| match state {
            slint::RenderingState::RenderingSetup => {
                player.setup_graphics(graphics_api);
            }
            slint::RenderingState::RenderingTeardown => {
                todo!()
            }
            slint::RenderingState::BeforeRendering => {
                if let Some(sample) = player.current_sample.lock().unwrap().as_ref() {
                    let buffer = sample.buffer_owned().unwrap();
                    let info = sample
                        .caps()
                        .map(|caps| gstreamer_video::VideoInfo::from_caps(caps).unwrap())
                        .unwrap();

                    {
                        // let sync_meta = buffer.meta::<gstreamer_gl::GLSyncMeta>().unwrap();
                        // sync_meta.set_sync_point(player.gst_gl_context.as_ref().unwrap());
                    }

                    if let Ok(frame) =
                        gstreamer_video::VideoFrame::from_buffer_readable_gl(buffer, &info)
                    {
                        //let sync_meta = frame.buffer().meta::<gstreamer_gl::GLSyncMeta>().unwrap();
                        //sync_meta.wait(player.gst_gl_context.as_ref().unwrap());

                        if let Some(texture) = frame.texture_id(0).and_then(|id| id.try_into().ok())
                        {
                            mw_weak.unwrap().set_texture(unsafe {
                                slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                                    texture,
                                    [frame.width(), frame.height()].into(),
                                )
                                .build()
                            });
                        }
                    }
                }
            }
            _ => {} // Nothing to do
        })
    {
        match error {
            slint::SetRenderingNotifierError::Unsupported => eprintln!("This example requires the use of the GL backend. Please run with the environment variable SLINT_BACKEND=GL set."),
            _ => unreachable!()
        }
        std::process::exit(1);
    };

    main_window.run()?;
    Ok(())
}
