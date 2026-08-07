// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod common;

use i_slint_core::input::{InternalKeyEvent, KeyEventType};
use i_slint_core::window::WindowInner;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, SoftwareRenderer, TargetPixel,
};
use std::rc::Rc;

#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
struct TestPixel(bool);

impl TargetPixel for TestPixel {
    fn blend(&mut self, _color: PremultipliedRgbaColor) {
        *self = Self(true);
    }

    fn from_rgb(_red: u8, _green: u8, _blue: u8) -> Self {
        Self(true)
    }
}

const WIDTH: usize = 200;
const HEIGHT: usize = 100;

fn setup() -> Rc<MinimalSoftwareWindow> {
    common::setup(WIDTH as u32, HEIGHT as u32)
}

fn render_and_get_miss_count(renderer: &SoftwareRenderer) -> u64 {
    renderer.text_layout_cache().reset_cache_miss_count();
    let mut buf = vec![TestPixel(false); WIDTH * HEIGHT];
    renderer.render(buf.as_mut_slice(), WIDTH);
    renderer.text_layout_cache().cache_miss_count()
}

/// Renders into real pixels and counts the reddish ones. The tests below paint everything else
/// white, so a red count is "did this run of glyphs get the color it was supposed to get".
fn render_and_count_red(window: &Rc<MinimalSoftwareWindow>) -> usize {
    let mut buf = vec![slint::Rgb8Pixel::default(); WIDTH * HEIGHT];
    window.draw_if_needed(|renderer| {
        renderer.render(buf.as_mut_slice(), WIDTH);
    });
    buf.iter().filter(|p| p.r > 120 && p.g < 120 && p.b < 120).count()
}

#[test]
fn cache_hit_avoids_reshaping() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            Text {
                text: "Hello World";
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    let mut miss_count = 0u64;

    // First render: should shape at least once
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected at least one cache miss on first render");

    // Second render without changes: should hit cache
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Expected zero cache misses on re-render without changes");
}

#[test]
fn text_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <string> label: "Hello";
            Text {
                text: label;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change text
    ui.set_label("Goodbye".into());

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after text change");
}

#[test]
fn font_size_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <length> size: 16px;
            Text {
                text: "Hello";
                font-size: size;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change font-size
    ui.set_size(24.0);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after font-size change");
}

#[test]
fn font_weight_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <int> weight: 400;
            Text {
                text: "Hello";
                font-weight: weight;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change font-weight
    ui.set_weight(700);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after font-weight change");
}

#[test]
fn wrap_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <bool> use-no-wrap: false;
            Text {
                text: "Hello World this is a long text";
                wrap: use-no-wrap ? no-wrap : word-wrap;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render (word-wrap)
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change wrap to no-wrap
    ui.set_use_no_wrap(true);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after wrap change");
}

#[test]
fn alignment_change_does_not_reshape() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <bool> use-center-align: false;
            Text {
                text: "Hello World";
                horizontal-alignment: use-center-align ? TextHorizontalAlignment.center : TextHorizontalAlignment.left;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render (left-aligned)
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change alignment to center
    ui.set_use_center_align(true);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Alignment change should not cause reshaping");
}

#[test]
fn overflow_change_does_not_reshape() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <bool> use-elide: false;
            Text {
                text: "Hello World";
                overflow: use-elide ? TextOverflow.elide : TextOverflow.clip;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render (clip)
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change overflow to elide
    ui.set_use_elide(true);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Overflow change should not cause reshaping");
}

#[test]
fn color_change_does_not_reshape() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <color> text-color: black;
            Text {
                text: "Hello World";
                color: text-color;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change color
    ui.set_text_color(slint::Color::from_rgb_u8(255, 0, 0));

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Color change should not cause reshaping");
}

#[test]
fn scale_factor_change_invalidates_cache() {
    let window = setup();

    // Shaped glyph advances are in physical pixels, so an entry is only good for the scale factor
    // it was shaped at. The renderers drop the cache when they notice a new scale factor, but that
    // happens when rendering starts -- and the layout pass that follows the change measures first.
    slint::slint! {
        export component ScaleComponent inherits Window {
            out property <length> preferred: label.preferred-width;
            HorizontalLayout {
                alignment: start;
                label := Text {
                    text: "Hello World";
                }
            }
        }
    }

    let ui = ScaleComponent::new().unwrap();
    ui.show().unwrap();

    // Render once at 1x so that the cache holds paragraphs shaped for that scale factor.
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });
    let at_one = ui.get_preferred();
    assert!(at_one > 0.);

    // Measure at the new scale factor before rendering again, the way a real layout pass does.
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    let at_two = ui.get_preferred();

    assert!(
        (at_two - at_one).abs() <= at_one * 0.05,
        "the logical width must not follow the scale factor ({at_one} at 1x, {at_two} at 2x)"
    );
}

