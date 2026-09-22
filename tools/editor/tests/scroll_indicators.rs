// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use slint::{
    ComponentHandle, LogicalPosition, platform::PointerEventButton, platform::WindowEvent,
};
use slint_interpreter::{Compiler, ComponentInstance, Value};

fn fixture(style: &str) -> ComponentInstance {
    let mut compiler = Compiler::default();
    compiler.set_style(style.into());
    compiler.compiler_configuration(i_slint_core::InternalToken).debug_info = true;
    let result = spin_on::spin_on(compiler.build_from_source(
        include_str!("scroll-indicators.slint").into(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scroll-indicators.slint"),
    ));
    let definition = result.component("ScrollIndicatorTest").unwrap_or_else(|| {
        panic!("scroll indicator fixture: {:?}", result.diagnostics().collect::<Vec<_>>())
    });
    let instance = definition.create().unwrap();
    instance.show().unwrap();
    advance(250);
    instance
}

fn advance(milliseconds: u64) {
    for _ in 0..milliseconds / 10 {
        mock_elapsed_time(10);
    }
    mock_elapsed_time(milliseconds % 10);
}

fn set(instance: &ComponentInstance, property: &str, value: impl Into<Value>) {
    instance.set_property(property, value.into()).unwrap();
    mock_elapsed_time(0);
}

fn number(instance: &ComponentInstance, property: &str) -> f64 {
    instance.get_property(property).unwrap().try_into().unwrap()
}

fn thumb(instance: &ComponentInstance, horizontal: bool) -> ElementHandle {
    let id = if horizontal {
        "EditorScrollIndicators::horizontal"
    } else {
        "EditorScrollIndicators::vertical"
    };
    ElementHandle::find_by_element_id(instance, id)
        .next()
        .unwrap()
        .query_descendants()
        .match_id("EditorScrollBar::thumb")
        .find_first()
        .unwrap()
}

fn center(element: &ElementHandle) -> LogicalPosition {
    let position = element.absolute_position();
    let size = element.size();
    LogicalPosition::new(position.x + size.width / 2., position.y + size.height / 2.)
}

#[test]
fn indicators_follow_motion_and_fade_without_changing_layout() {
    i_slint_backend_testing::init_no_event_loop();
    for style in ["cupertino", "fluent", "cosmic", "material"] {
        let instance = fixture(style);
        let vertical = thumb(&instance, false);
        let horizontal = thumb(&instance, true);
        assert_eq!(vertical.computed_opacity(), 0., "{style}");
        assert_eq!(horizontal.computed_opacity(), 0., "{style}");
        let size = (number(&instance, "visible-width"), number(&instance, "visible-height"));

        set(&instance, "content-height", 1600.);
        advance(1000);
        assert_eq!(vertical.computed_opacity(), 0.);

        set(&instance, "glide", true);
        advance(100);
        assert!(vertical.computed_opacity() > 0.99);
        advance(1000);
        assert!(vertical.computed_opacity() > 0.99, "must remain visible throughout movement");
        advance(1000);
        assert!(vertical.computed_opacity() > 0.99, "idle delay starts after movement ends");
        advance(300);
        assert!(vertical.computed_opacity() > 0. && vertical.computed_opacity() < 1.);

        set(&instance, "content-y", -650.);
        advance(100);
        assert!(vertical.computed_opacity() > 0.99, "new movement cancels fading");
        advance(900);
        assert_eq!(vertical.computed_opacity(), 0.);
        assert_eq!(horizontal.computed_opacity(), 0.);
        assert_eq!(size, (number(&instance, "visible-width"), number(&instance, "visible-height")));
    }
}

#[test]
fn thumbs_drag_hover_and_leave_hidden_content_clickable() {
    i_slint_backend_testing::init_no_event_loop();
    for horizontal in [false, true] {
        let instance = fixture("cupertino");
        let thumb = thumb(&instance, horizontal);
        let property = if horizontal { "content-x" } else { "content-y" };
        let button = PointerEventButton::Left;
        let hidden_position = center(&thumb);
        instance.window().dispatch_event(WindowEvent::PointerMoved { position: hidden_position });
        advance(1000);
        assert_eq!(thumb.computed_opacity(), 0., "hover alone must not reveal a hidden thumb");
        instance
            .window()
            .dispatch_event(WindowEvent::PointerPressed { position: hidden_position, button });
        instance
            .window()
            .dispatch_event(WindowEvent::PointerReleased { position: hidden_position, button });
        assert_eq!(number(&instance, "clicks"), 1.);

        instance.window().dispatch_event(WindowEvent::PointerExited);
        set(&instance, property, -150.);
        advance(100);
        let start = center(&thumb);
        instance.window().dispatch_event(WindowEvent::PointerMoved { position: start });
        advance(1200);
        assert_eq!(thumb.computed_opacity(), 1., "hover holds a revealed thumb");
        instance.window().dispatch_event(WindowEvent::PointerPressed { position: start, button });
        let end = LogicalPosition::new(
            start.x + if horizontal { 20. } else { 0. },
            start.y + if horizontal { 0. } else { 20. },
        );
        instance.window().dispatch_event(WindowEvent::PointerMoved { position: end });
        mock_elapsed_time(0);
        let moved = number(&instance, property);
        assert!(moved < -150., "drag must change the content offset");
        assert!(number(&instance, "scroll-events") > 0.);

        set(&instance, if horizontal { "content-width" } else { "content-height" }, 2400.);
        instance.window().dispatch_event(WindowEvent::PointerMoved { position: end });
        assert!(
            (number(&instance, property) - moved).abs() < 0.01,
            "size changes must rebase an active drag"
        );
        advance(1200);
        assert_eq!(thumb.computed_opacity(), 1.);
        instance.window().dispatch_event(WindowEvent::PointerReleased { position: end, button });
        instance.window().dispatch_event(WindowEvent::PointerExited);
        advance(1000);
        assert_eq!(thumb.computed_opacity(), 0.);

        set(&instance, property, -150.);
        advance(100);
        let position = center(&thumb);
        instance.window().dispatch_event(WindowEvent::PointerScrolled {
            position,
            delta_x: if horizontal { -50. } else { 0. },
            delta_y: if horizontal { 0. } else { -50. },
        });
        advance(300);
        assert!(
            number(&instance, property) < -150.,
            "wheel input over a thumb must reach the view"
        );
    }
}

#[test]
fn policies_and_small_viewports_keep_thumb_geometry_valid() {
    i_slint_backend_testing::init_no_event_loop();
    let instance = fixture("cupertino");
    let vertical = thumb(&instance, false);
    let horizontal = thumb(&instance, true);
    set(&instance, "always-on", true);
    advance(1000);
    assert_eq!(vertical.computed_opacity(), 1.);
    assert_eq!(horizontal.computed_opacity(), 1.);
    assert!(
        vertical.absolute_position().y + vertical.size().height < horizontal.absolute_position().y
    );

    set(&instance, "always-off", true);
    set(&instance, "content-y", -200.);
    advance(1000);
    assert_eq!(vertical.computed_opacity(), 0.);
    set(&instance, "always-off", false);
    set(&instance, "content-height", 0.);
    set(&instance, "content-width", 0.);
    advance(1000);
    assert_eq!(vertical.computed_opacity(), 0.);
    assert_eq!(horizontal.computed_opacity(), 0.);

    set(&instance, "content-width", 100000.);
    set(&instance, "view-width", 16.);
    advance(250);
    assert!(horizontal.size().width.is_finite());
    assert!(horizontal.size().width >= 0. && horizontal.size().width <= 16.);
    assert!(horizontal.absolute_position().x.is_finite());
}

#[test]
fn indicators_render_translucently_in_both_themes() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            renderer_name: Some("skia".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    for style in ["cupertino-light", "cupertino-dark"] {
        let instance = fixture(style);
        let idle = instance.window().take_snapshot().unwrap();
        set(&instance, "content-y", -200.);
        advance(100);
        let position = center(&thumb(&instance, false));
        let active = instance.window().take_snapshot().unwrap();
        let scale = instance.window().scale_factor();
        let index =
            (position.y * scale) as usize * active.width() as usize + (position.x * scale) as usize;
        let background = idle.as_slice()[index];
        let foreground = active.as_slice()[index];
        let brighter =
            |pixel: slint::Rgba8Pixel| u32::from(pixel.r) + u32::from(pixel.g) + u32::from(pixel.b);
        if style == "cupertino-light" {
            assert!(brighter(foreground) < brighter(background));
            assert!(foreground.r > 50, "thumb blends with the light surface");
        } else {
            assert!(brighter(foreground) > brighter(background));
            assert!(foreground.r < 200, "thumb blends with the dark surface");
        }
        advance(1000);
        let hidden = instance.window().take_snapshot().unwrap();
        assert_eq!(hidden.as_slice()[index], background);
    }
}
