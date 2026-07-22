// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::{FilterModel, Model, SortModel};
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    let state = init();
    let main_window = state.main_window.clone_strong();
    #[cfg(target_os = "android")]
    STATE.with(|ui| *ui.borrow_mut() = Some(state));
    main_window.run().unwrap();
}

fn init() -> State {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let todo_model = Rc::new(slint::VecModel::<TodoItem>::from(vec![
        TodoItem { checked: true, title: "Implement the .slint file".into() },
        TodoItem { checked: true, title: "Do the Rust part".into() },
        TodoItem { checked: false, title: "Make the C++ code".into() },
        TodoItem { checked: false, title: "Write some JavaScript code".into() },
        TodoItem { checked: false, title: "Test the application".into() },
        TodoItem { checked: false, title: "Ship to customer".into() },
        TodoItem { checked: false, title: "???".into() },
        TodoItem { checked: false, title: "Profit".into() },
    ]));

    let main_window = MainWindow::new().unwrap();

    main_window.on_todo_added({
        let todo_model = todo_model.clone();
        move |text| todo_model.push(TodoItem { checked: false, title: text })
    });
    main_window.on_remove_done({
        let todo_model = todo_model.clone();
        move || {
            let mut offset = 0;
            for i in 0..todo_model.row_count() {
                if todo_model.row_data(i - offset).unwrap().checked {
                    todo_model.remove(i - offset);
                    offset += 1;
                }
            }
        }
    });

    let weak_window = main_window.as_weak();
    main_window.on_popup_confirmed(move || {
        let window = weak_window.unwrap();
        window.hide().unwrap();
    });

    {
        let weak_window = main_window.as_weak();
        let todo_model = todo_model.clone();
        main_window.window().on_close_requested(move || {
            let window = weak_window.unwrap();

            if todo_model.iter().any(|t| !t.checked) {
                window.invoke_show_confirm_popup();
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                slint::CloseRequestResponse::HideWindow
            }
        });
    }

    main_window.on_apply_sorting_and_filtering({
        let weak_window = main_window.as_weak();
        let todo_model = todo_model.clone();

        move || {
            let window = weak_window.unwrap();
            window.set_todo_model(todo_model.clone().into());

            if window.get_hide_done_items() {
                window.set_todo_model(
                    Rc::new(FilterModel::new(window.get_todo_model(), |e| !e.checked)).into(),
                );
            }

            if window.get_is_sort_by_name() {
                window.set_todo_model(
                    Rc::new(SortModel::new(window.get_todo_model(), |lhs, rhs| {
                        lhs.title.to_lowercase().cmp(&rhs.title.to_lowercase())
                    }))
                    .into(),
                );
            }
        }
    });

    main_window.set_show_header(true);
    main_window.set_todo_model(todo_model.clone().into());
    State { main_window, todo_model }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    use slint::android::android_activity::{MainEvent, PollEvent};
    slint::android::init_with_event_listener(app, |event| {
        match event {
            PollEvent::Main(MainEvent::SaveState { saver, .. }) => {
                STATE.with(|state| -> Option<()> {
                    let todo_state = SerializedState::save(state.borrow().as_ref()?);
                    saver.store(&serde_json::to_vec(&todo_state).ok()?);
                    Some(())
                });
            }
            PollEvent::Main(MainEvent::Resume { loader, .. }) => {
                STATE.with(|state| -> Option<()> {
                    let bytes: Vec<u8> = loader.load()?;
                    let todo_state: SerializedState = serde_json::from_slice(&bytes).ok()?;
                    todo_state.restore(state.borrow().as_ref()?);
                    Some(())
                });
            }
            _ => {}
        };
    })
    .unwrap();
    main();
}

pub struct State {
    pub main_window: MainWindow,
    pub todo_model: Rc<slint::VecModel<TodoItem>>,
}

#[cfg(target_os = "android")]
thread_local! {
    static STATE : core::cell::RefCell<Option<State>> = Default::default();
}

#[cfg(target_os = "android")]
#[derive(serde::Serialize, serde::Deserialize)]
struct SerializedState {
    items: Vec<TodoItem>,
    sort: bool,
    hide_done: bool,
}

#[cfg(target_os = "android")]
impl SerializedState {
    fn restore(self, state: &State) {
        state.todo_model.set_vec(self.items);
        state.main_window.set_hide_done_items(self.hide_done);
        state.main_window.set_is_sort_by_name(self.sort);
        state.main_window.invoke_apply_sorting_and_filtering();
    }
    fn save(state: &State) -> Self {
        Self {
            items: state.todo_model.iter().collect(),
            sort: state.main_window.get_is_sort_by_name(),
            hide_done: state.main_window.get_hide_done_items(),
        }
    }
}

#[test]
fn press_add_adds_one_todo() {
    if option_env!("SLINT_EMIT_DEBUG_INFO").unwrap_or_default() != "1" {
        println!("This test needs to be build with `SLINT_EMIT_DEBUG_INFO=1` in the environment");
        return;
    }
    i_slint_backend_testing::init_no_event_loop();
    use i_slint_backend_testing::{ElementHandle, ElementQuery};
    let state = init();
    state.todo_model.set_vec(vec![TodoItem { checked: false, title: "first".into() }]);
    let line_edit = ElementQuery::from_root(&state.main_window)
        .match_id("MainWindow::text-edit")
        .find_first()
        .unwrap();
    assert_eq!(line_edit.accessible_value().unwrap(), "");
    line_edit.set_accessible_value("second");

    let button = ElementHandle::find_by_accessible_label(&state.main_window, "Add New Entry")
        .next()
        .unwrap();
    button.invoke_accessible_default_action();

    assert_eq!(state.todo_model.row_count(), 2);
    assert_eq!(
        state.todo_model.row_data(0).unwrap(),
        TodoItem { checked: false, title: "first".into() }
    );
    assert_eq!(
        state.todo_model.row_data(1).unwrap(),
        TodoItem { checked: false, title: "second".into() }
    );

    assert_eq!(line_edit.accessible_value().unwrap(), "");
}