#[test]
fn link_color_survives_measuring() {
    let window = setup();

    // The link color is baked into the shaped glyph brushes, so measuring and drawing have to
    // shape it identically -- otherwise whichever runs first wins and the other one's color is
    // silently dropped. Layout runs first, so a mismatch shows up as unstyled link text.
    slint::slint! {
        export component TestComponent inherits Window {
            background: white;
            StyledText {
                text: @markdown("[hello](http://example.com)");
                default-color: white;
                link-color: #ff0000;
                default-font-size: 30px;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    assert!(render_and_count_red(&window) > 0, "link color was lost");
}

#[test]
fn text_input_cache_hit_avoids_reshaping() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <string> label: "Hello World";
            TextInput {
                text: label;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));

    // The first render may well find the text already shaped by the layout pass that preceded it,
    // so provoke a reshape rather than assuming one, and only then check that a redraw reuses it.
    ui.set_label("Goodbye World".into());
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected a cache miss after the text changed");

    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Expected zero cache misses on re-render without changes");
}

#[test]
fn password_input_is_cached() {
    let window = setup();

    // A password field shapes a substituted text, but the substitution is the same on every path,
    // so the result is a pure function of its properties like any other text.
    slint::slint! {
        export component TestComponent inherits Window {
            in property <string> secret: "hunter2";
            TextInput {
                text: secret;
                input-type: password;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));

    ui.set_secret("correct horse".into());
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected a cache miss after the text changed");

    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Expected zero cache misses on re-render without changes");
}

#[test]
fn composition_is_shaped_through_the_cache() {
    let window = setup();

    // An IME composition is part of the displayed text, so every path shapes it like any other
    // text and shares one cache entry -- drawing included, now that the composition's decoration
    // is applied by clipping at draw time instead of being baked into the glyph brushes.
    //
    // The input is sized by hand, so no layout pass measures it once the composition starts: the
    // cursor rect query is then the only thing that shapes the composed text, and the cache miss
    // it accounts for is what tells the two apart.
    slint::slint! {
        export component CursorComponent inherits Window {
            forward-focus: input;
            out property <length> cursor-x;
            out property <length> text-width: input.preferred-width;
            input := TextInput {
                x: 0; y: 0; width: 180px; height: 40px;
                text: "ab";
                wrap: no-wrap;
                cursor-position-changed(position) => { root.cursor-x = position.x; }
            }
        }
    }

    let ui = CursorComponent::new().unwrap();
    ui.show().unwrap();
    ui.window().dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // Render once so everything is shaped and settled, then start counting from zero.
    let mut buf = vec![TestPixel(false); WIDTH * HEIGHT];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(buf.as_mut_slice(), WIDTH);
        renderer.text_layout_cache().reset_cache_miss_count();
    }));
    // Where the text ends without a composition, i.e. the width of "ab".
    let without_composition = ui.get_text_width();

    // Updating the composition moves the cursor, and the cursor rect is the first thing to shape
    // the composed text.
    let mut event = InternalKeyEvent::default();
    event.event_type = KeyEventType::UpdateComposition;
    event.preedit_text = "WWWWWWWWWW".into();
    // Where the IME puts the cursor: at the end of the composition.
    event.cursor_position = Some(2 + "WWWWWWWWWW".len() as i32);
    WindowInner::from_pub(ui.window()).process_key_input(event);

    // Read the counter before rendering, so it only covers the cursor rect query above, and again
    // afterwards to see what drawing the composition adds.
    let (mut after_cursor_rect, mut after_render) = (0u64, 0u64);
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        after_cursor_rect = renderer.text_layout_cache().cache_miss_count();
        renderer.render(buf.as_mut_slice(), WIDTH);
        after_render = renderer.text_layout_cache().cache_miss_count();
    }));

    // One miss, not none: the cursor rect went through the cache rather than shaping beside it.
    assert_eq!(
        after_cursor_rect, 1,
        "the cursor rect should shape the composition through the cache"
    );
    // Drawing is served by that same entry, so it neither adds a miss nor costs the next query
    // its hit.
    assert_eq!(after_render, 1, "drawing the composition should reuse the cache entry");

    assert!(
        ui.get_cursor_x() > without_composition,
        "the cursor should sit past the composition ({} with, text ends at {without_composition})",
        ui.get_cursor_x()
    );
}

