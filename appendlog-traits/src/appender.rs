use crate::Index;

pub trait Appender {
    type Item;

    fn append(&self, item: Self::Item) -> Index;
}

pub trait AsyncAppender {
    type Item;
    type Error;

    fn append(&self, item: Self::Item) -> impl std::future::Future<Output = Result<Index, Self::Error>> + Send;
}
