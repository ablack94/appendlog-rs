use crate::Index;

pub trait Appender {
    type Item;

    fn append(&self, item: Self::Item) -> Index;
}

pub trait AsyncAppender {
    type Item;

    fn append(&self, item: Self::Item) -> impl std::future::Future<Output = Index> + Send;
}
