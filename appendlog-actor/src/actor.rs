pub trait Actor {
    type Input;
    type Output;
    type State: Default;

    fn handle(&self, input: Self::Input, state: Self::State) -> (Vec<Self::Output>, Self::State);
}
