use crate::{Index, Record};

pub trait Lookup {
    type Item;

    fn get(&self, index: Index) -> Option<Record<Self::Item>>;
}

pub trait AsyncLookup {
    type Item;

    fn get(
        &self,
        index: Index,
    ) -> impl std::future::Future<Output = Option<Record<Self::Item>>> + Send;
}
