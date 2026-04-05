use std::fmt;
use std::sync::Arc;

use appendlog_traits::{AsyncAppender, AsyncConsumer};

use crate::{Actor, AsyncStateStore};

pub enum RunError<CE, AE, SE> {
    Consumer(CE),
    Appender(AE),
    StateStore(SE),
}

impl<CE: fmt::Debug, AE: fmt::Debug, SE: fmt::Debug> fmt::Debug for RunError<CE, AE, SE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Consumer(e) => f.debug_tuple("RunError::Consumer").field(e).finish(),
            RunError::Appender(e) => f.debug_tuple("RunError::Appender").field(e).finish(),
            RunError::StateStore(e) => f.debug_tuple("RunError::StateStore").field(e).finish(),
        }
    }
}

impl<CE: fmt::Display, AE: fmt::Display, SE: fmt::Display> fmt::Display for RunError<CE, AE, SE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Consumer(e) => write!(f, "consumer error: {e}"),
            RunError::Appender(e) => write!(f, "appender error: {e}"),
            RunError::StateStore(e) => write!(f, "state store error: {e}"),
        }
    }
}

impl<
        CE: fmt::Debug + fmt::Display,
        AE: fmt::Debug + fmt::Display,
        SE: fmt::Debug + fmt::Display,
    > std::error::Error for RunError<CE, AE, SE>
{
}

pub async fn run<A, C, P, SS>(
    actor: A,
    mut consumer: C,
    appender: P,
    state_store: SS,
) -> Result<A::State, RunError<C::Error, P::Error, SS::Error>>
where
    A: Actor<Input: Clone>,
    C: AsyncConsumer<Item = A::Input>,
    P: AsyncAppender<Item = A::Output>,
    SS: AsyncStateStore<State = A::State>,
{
    let mut state = state_store
        .load()
        .await
        .map_err(RunError::StateStore)?
        .unwrap_or_default();
    while let Some(record) = consumer.next().await.map_err(RunError::Consumer)? {
        let (outputs, new_state) = actor.handle(Arc::unwrap_or_clone(record.data), state);
        state = new_state;
        for output in outputs {
            appender.append(output).await.map_err(RunError::Appender)?;
        }
        state_store
            .save(&state)
            .await
            .map_err(RunError::StateStore)?;
        consumer.ack().await.map_err(RunError::Consumer)?;
    }
    Ok(state)
}
