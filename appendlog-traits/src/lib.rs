#[derive(Debug, Copy, Clone)]
pub struct Index(u64);

impl Index {
    pub fn new(index: impl Into<u64>) -> Self {
        Self(index.into())
    }
}

impl Default for Index {
    fn default() -> Self {
        Self(0)
    }
}

impl From<u64> for Index {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Index> for u64 {
    fn from(value: Index) -> Self {
        value.0
    }
}

impl TryFrom<Index> for usize {
    type Error = std::num::TryFromIntError;

    fn try_from(value: Index) -> Result<Self, Self::Error> {
        value.0.try_into()
    }
}

pub trait Appender {
    type Item;

    fn append(&self, item: Self::Item) -> Index;
}

pub trait Lookup {
    type Item;

    fn get(&self, index: Index) -> Option<Self::Item>;
}

pub trait AsyncAppender {
    type Item;

    fn append(&self, item: Self::Item) -> impl std::future::Future<Output = Index> + Send;
}

pub trait AsyncLookup {
    type Item;

    fn get(&self, index: Index) -> impl std::future::Future<Output = Option<Self::Item>> + Send;
}
