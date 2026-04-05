pub trait Actor {
    type Input;
    type Output;
    type State: Default;
    type Outputs: IntoIterator<Item = Self::Output>;

    fn handle(&self, input: Self::Input, state: Self::State) -> (Self::Outputs, Self::State);
}
