use std::error::Error;
use std::future::Future;
use std::mem;

use appendlog_traits::Index;

use crate::{Actor, AsyncStateStore};

pub trait Handler<E>: Send {
    fn init(
        &mut self,
    ) -> impl Future<Output = Result<Option<Index>, Box<dyn Error + Send + Sync>>> + Send;

    fn handle(&mut self, event: &E) -> Vec<E>;
    
    fn save_state(
        &mut self,
        index: Index,
    ) -> impl Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send;
}

pub struct ActorHandler<A: Actor, SS> {
    actor: A,
    state: A::State,
    state_store: SS,
}

impl<A: Actor, SS> ActorHandler<A, SS> {
    pub fn new(actor: A, state_store: SS) -> Self {
        Self {
            actor,
            state: A::State::default(),
            state_store,
        }
    }
}

impl<E, A, SS> Handler<E> for ActorHandler<A, SS>
where
    A: Actor + Send,
    A::Event: for<'a> TryFrom<&'a E> + Into<E>,
    A::State: Send,
    A::Outputs: Send,
    SS: AsyncStateStore<State = A::State> + Send,
    SS::Error: Error + Send + Sync + 'static,
{
    async fn init(&mut self) -> Result<Option<Index>, Box<dyn Error + Send + Sync>> {
        if let Some((state, index)) = self.state_store.load().await.map_err(Box::new)? {
            self.state = state;
            Ok(Some(index))
        } else {
            Ok(None)
        }
    }

    fn handle(&mut self, event: &E) -> Vec<E> {
        let Ok(actor_event) = A::Event::try_from(event) else {
            return Vec::new();
        };
        let state = mem::take(&mut self.state);
        let (outputs, new_state) = self.actor.handle(actor_event, state);
        self.state = new_state;
        outputs.into_iter().map(Into::into).collect()
    }

    async fn save_state(&mut self, index: Index) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.state_store
            .save(&self.state, index)
            .await
            .map_err(|e| Box::new(e) as _)
    }
}

// Tuple implementations for composing handlers

macro_rules! impl_handler_for_tuple {
    ($($idx:tt $H:ident),+) => {
        impl<E, $($H),+> Handler<E> for ($($H,)+)
        where
            E: Send,
            $($H: Handler<E>,)+
        {
            async fn init(&mut self) -> Result<Option<Index>, Box<dyn Error + Send + Sync>> {
                let mut min: Option<Index> = None;
                $(
                    match self.$idx.init().await? {
                        Some(idx) => {
                            min = Some(match min {
                                Some(m) if m < idx => m,
                                _ => idx,
                            });
                        }
                        None => return Ok(None),
                    }
                )+
                Ok(min)
            }

            fn handle(&mut self, event: &E) -> Vec<E> {
                let mut out = Vec::new();
                $({
                    let _span = tracing::info_span!("handler", slot = $idx).entered();
                    out.extend(self.$idx.handle(event));
                })+
                out
            }

            async fn save_state(&mut self, index: Index) -> Result<(), Box<dyn Error + Send + Sync>> {
                $(self.$idx.save_state(index).await?;)+
                Ok(())
            }
        }
    };
}

impl_handler_for_tuple!(0 H0);
impl_handler_for_tuple!(0 H0, 1 H1);
impl_handler_for_tuple!(0 H0, 1 H1, 2 H2);
impl_handler_for_tuple!(0 H0, 1 H1, 2 H2, 3 H3);
impl_handler_for_tuple!(0 H0, 1 H1, 2 H2, 3 H3, 4 H4);
