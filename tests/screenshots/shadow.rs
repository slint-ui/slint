// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint_interpreter::ComponentHandle;

pub fn assert_shadow_tracks_source_paint() {
    let compiler = slint_interpreter::Compiler::default();
    let compiled = crate::testing::poll_once(
        compiler.build_from_source(
            r#"
        export component TestCase inherits Window {
            width: 130px;
            height: 70px;
            background: white;
            in-out property<brush> fill: transparent;
            in-out property<brush> stroke: black;
            in-out property<length> border-size: 4px;
            Rectangle {
                x: 10px;
                y: 10px;
                width: 40px;
                height: 40px;
                background: root.fill;
                border-color: root.stroke;
                border-width: root.border-size;
                drop-shadow-color: red;
                drop-shadow-offset-x: 60px;
            }
        }
        "#
            .into(),
            "shadow_tracks_source_paint.slint".into(),
        ),
    )
    .unwrap();
    assert!(!compiled.has_errors());
    let component = compiled.components().last().unwrap().create().unwrap();
    component.show().unwrap();
    let sample = |x: usize| {
        let image = component.window().take_snapshot().unwrap();
        let pixel = image.as_slice()[30 * image.width() as usize + x];
        (pixel.r, pixel.g, pixel.b)
    };
    assert_eq!(sample(90), (255, 255, 255));
    assert_eq!(sample(72), (255, 0, 0));
    component
        .set_property(
            "fill",
            slint_interpreter::Brush::from(slint_interpreter::Color::from_argb_u8(128, 0, 128, 0))
                .into(),
        )
        .unwrap();
    let (r, g, b) = sample(90);
    assert_eq!(r, 255);
    assert!((126..=128).contains(&g) && (126..=128).contains(&b));
    component.set_property("fill", slint_interpreter::Brush::default().into()).unwrap();
    component.set_property("stroke", slint_interpreter::Brush::default().into()).unwrap();
    assert_eq!(sample(72), (255, 255, 255));
    component
        .set_property(
            "stroke",
            slint_interpreter::Brush::from(slint_interpreter::Color::from_rgb_u8(0, 0, 0)).into(),
        )
        .unwrap();
    component.set_property("border-size", 0.into()).unwrap();
    assert_eq!(sample(72), (255, 255, 255));
    component.set_property("border-size", 4.into()).unwrap();
    assert_eq!(sample(72), (255, 0, 0));
}

pub fn assert_shadow_spread_preserves_adjusted_corner_radii() {
    let compiler = slint_interpreter::Compiler::default();
    let compiled = crate::testing::poll_once(
        compiler.build_from_source(
            r#"
        export component TestCase inherits Window {
            width: 160px;
            height: 90px;
            background: white;
            in-out property<length> radius: 1px;
            in-out property<length> spread: 2px;
            in-out property<color> stroke: #0000ff80;
            Rectangle {
                x: 10px;
                y: 15px;
                width: 50px;
                height: 50px;
                background: #00800080;
                border-width: 20px;
                border-color: root.stroke;
                border-top-left-radius: root.radius;
                border-bottom-right-radius: root.radius;
                drop-shadow-color: red;
                drop-shadow-offset-x: 75px;
                drop-shadow-spread: root.spread;
            }
        }
        "#
            .into(),
            "shadow_spread_preserves_adjusted_corner_radii.slint".into(),
        ),
    )
    .unwrap();
    assert!(!compiled.has_errors());
    let component = compiled.components().last().unwrap().create().unwrap();
    component.show().unwrap();
    for alpha in [128, 255] {
        component
            .set_property(
                "stroke",
                slint_interpreter::Brush::from(slint_interpreter::Color::from_argb_u8(
                    alpha, 0, 0, 255,
                ))
                .into(),
            )
            .unwrap();
        for spread in [-2., 0., 2.] {
            component.set_property("spread", spread.into()).unwrap();
            component.set_property("radius", 1.into()).unwrap();
            let small_radius = component.window().take_snapshot().unwrap();
            // Both radii produce the same painted corners: the border renderer raises
            // positive radii below half the border width to 10.01px.
            component.set_property("radius", 10.01.into()).unwrap();
            let adjusted_radius = component.window().take_snapshot().unwrap();
            let differences = small_radius
                .as_bytes()
                .iter()
                .zip(adjusted_radius.as_bytes())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(
                differences, 0,
                "equal painted shapes must cast equal shadows at spread {spread}"
            );
        }
    }
}
