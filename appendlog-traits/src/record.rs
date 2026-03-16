use crate::Index;

#[derive(Debug)]
pub struct Record<T> {
    pub index: Index,
    pub data: T,
}
