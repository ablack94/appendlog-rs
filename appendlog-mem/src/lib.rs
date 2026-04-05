use std::{collections::VecDeque, sync::Arc};

use appendlog_traits::{Appender, Index, Lookup, Record};
use parking_lot::{Condvar, Mutex};

struct LogInner<T> {
    entries: VecDeque<T>,
    closed: bool,
}

struct LogState<T> {
    cond_added: Condvar,
    inner: Mutex<LogInner<T>>,
}

impl<T> LogState<T> {
    fn new() -> Self {
        Self {
            cond_added: Condvar::new(),
            inner: Mutex::new(LogInner {
                entries: VecDeque::new(),
                closed: false,
            }),
        }
    }

    fn append(&self, item: T) -> Index {
        let mut inner = self.inner.lock();
        let pos = inner.entries.len() as u64;
        inner.entries.push_back(item);
        drop(inner);
        self.cond_added.notify_all();
        Index::new(pos)
    }
}

impl<T: Clone> LogState<T> {
    fn get(&self, index: usize) -> Option<T> {
        self.inner.lock().entries.get(index).cloned()
    }

    fn wait_for_index(&self, index: usize) -> Option<T> {
        let mut inner = self.inner.lock();
        loop {
            if let Some(item) = inner.entries.get(index).cloned() {
                return Some(item);
            }
            if inner.closed {
                return None;
            }
            self.cond_added.wait(&mut inner);
        }
    }
}

pub struct Log<T> {
    state: Arc<LogState<T>>,
}

impl<T> Log<T> {
    pub fn new() -> Self {
        Self {
            state: Arc::new(LogState::new()),
        }
    }
}

impl<T> Drop for Log<T> {
    fn drop(&mut self) {
        self.state.inner.lock().closed = true;
        self.state.cond_added.notify_all();
    }
}

impl<T: Clone> Appender for Log<T> {
    type Item = T;

    fn append(&self, item: Self::Item) -> Index {
        self.state.append(item)
    }
}

impl<T: Clone> Lookup for Log<T> {
    type Item = T;

    fn get(&self, index: Index) -> Option<Record<Self::Item>> {
        match index.try_into() {
            Ok(index_usize) => self
                .state
                .get(index_usize)
                .map(|data| Record::new(index, data)),
            Err(_) => None,
        }
    }
}

pub struct LogConsumer<T> {
    index: usize,
    state: Arc<LogState<T>>,
}

impl<T: Clone> LogConsumer<T> {
    pub fn new(log: &Log<T>) -> Self {
        Self {
            index: 0,
            state: Arc::clone(&log.state),
        }
    }
}

impl<T: Clone> Iterator for LogConsumer<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.state.wait_for_index(self.index)?;
        self.index += 1;
        Some(item)
    }
}
