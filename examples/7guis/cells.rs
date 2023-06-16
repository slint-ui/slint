// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
use slint::{Model, ModelRc, SharedString};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::{Rc, Weak};

slint::slint!(import { MainWindow } from "cells.slint";);

const ROW_COUNT: usize = 100;
const COL_COUNT: usize = 26;

/// Graph of dependencies so that node B depends on node A.
/// So when node A is updated, node B also need to re updated.
#[derive(Default)]
struct DepGraph<A, B> {
    dependent_index: HashMap<A, HashSet<B>>,
    dependency_index: HashMap<B, HashSet<A>>,
}

impl<A: Clone + Eq + Hash, B: Clone + Eq + Hash> DepGraph<A, B> {
    pub fn add_dep(&mut self, a: A, b: B) {
        self.dependent_index.entry(a.clone()).or_default().insert(b.clone());
        self.dependency_index.entry(b).or_default().insert(a);
    }

    pub fn dependents<'a>(&'a self, a: &A) -> impl Iterator<Item = &B> + 'a {
        self.dependent_index.get(a).into_iter().flat_map(|x| x.iter())
    }

    pub fn remove_dependencies(&mut self, b: &B) {
        if let Some(h) = self.dependency_index.remove(b) {
            for a in h {
                self.dependent_index.get_mut(&a).map(|x| x.remove(b));
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum Expr {
    Value(f32),
    Cell(usize, usize), // (row, column)
    Add(Box<(Expr, Expr)>),
    Sub(Box<(Expr, Expr)>),
}

fn parse_formula(formula: &str) -> Option<Expr> {
    let formula = formula.trim();
    let alpha_n = formula.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(formula.len());
    let num_n = formula[alpha_n..]
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .map_or(formula.len(), |x| x + alpha_n);

    let e = if alpha_n > 0 {
        if alpha_n != 1 {
            return None;
        };
        let col = formula.as_bytes()[0].to_ascii_lowercase() - b'a';
        let row = formula[alpha_n..num_n].parse().ok()?;
        Expr::Cell(row, col as usize)
    } else if num_n > 0 {
        Expr::Value(formula[alpha_n..num_n].parse().ok()?)
    } else {
        return None;
    };

    let rest = formula[num_n..].trim();

    if rest.is_empty() {
        Some(e)
    } else if let Some(x) = rest.strip_prefix("+") {
        Some(Expr::Add(Box::new((e, parse_formula(x)?))))
    } else if let Some(x) = rest.strip_prefix("-") {
        Some(Expr::Sub(Box::new((e, parse_formula(x)?))))
    } else {
        None
    }
}

#[test]
fn test_parse_formula() {
    assert_eq!(parse_formula("42"), Some(Expr::Value(42.0)));
    assert_eq!(parse_formula(" 49.5 "), Some(Expr::Value(49.5)));
    assert_eq!(parse_formula("B4"), Some(Expr::Cell(4, 1)));
    assert_eq!(parse_formula("B 4"), None);
    assert_eq!(parse_formula("B4.2"), None);
    assert_eq!(parse_formula("AB5"), None);
    assert_eq!(parse_formula("4B"), None);
    assert_eq!(
        parse_formula("8 + C6"),
        Some(Expr::Add(Box::new((Expr::Value(8.), Expr::Cell(6, 2)))))
    );
    assert_eq!(
        parse_formula(" a9-b2 "),
        Some(Expr::Sub(Box::new((Expr::Cell(9, 0), Expr::Cell(2, 1)))))
    );
    assert_eq!(
        parse_formula("D22+B12+85.5"),
        Some(Expr::Add(Box::new((
            Expr::Cell(22, 3),
            Expr::Add(Box::new((Expr::Cell(12, 1), Expr::Value(85.5))))
        ))))
    );
}

struct RowModel {
    row: usize,
    row_elements: RefCell<Vec<CellContent>>,
    base_model: Weak<CellsModel>,
    notify: slint::ModelNotify,
}

impl slint::Model for RowModel {
    type Data = CellContent;

    fn row_count(&self) -> usize {
        self.row_elements.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.row_elements.borrow().get(row).cloned()
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &self.notify
    }

    fn set_row_data(&self, index: usize, data: CellContent) {
        if let Some(cells) = self.base_model.upgrade() {
            cells.update_cell(self.row, index, Some(data.formula));
        }
    }
}

struct CellsModel {
    rows: Vec<Rc<RowModel>>,
    dep_graph: RefCell<DepGraph<(usize, usize), (usize, usize)>>,
}

impl CellsModel {
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|w| Self {
            rows: (0..ROW_COUNT)
                .map(|row| {
                    Rc::new(RowModel {
                        row,
                        row_elements: vec![CellContent::default(); COL_COUNT].into(),
                        base_model: w.clone(),
                        notify: Default::default(),
                    })
                })
                .collect(),
            dep_graph: Default::default(),
        })
    }

    fn get_cell_value(&self, row: usize, col: usize) -> Option<f32> {
        self.rows.get(row)?.row_elements.borrow().get(col)?.value.into()
    }

    /// Update a cell to a new formula, or re-evaluate the current formula of that cell
    fn update_cell(&self, row: usize, col: usize, new_formula: Option<SharedString>) -> Option<()> {
        let r_model = self.rows.get(row)?;
        let mut r = r_model.row_elements.borrow_mut();
        let data = r.get_mut(col)?;
        let new_form = new_formula.is_some();
        if let Some(new_formula) = new_formula {
            data.formula = new_formula;
        };
        let exp = parse_formula(&data.formula).unwrap_or(Expr::Value(0.));

        drop(r);
        self.dep_graph.borrow_mut().remove_dependencies(&(row, col));
        let new = self.eval(&exp);
        let mut r = r_model.row_elements.borrow_mut();
        let data = r.get_mut(col)?;
        if data.value != new {
            data.value = new;
            drop(r);
            r_model.notify.row_changed(col);
            let deps = self.dep_graph.borrow().dependents(&(row, col)).cloned().collect::<Vec<_>>();
            for (r, c) in deps {
                self.update_cell(r, c, None);
            }
        } else if new_form {
            r_model.notify.row_changed(col);
        }

        make_deps(&mut *self.dep_graph.borrow_mut(), (row, col), &exp);

        Some(())
    }

    /// Evaluate an expression recursively
    fn eval(&self, exp: &Expr) -> f32 {
        match exp {
            Expr::Value(x) => *x,
            Expr::Cell(row, col) => self.get_cell_value(*row, *col).unwrap_or(0.),
            Expr::Add(x) => self.eval(&x.0) + self.eval(&x.1),
            Expr::Sub(x) => self.eval(&x.0) - self.eval(&x.1),
        }
    }
}

/// Traverse a given expression to register the dependencies
fn make_deps(
    graph: &mut DepGraph<(usize, usize), (usize, usize)>,
    orig: (usize, usize),
    exp: &Expr,
) {
    match exp {
        Expr::Value(_) => {}
        Expr::Cell(row, col) => graph.add_dep((*row, *col), orig),
        Expr::Add(x) | Expr::Sub(x) => {
            make_deps(graph, orig, &x.0);
            make_deps(graph, orig, &x.1)
        }
    }
}

impl Model for CellsModel {
    type Data = ModelRc<CellContent>;

    fn row_count(&self) -> usize {
        ROW_COUNT
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.rows.get(row).map(|x| x.clone().into())
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &()
    }
}

pub fn main() {
    let main_window = MainWindow::new().unwrap();
    let cells_model = CellsModel::new();
    main_window.set_cells(ModelRc::from(cells_model));
    main_window.run().unwrap();
}
