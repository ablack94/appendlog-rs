use crate::Record;

pub trait AsyncConsumer {
    type Item;
    type Error;

    fn next(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<Record<Self::Item>>, Self::Error>> + Send;
    fn ack(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}
