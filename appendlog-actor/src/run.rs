use std::fmt;
use std::sync::Arc;

use appendlog_traits::{AsyncAppender, AsyncConsumer};
use tracing::{debug, info_span, Instrument};

#[cfg(feature = "otel")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

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

impl<CE, AE, SE> std::error::Error for RunError<CE, AE, SE>
where
    CE: fmt::Debug + fmt::Display,
    AE: fmt::Debug + fmt::Display,
    SE: fmt::Debug + fmt::Display,
{
}

pub async fn run<A, C, App, SS>(
    actor: A,
    mut consumer: C,
    appender: App,
    state_store: SS,
) -> Result<(), RunError<C::Error, App::Error, SS::Error>>
where
    A: Actor,
    A::Event: Clone,
    C: AsyncConsumer<Item = A::Event>,
    App: AsyncAppender<Item = A::Event>,
    SS: AsyncStateStore<State = A::State>,
{
    let (mut state, last_index) = match state_store.load().await.map_err(RunError::StateStore)? {
        Some((s, idx)) => (s, Some(idx)),
        None => (A::State::default(), None),
    };

    while let Some(record) = consumer.next().await.map_err(RunError::Consumer)? {
        let index = record.index;

        if let Some(last) = last_index {
            if index <= last {
                debug!(index = u64::from(index), "skipping already-processed event");
                consumer.ack().await.map_err(RunError::Consumer)?;
                continue;
            }
        }

        let span = info_span!("process", index = u64::from(index));

        #[cfg(feature = "otel")]
        if let Some(metadata) = consumer.record_metadata() {
            if let Some(receive) = metadata.downcast_ref::<tracing::Span>() {
                let _ = span.set_parent(receive.context());
            }
        }

        async {
            let event = Arc::unwrap_or_clone(record.data);
            let prev_state = std::mem::take(&mut state);
            let (outputs, new_state) = actor.handle(event, prev_state);
            state = new_state;

            for (seq, output) in outputs.into_iter().enumerate() {
                let output_index = appender.append(output).await.map_err(RunError::Appender)?;
                debug!(seq, output_index = u64::from(output_index), "appended");
            }
            state_store
                .save(&state, index)
                .await
                .map_err(RunError::StateStore)?;
            consumer.ack().await.map_err(RunError::Consumer)?;
            Ok::<_, RunError<C::Error, App::Error, SS::Error>>(())
        }
        .instrument(span)
        .await?;
    }
    Ok(())
}
