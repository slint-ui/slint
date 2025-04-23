// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::{Model, VecModel};

use crate::preview::ui;

use std::rc::Rc;

fn find_index_for_position(model: &slint::ModelRc<ui::GradientStop>, position: f32) -> usize {
    let position = position.clamp(0.0, 1.0);

    model
        .iter()
        .position(|gs| gs.position.total_cmp(&position) != std::cmp::Ordering::Less)
        .unwrap_or(model.row_count())
}

pub fn add_gradient_stop(model: slint::ModelRc<ui::GradientStop>, value: ui::GradientStop) -> i32 {
    let insert_pos = find_index_for_position(&model, value.position);
    let m = model.as_any().downcast_ref::<VecModel<_>>().unwrap();
    m.insert(insert_pos, value);
    (insert_pos) as i32
}

pub fn remove_gradient_stop(model: slint::ModelRc<ui::GradientStop>, row: i32) {
    if row < 0 {
        return;
    }
    let row = row as usize;
    if row < model.row_count() {
        model.as_any().downcast_ref::<VecModel<ui::GradientStop>>().unwrap().remove(row);
    }
}

pub fn move_gradient_stop(
    model: slint::ModelRc<ui::GradientStop>,
    row: i32,
    new_position: f32,
) -> i32 {
    let mut row_usize = row as usize;
    if row < 0 || row_usize >= model.row_count() {
        return row;
    }

    let m = model.as_any().downcast_ref::<VecModel<ui::GradientStop>>().unwrap();

    let mut gs = model.row_data(row_usize).unwrap();
    gs.position = new_position;
    model.set_row_data(row_usize, gs);

    fn swap_direction(
        model: &VecModel<ui::GradientStop>,
        row: usize,
        value: f32,
    ) -> Option<(usize, usize)> {
        let previous = model.row_data(row.saturating_sub(1));
        let next = model.row_data(row + 1);
        let previous_order = previous.map(|gs| value.total_cmp(&gs.position));
        let next_order = next.map(|gs| value.total_cmp(&gs.position));

        match (previous_order, next_order) {
            (Some(std::cmp::Ordering::Less), _) => Some((row, row - 1)),
            (_, Some(std::cmp::Ordering::Greater)) => Some((row, row + 1)),
            _ => None,
        }
    }

    while let Some((old_row, new_row)) = swap_direction(m, row_usize, new_position) {
        m.swap(old_row, new_row);
        row_usize = new_row;
    }

    row_usize as i32
}

fn interpolate(
    previous: ui::GradientStop,
    next: ui::GradientStop,
    factor: f32,
) -> ui::GradientStop {
    let position = (previous.position + (next.position - previous.position) * factor)
        .clamp(previous.position, next.position);
    let color = next.color.mix(&previous.color, factor);

    ui::GradientStop { position, color }
}

fn fallback_gradient_stop(position: f32) -> ui::GradientStop {
    ui::GradientStop { position, color: slint::Color::from_argb_u8(0xff, 0x80, 0x80, 0x80) }
}

pub fn suggest_gradient_stop_at_row(
    model: slint::ModelRc<ui::GradientStop>,
    row: i32,
) -> ui::GradientStop {
    let row_usize = row as usize;
    if row < 0 || row_usize > model.row_count() {
        return fallback_gradient_stop(0.0);
    }

    let (prev, next) = if row_usize == 0 {
        let first_stop = model.row_data(0).unwrap_or(fallback_gradient_stop(0.0));
        let very_first_stop = ui::GradientStop { position: 0.0, color: first_stop.color };
        (very_first_stop.clone(), very_first_stop)
    } else if row_usize == model.row_count() {
        let last_stop = model.row_data(row_usize - 1).unwrap_or(fallback_gradient_stop(1.0));
        let very_last_stop = ui::GradientStop { position: 1.0, color: last_stop.color };
        (very_last_stop.clone(), very_last_stop)
    } else {
        (
            model.row_data(row_usize - 1).expect("Index was tested to be valid"),
            model.row_data(row_usize).expect("index was tested to be valid"),
        )
    };

    interpolate(prev, next, 0.5)
}

pub fn suggest_gradient_stop_at_position(
    model: slint::ModelRc<ui::GradientStop>,
    position: f32,
) -> ui::GradientStop {
    let position = position.clamp(0.0, 1.0);

    if model.row_count() == 0 {
        return fallback_gradient_stop(position);
    }

    let mut prev = model.row_data(0).expect("Not empty");
    prev.position = 0.0;
    let mut next = model.row_data(model.row_count() - 1).expect("Not empty");
    next.position = 1.0;

    for current in model.iter() {
        if current.position > position {
            next = current;
            break;
        }

        if current.position <= position {
            prev = current;
        }
    }

    let factor = (position - prev.position) / (next.position - prev.position);

    interpolate(prev, next, factor)
}

pub fn clone_gradient_stops(
    model: slint::ModelRc<ui::GradientStop>,
) -> slint::ModelRc<ui::GradientStop> {
    let cloned_data = model.iter().collect::<Vec<_>>();
    Rc::new(VecModel::from(cloned_data)).into()
}

#[cfg(test)]
mod tests {
    use crate::preview::ui;

    use slint::{Model, ModelRc, VecModel};

    use std::rc::Rc;

    fn make_empty_model() -> ModelRc<ui::GradientStop> {
        Rc::new(VecModel::default()).into()
    }

    #[test]
    fn test_add_and_remove_gradient_stops() {
        let model = make_empty_model();

        super::remove_gradient_stop(model.clone(), 0);

        let mut it = model.iter();
        assert_eq!(it.next(), None);

        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff010101) },
        );

        super::remove_gradient_stop(model.clone(), 0);
        let mut it = model.iter();
        assert_eq!(it.next(), None);

        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff010101) },
        );

        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff020202) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff030303) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.5, color: slint::Color::from_argb_encoded(0xff050505) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff040404) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            },
        );

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 2);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), -1);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 42);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 0);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 3);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);
    }

    fn make_model() -> ModelRc<ui::GradientStop> {
        let model = make_empty_model();
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff040404) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff030303) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.5, color: slint::Color::from_argb_encoded(0xff050505) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff020202) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff010101) },
        );

        model
    }

    #[test]
    fn test_move_gradient_stop() {
        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 0.4), 3);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.4,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 0.1), 2);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 0, 0.05), 1);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.05,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 0.0), 2);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 1.0), 3);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);
    }
}
