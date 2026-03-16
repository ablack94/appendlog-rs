use crate::Record;

pub trait AsyncConsumer {
    type Item;

    fn next(&mut self) -> impl std::future::Future<Output = Option<Record<Self::Item>>> + Send;
    fn ack(&mut self) -> impl std::future::Future<Output = ()> + Send;
}
