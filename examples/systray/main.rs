slint::include_modules!();

fn main() {
    let my_tray = App::new().unwrap();
    let _tray = slint::private_unstable_api::create_system_tray(slint::system_tray::Params {
        icon: &my_tray.get_video_frame(),
        tooltip: "blah",
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}
