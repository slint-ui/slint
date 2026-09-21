// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint! {
    import { ScrollView } from "std-widgets.slint";

    export component Comparison inherits Window {
        title: "Android / Slint";
        background: #f4f6fa;
        out property <float> scroll-offset: -list.content-y / 1px;

        Text {
            x: root.width / 2;
            y: 0;
            width: root.width / 2;
            height: 48px;
            text: "Slint";
            horizontal-alignment: center;
            vertical-alignment: center;
            font-size: 18px;
            font-weight: 700;
            color: #17233b;
        }

        TouchArea {
            x: root.width / 2;
            y: 0;
            width: root.width / 2;
            height: 48px;
            clicked => {
                list.content-y = list.content-y == 0px
                    ? list.height - list.content-height
                    : 0px;
            }
        }

        Rectangle {
            x: root.width / 2;
            y: 0;
            width: 1px;
            height: root.height;
            background: #bcc5d5;
        }

        list := ScrollView {
            x: root.width / 2 + 1px;
            y: 48px;
            width: root.width / 2 - 1px;
            height: root.height - 48px;
            content-width: self.width - 12px;
            content-height: 1000 * 56px;
            horizontal-scrollbar-policy: ScrollBarPolicy.always-off;

            for row in 1000 : Rectangle {
                y: row * 56px;
                height: 56px;
                width: list.content-width;
                background: mod(row, 2) == 0 ? #e4ebf5 : #ffffff;

                Text {
                    x: 12px;
                    height: parent.height;
                    vertical-alignment: center;
                    text: "Row " + (row + 1);
                    font-size: 18px;
                    color: #17233b;
                }
            }
        }
    }
}

fn run() -> Result<(), slint::PlatformError> {
    let app = Comparison::new()?;
    let weak = app.as_weak();
    let start = std::time::Instant::now();
    let sample_timer = slint::Timer::default();
    sample_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(8),
        move || {
            if let Some(app) = weak.upgrade() {
                log::info!(
                    "SCROLL_COMPARE,S,{:.3},{:.3}",
                    start.elapsed().as_secs_f64() * 1000.0,
                    app.get_scroll_offset()
                );
            }
        },
    );
    app.run()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), slint::PlatformError> {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    slint::android::init(app).unwrap();
    run()
}
