pub trait AsyncStateStore {
    type State;

    fn load(&self) -> impl std::future::Future<Output = Option<Self::State>> + Send;
    fn save(&self, state: &Self::State) -> impl std::future::Future<Output = ()> + Send;
}
