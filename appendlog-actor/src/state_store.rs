pub trait AsyncStateStore {
    type State;
    type Error;

    fn load(
        &self,
    ) -> impl std::future::Future<Output = Result<Option<Self::State>, Self::Error>> + Send;
    fn save(
        &self,
        state: &Self::State,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}
