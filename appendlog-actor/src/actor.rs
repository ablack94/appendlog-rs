pub trait Actor {
    type Event;
    type State: Default;
    type Outputs: IntoIterator<Item = Self::Event>;

    fn handle(&self, event: Self::Event, state: Self::State) -> (Self::Outputs, Self::State);
}
