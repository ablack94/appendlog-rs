use std::fmt;
use std::sync::Arc;

use crate::Index;

pub struct Record<T> {
    pub index: Index,
    pub data: Arc<T>,
}

impl<T> Record<T> {
    pub fn new(index: Index, data: T) -> Self {
        Self {
            index,
            data: Arc::new(data),
        }
    }
}

impl<T> Clone for Record<T> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            data: Arc::clone(&self.data),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Record<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Record")
            .field("index", &self.index)
            .field("data", &self.data)
            .finish()
    }
}
