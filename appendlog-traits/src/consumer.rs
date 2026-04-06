use crate::Record;

pub trait AsyncConsumer {
    type Item;
    type Error;

    fn next(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<Record<Self::Item>>, Self::Error>> + Send;
    fn ack(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;

    /// Returns transport-layer metadata for the most recently fetched record.
    /// Backends can override this to provide context propagation data (e.g., OTel context).
    fn record_metadata(&self) -> Option<&(dyn std::any::Any + Send + Sync)> {
        None
    }
}
