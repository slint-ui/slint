// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::Model;
use slint::VecModel;
use std::cell::RefCell;
use std::rc::Rc;

slint::slint!(import { MainWindow } from "circledraw.slint";);

enum Change {
    CircleAdded { row: usize },
    CircleRemoved { row: usize, circle: Circle },
    CircleResized { row: usize, old_d: f32 },
}

struct UndoStack<F> {
    stack: Vec<Option<Change>>,
    // Everything at and after this is a redo action
    redo_offset: usize,
    undo2redo: F,
}

impl<F> UndoStack<F>
where
    F: Fn(Change) -> Change,
{
    fn new(undo2redo: F) -> Self {
        Self { stack: Vec::new(), redo_offset: 0, undo2redo }
    }

    fn push(&mut self, change: Change) {
        self.stack.truncate(self.redo_offset);
        self.stack.push(Some(change));
        self.redo_offset += 1;
    }

    fn undoable(&self) -> bool {
        self.redo_offset > 0
    }

    fn redoable(&self) -> bool {
        self.redo_offset < self.stack.len()
    }

    fn undo(&mut self) {
        self.redo_offset -= 1;

        let undo = self.stack.get_mut(self.redo_offset).unwrap().take().unwrap();
        let redo = (self.undo2redo)(undo);
        self.stack[self.redo_offset] = Some(redo);
    }

    fn redo(&mut self) {
        let redo = self.stack.get_mut(self.redo_offset).unwrap().take().unwrap();
        let undo = (self.undo2redo)(redo);
        self.stack[self.redo_offset] = Some(undo);

        self.redo_offset += 1;
    }
}

pub fn main() {
    let main_window = MainWindow::new().unwrap();

    let model = Rc::new(VecModel::default());
    main_window.set_model(model.clone().into());

    let undo_stack;
    {
        let model = model.clone();
        undo_stack = Rc::new(RefCell::new(UndoStack::new(move |change| match change {
            Change::CircleAdded { row } => {
                let circle = model.row_data(row).unwrap();
                model.remove(row);
                Change::CircleRemoved { row, circle }
            }
            Change::CircleRemoved { row, circle } => {
                model.insert(row, circle);
                Change::CircleAdded { row }
            }
            Change::CircleResized { row, old_d } => {
                let mut circle = model.row_data(row).unwrap();
                let d = circle.d;
                circle.d = old_d;
                model.set_row_data(row, circle);
                Change::CircleResized { row, old_d: d }
            }
        })));
    }

    {
        let model = model.clone();
        let undo_stack = undo_stack.clone();
        let window_weak = main_window.as_weak();
        main_window.on_background_clicked(move |x, y| {
            let mut undo_stack = undo_stack.borrow_mut();
            let main_window = window_weak.unwrap();

            model.push(Circle { x: x as f32, y: y as f32, d: 30.0 });
            undo_stack.push(Change::CircleAdded { row: model.row_count() - 1 });

            main_window.set_undoable(undo_stack.undoable());
            main_window.set_redoable(undo_stack.redoable());
        });
    }

    {
        let undo_stack = undo_stack.clone();
        let window_weak = main_window.as_weak();
        main_window.on_undo_clicked(move || {
            let mut undo_stack = undo_stack.borrow_mut();
            let main_window = window_weak.unwrap();
            undo_stack.undo();
            main_window.set_undoable(undo_stack.undoable());
            main_window.set_redoable(undo_stack.redoable());
        });
    }

    {
        let undo_stack = undo_stack.clone();
        let window_weak = main_window.as_weak();
        main_window.on_redo_clicked(move || {
            let mut undo_stack = undo_stack.borrow_mut();
            let main_window = window_weak.unwrap();
            undo_stack.redo();
            main_window.set_undoable(undo_stack.undoable());
            main_window.set_redoable(undo_stack.redoable());
        });
    }

    {
        let model = model.clone();
        let undo_stack = undo_stack.clone();
        let window_weak = main_window.as_weak();
        main_window.on_circle_resized(move |row, diameter| {
            let row = row as usize;
            let mut undo_stack = undo_stack.borrow_mut();
            let main_window = window_weak.unwrap();

            let mut circle = model.row_data(row).unwrap();
            let old_d = circle.d;
            circle.d = diameter;
            model.set_row_data(row, circle);
            undo_stack.push(Change::CircleResized { row, old_d });

            main_window.set_undoable(undo_stack.undoable());
            main_window.set_redoable(undo_stack.redoable());
        });
    }

    main_window.run().unwrap();
}
