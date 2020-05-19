/*!
    Property binding engine.

    The current implementation uses lots of heap allocation but that can be optimized later using
    thin dst container, and intrusive linked list
*/

use std::cell::RefCell;
use std::{
    ops::DerefMut,
    rc::{Rc, Weak},
};

thread_local!(static CURRENT_PROPERTY : RefCell<Option<Rc<dyn PropertyNotify>>> = Default::default());

#[derive(Default)]
struct PropertyImpl<T> {
    binding: Option<Box<dyn Fn() -> T>>,
    dependencies: Vec<Weak<dyn PropertyNotify>>,
    //updating: bool,
    value: T,
}

trait PropertyNotify {
    fn update_dependencies(self: Rc<Self>);
    fn update(self: Rc<Self>);
}

impl<T: 'static> PropertyNotify for RefCell<PropertyImpl<T>> {
    fn update_dependencies(self: Rc<Self>) {
        let mut v = vec![];
        {
            let mut dep = self.borrow_mut();
            std::mem::swap(&mut dep.dependencies, &mut v);
        }
        for d in &v {
            if let Some(d) = d.upgrade() {
                d.update();
            }
        }
    }
    fn update(self: Rc<Self>) {
        let new_val = if let Some(binding) = &self.borrow().binding {
            //if self.updating.get() {
            //    panic!("Circular dependency found : {}", self.description());
            //}
            //self.updating.set(true);

            let mut old: Option<Rc<dyn PropertyNotify>> = Some(self.clone());

            CURRENT_PROPERTY.with(|cur_dep| {
                let mut m = cur_dep.borrow_mut();
                std::mem::swap(m.deref_mut(), &mut old);
            });
            let new_val = binding();
            CURRENT_PROPERTY.with(|cur_dep| {
                let mut m = cur_dep.borrow_mut();
                std::mem::swap(m.deref_mut(), &mut old);
                //somehow ptr_eq does not work as expected despite the pointer are equal
                //debug_assert!(Rc::ptr_eq(&(self.clone() as Rc<dyn PropertyNotify>), &old.unwrap()));
            });
            new_val
        } else {
            return;
        };
        self.borrow_mut().value = new_val;
        self.update_dependencies();
    }
}

#[repr(C)]
#[derive(Default, Clone)]
pub struct Property<T: 'static> {
    inner: Rc<RefCell<PropertyImpl<T>>>,
    //value: T,
}

impl<T: Clone + 'static> Property<T> {
    pub fn get(&self) -> T {
        self.notify();
        self.inner.borrow().value.clone()
    }

    fn notify(&self) {
        CURRENT_PROPERTY.with(|cur_dep| {
            if let Some(m) = &(*cur_dep.borrow()) {
                self.inner.borrow_mut().dependencies.push(Rc::downgrade(m));
            }
        });
    }

    pub fn set(&self, t: T) {
        self.inner.borrow_mut().binding = None;
        self.inner.borrow_mut().value = t;
        self.inner.clone().update_dependencies();
    }

    pub fn set_binding(&self, f: Box<dyn Fn() -> T>) {
        self.inner.borrow_mut().binding = Some(f);
        self.inner.clone().update()
    }
}

#[test]
fn properties_simple_test() {
    let width = Property::<i32>::default();
    let height = Property::<i32>::default();
    let area = Property::<i32>::default();
    area.set_binding(Box::new({
        let (width, height) = (width.clone(), height.clone());
        move || width.get() * height.get()
    }));
    width.set(4);
    height.set(8);
    assert_eq!(width.get(), 4);
    assert_eq!(height.get(), 8);
    assert_eq!(area.get(), 4 * 8);

    width.set_binding(Box::new({
        let height = height.clone();
        move || height.get() * 2
    }));
    assert_eq!(width.get(), 8 * 2);
    assert_eq!(height.get(), 8);
    assert_eq!(area.get(), 8 * 8 * 2);
}
