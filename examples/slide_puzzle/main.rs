/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use sixtyfps::Model;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

sixtyfps::include_modules!();

fn shuffle() -> Vec<i8> {
    let mut vec = ((-1)..15).into_iter().collect::<Vec<i8>>();
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    vec.shuffle(&mut rng);
    vec
}

struct AppState {
    pieces: Rc<sixtyfps::VecModel<Piece>>,
    main_window: sixtyfps::ComponentWeakHandle<MainWindow>,
    /// An array of 16 values wixh represent a 4x4 matrix containing the piece number in that
    /// position. -1 is no piece.
    positions: Vec<i8>,
}

impl AppState {
    fn set_pieces_pos(&self, p: i8, pos: i8) {
        if p >= 0 {
            self.pieces
                .set_row_data(p as usize, Piece { pos_x: (pos % 4) as _, pos_y: (pos / 4) as _ });
        }
    }

    fn randomize(&mut self) {
        self.positions = shuffle();
        for (i, p) in self.positions.iter().enumerate() {
            self.set_pieces_pos(*p, i as _);
        }
        self.main_window.upgrade().map(|x| x.as_ref().set_moves(0));
        self.apply_tiles_left();
    }

    fn apply_tiles_left(&self) {
        let left = 15 - self.positions.iter().enumerate().filter(|(i, x)| *i as i8 == **x).count();
        self.main_window.upgrade().map(|x| x.as_ref().set_tiles_left(left as _));
    }

    fn piece_clicked(&mut self, p: i8) {
        let piece = self.pieces.row_data(p as usize);
        assert_eq!(self.positions[(piece.pos_y * 4 + piece.pos_x) as usize], p);

        // find the coordinate of the hole.
        let hole = self.positions.iter().position(|x| *x == -1).unwrap() as i8;
        let pos = (piece.pos_y * 4 + piece.pos_x) as i8;
        let sign = if pos > hole { -1 } else { 1 };
        if hole % 4 == piece.pos_x as i8 {
            self.slide(pos, sign * 4)
        } else if hole / 4 == piece.pos_y as i8 {
            self.slide(pos, sign)
        } else {
            return;
        };
        self.apply_tiles_left();
        self.main_window.upgrade().map(|x| x.as_ref().set_moves(x.as_ref().get_moves() + 1));
    }

    fn slide(&mut self, pos: i8, offset: i8) {
        let mut swap = pos;
        while self.positions[pos as usize] != -1 {
            swap += offset;
            self.positions.swap(pos as usize, swap as usize);
            self.set_pieces_pos(self.positions[swap as usize] as _, swap);
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let main_window = MainWindow::new();
    let state = Rc::new(RefCell::new(AppState {
        pieces: Rc::new(sixtyfps::VecModel::<Piece>::from(vec![Piece::default(); 15])),
        main_window: main_window.as_weak(),
        positions: vec![],
    }));
    state.borrow_mut().randomize();
    main_window.as_ref().set_pieces(sixtyfps::ModelHandle::new(state.borrow().pieces.clone()));
    let state_copy = state.clone();
    main_window.as_ref().on_piece_cliked(move |p| state_copy.borrow_mut().piece_clicked(p as i8));
    let state_copy = state.clone();
    main_window.as_ref().on_reset(move || state_copy.borrow_mut().randomize());
    main_window.run();
}
