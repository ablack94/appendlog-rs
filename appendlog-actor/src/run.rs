use std::sync::Arc;

use appendlog_traits::{AsyncAppender, AsyncConsumer};

use crate::{Actor, AsyncStateStore};

pub async fn run<A, C, P, SS>(
    actor: A,
    mut consumer: C,
    appender: P,
    state_store: SS,
) -> A::State
where
    A: Actor<Input: Clone>,
    C: AsyncConsumer<Item = A::Input>,
    P: AsyncAppender<Item = A::Output>,
    SS: AsyncStateStore<State = A::State>,
{
    let mut state = state_store.load().await.unwrap_or_default();
    while let Some(record) = consumer.next().await {
        let (outputs, new_state) = actor.handle(Arc::unwrap_or_clone(record.data), state);
        state = new_state;
        for output in outputs {
            appender.append(output).await;
        }
        state_store.save(&state).await;
        consumer.ack().await;
    }
    state
}