#[test]
fn text_input_selection_still_colors_text() {
    let window = setup();

    // The selection foreground is applied when drawing, by clipping the runs the selection covers,
    // so the selected text is served from the very same cache entry as the unselected text and has
    // to come out red all the same.
    slint::slint! {
        export component TestComponent inherits Window {
            background: white;
            public function select() { input.select-all(); }
            input := TextInput {
                text: "Hello";
                color: white;
                selection-background-color: white;
                selection-foreground-color: #ff0000;
                font-size: 30px;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // Render once unselected, which populates the cache entry, and only then select.
    assert_eq!(render_and_count_red(&window), 0, "nothing should be red before selecting");

    ui.invoke_select();
    window.request_redraw();
    assert!(render_and_count_red(&window) > 0, "selection foreground color was lost");
}

#[test]
fn text_input_selection_change_does_not_reshape() {
    let window = setup();

    // Selecting is not a shaping input: the shaped glyphs are identical with and without a
    // selection, which is what keeps a drag-select from re-shaping the whole document on every
    // mouse move. Moving the cursor is the same story.
    slint::slint! {
        export component TestComponent inherits Window {
            in property <string> content: "Hello World";
            callback select(start: int, end: int);
            input := TextInput {
                text: content;
            }
            select(start, end) => {
                input.set-selection-offsets(start, end);
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));

    // As in `text_input_cache_hit_avoids_reshaping`, the first render may find the text already
    // shaped by the layout pass before it, so provoke a miss rather than assuming one. It also
    // shows this input's draw is on the cache at all, and can therefore miss it.
    //
    // What the zero-miss assertions below catch is a selection becoming a shaping input again:
    // baking the selection colors into the glyphs makes the selection properties dependencies of
    // the entry, and each change then invalidates it. They cannot catch shaping that deliberately
    // side-steps the cache, since that records no miss to count.
    ui.set_content("Hello World!".into());
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected a cache miss after the text changed");

    for (start, end, what) in
        [(0, 5, "selecting"), (0, 8, "extending the selection"), (3, 8, "moving the selection")]
    {
        ui.invoke_select(start, end);
        window.request_redraw();
        assert!(window.draw_if_needed(|renderer| {
            miss_count = render_and_get_miss_count(renderer);
        }));
        assert_eq!(miss_count, 0, "{what} should not cause reshaping");
    }
}

#[test]
fn ime_composition_is_not_served_a_stale_size() {
    let window = setup();

    // Measuring and drawing share one cache entry, and the entry is invalidated by whatever the
    // path that filled it happened to read. Both must therefore shape through the same accessor:
    // if drawing filled the entry without looking at the composition, a later measurement would
    // trust that entry and size the box for the pre-composition text.
    slint::slint! {
        export component ImeComponent inherits Window {
            forward-focus: input;
            out property <length> preferred: input.preferred-width;
            HorizontalLayout {
                alignment: start;
                input := TextInput {
                    text: "ab";
                    wrap: no-wrap;
                }
            }
        }
    }

    let ui = ImeComponent::new().unwrap();
    ui.show().unwrap();
    ui.window().dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });
    let before = ui.get_preferred();

    let mut event = InternalKeyEvent::default();
    event.event_type = KeyEventType::UpdateComposition;
    event.preedit_text = "WWWWWWWWWW".into();
    WindowInner::from_pub(ui.window()).process_key_input(event);

    window.request_redraw();
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });
    assert!(
        ui.get_preferred() > before,
        "the box should grow to fit the composition ({before} before, {} during)",
        ui.get_preferred()
    );
}

fn render_and_get_layout_miss_count(renderer: &SoftwareRenderer) -> u64 {
    renderer.text_layout_cache().reset_layout_miss_count();
    let mut buf = vec![TestPixel(false); WIDTH * HEIGHT];
    renderer.render(buf.as_mut_slice(), WIDTH);
    renderer.text_layout_cache().layout_miss_count()
}

#[test]
fn unchanged_layout_inputs_reuse_the_line_breaking() {
    let window = setup();

    // Line breaking is retained in the cache entry alongside the shaped paragraphs: a draw whose
    // breaking inputs (width, horizontal alignment, max-lines, overflow) match the retained state
    // reuses it wholesale. Anything else -- moving the item, vertical alignment -- must not break
    // lines again.
    slint::slint! {
        export component TestComponent inherits Window {
            in property <length> item-x: 0px;
            in property <length> w: 180px;
            in property <bool> align-right: false;
            in property <bool> align-bottom: false;
            in property <string> content: "Hello World this text wraps across several lines";
            TextInput {
                x: item-x; y: 0; width: w; height: 80px;
                horizontal-alignment: align-right ? TextHorizontalAlignment.right : TextHorizontalAlignment.left;
                vertical-alignment: align-bottom ? TextVerticalAlignment.bottom : TextVerticalAlignment.top;
                text: content;
                wrap: word-wrap;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    let mut count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        count = render_and_get_layout_miss_count(renderer);
    }));

    // Moving the item redraws it at the same width: the retained breaking must be reused.
    ui.set_item_x(5.0);
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        count = render_and_get_layout_miss_count(renderer);
    }));
    assert_eq!(count, 0, "moving the item must not break its lines again");

    // The width is a breaking input.
    ui.set_w(120.0);
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        count = render_and_get_layout_miss_count(renderer);
    }));
    assert!(count > 0, "a width change must break lines again");

    // So is the horizontal alignment (parley bakes it into the line layout).
    ui.set_align_right(true);
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        count = render_and_get_layout_miss_count(renderer);
    }));
    assert!(count > 0, "an alignment change must break lines again");

    // Vertical alignment only moves the finished block; deliberately not a breaking input.
    ui.set_align_bottom(true);
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        count = render_and_get_layout_miss_count(renderer);
    }));
    assert_eq!(count, 0, "vertical alignment must not break lines again");

    // A text change reshapes, which discards the retained breaking with the entry.
    ui.set_content("Different text that also wraps across several lines".into());
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        count = render_and_get_layout_miss_count(renderer);
    }));
    assert!(count > 0, "a text change must break lines again");
}
