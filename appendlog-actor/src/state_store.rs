use appendlog_traits::Index;

impl AsyncStateStore for () {
    type State = ();
    type Error = std::convert::Infallible;

    async fn load(&self) -> Result<Option<((), Index)>, Self::Error> {
        Ok(None)
    }

    async fn save(&self, _: &(), _: Index) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub trait AsyncStateStore {
    type State;
    type Error;

    fn load(
        &self,
    ) -> impl std::future::Future<Output = Result<Option<(Self::State, Index)>, Self::Error>> + Send;
    fn save(
        &self,
        state: &Self::State,
        index: Index,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}
