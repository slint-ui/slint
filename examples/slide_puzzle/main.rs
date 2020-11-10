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

fn set_pieces_pos(p: i8, pos: i8, pieces: &sixtyfps::VecModel<Piece>) {
    if p >= 0 {
        pieces.set_row_data(p as usize, Piece { pos_x: (pos % 4) as _, pos_y: (pos / 4) as _ });
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let pieces = Rc::new(sixtyfps::VecModel::<Piece>::from(vec![Piece::default(); 15]));

    let positions = shuffle();
    for (i, p) in positions.iter().enumerate() {
        set_pieces_pos(*p, i as _, &pieces);
    }

    let main_window = Main::new();
    main_window.as_ref().set_pieces(sixtyfps::ModelHandle::new(pieces.clone()));
    let positions = RefCell::new(positions);
    main_window.as_ref().on_piece_cliked(move |p| {
        let mut positions = positions.borrow_mut();
        let piece = pieces.row_data(p as usize);
        assert_eq!(positions[(piece.pos_y * 4 + piece.pos_x) as usize], p as i8);

        let pos = (piece.pos_y * 4 + piece.pos_x) as i8;
        let mut check = |ofst: i8| {
            let maybe_empty = pos + ofst;
            if maybe_empty < 0 || maybe_empty > 15 {
                return false;
            }
            if positions[maybe_empty as usize] != -1 {
                return false;
            }
            positions.swap(pos as usize, maybe_empty as usize);
            set_pieces_pos(p as _, maybe_empty, &pieces);
            true
        };
        if check(-4) {
        } else if check(4) {
        } else if check(1) {
        } else if check(-1) {
        }
    });
    main_window.run();
}
