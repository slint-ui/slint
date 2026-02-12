use std::sync::Arc;

use arc_swap::ArcSwapOption;
use event_listener::{Event, IntoNotification};

pub struct Watcher<T> {
    inner: Event<Option<Arc<T>>>,
    data: ArcSwapOption<T>,
}

impl<T> Watcher<T> {
    pub const fn new() -> Self {
        Self { inner: Event::with_tag(), data: ArcSwapOption::const_empty() }
    }

    pub fn set(&self, data: T) {
        let data = Some(Arc::new(data));
        self.data.store(data.clone());
        self.inner.notify(usize::MAX.tag(data));
    }

    #[allow(unused)]
    pub fn clear(&self) {
        self.data.store(None);
        self.inner.notify(usize::MAX.tag(None));
    }

    pub fn get(&self) -> Option<Arc<T>> {
        self.data.load_full()
    }

    pub fn listener(&self) -> event_listener::EventListener<Option<Arc<T>>> {
        self.inner.listen()
    }
}

impl<T> Default for Watcher<T> {
    fn default() -> Self {
        Self::new()
    }
}
