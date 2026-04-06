use std::fmt;
use std::sync::Arc;

use appendlog_traits::{AsyncAppender, AsyncConsumer};
use tracing::{debug, info_span, Instrument};

#[cfg(feature = "otel")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub enum BridgeError<CE, AE> {
    Consumer(CE),
    Appender(AE),
}

impl<CE: fmt::Debug, AE: fmt::Debug> fmt::Debug for BridgeError<CE, AE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::Consumer(e) => f.debug_tuple("BridgeError::Consumer").field(e).finish(),
            BridgeError::Appender(e) => f.debug_tuple("BridgeError::Appender").field(e).finish(),
        }
    }
}

impl<CE: fmt::Display, AE: fmt::Display> fmt::Display for BridgeError<CE, AE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::Consumer(e) => write!(f, "consumer error: {e}"),
            BridgeError::Appender(e) => write!(f, "appender error: {e}"),
        }
    }
}

impl<CE: fmt::Debug + fmt::Display, AE: fmt::Debug + fmt::Display> std::error::Error
    for BridgeError<CE, AE>
{
}

pub async fn bridge<T, C, A>(
    mut consumer: C,
    appender: A,
) -> Result<(), BridgeError<C::Error, A::Error>>
where
    C: AsyncConsumer<Item = T>,
    A: AsyncAppender<Item = T>,
    T: Clone,
{
    while let Some(record) = consumer.next().await.map_err(BridgeError::Consumer)? {
        let index = record.index;
        let span = info_span!("bridge", index = u64::from(index));

        #[cfg(feature = "otel")]
        if let Some(metadata) = consumer.record_metadata() {
            if let Some(receive) = metadata.downcast_ref::<tracing::Span>() {
                let _ = span.set_parent(receive.context());
            }
        }

        async {
            let output_index = appender
                .append(Arc::unwrap_or_clone(record.data))
                .await
                .map_err(BridgeError::Appender)?;
            debug!(output_index = u64::from(output_index), "forwarded");
            consumer.ack().await.map_err(BridgeError::Consumer)?;
            Ok::<_, BridgeError<C::Error, A::Error>>(())
        }
        .instrument(span)
        .await?;
    }
    Ok(())
}

pub async fn bridge_map<In, Out, C, A, F>(
    mut consumer: C,
    appender: A,
    f: F,
) -> Result<(), BridgeError<C::Error, A::Error>>
where
    C: AsyncConsumer<Item = In>,
    A: AsyncAppender<Item = Out>,
    In: Clone,
    F: Fn(In) -> Option<Out>,
{
    while let Some(record) = consumer.next().await.map_err(BridgeError::Consumer)? {
        let index = record.index;
        let span = info_span!("bridge_map", index = u64::from(index));

        #[cfg(feature = "otel")]
        if let Some(metadata) = consumer.record_metadata() {
            if let Some(receive) = metadata.downcast_ref::<tracing::Span>() {
                let _ = span.set_parent(receive.context());
            }
        }

        async {
            if let Some(out) = f(Arc::unwrap_or_clone(record.data)) {
                let output_index = appender.append(out).await.map_err(BridgeError::Appender)?;
                debug!(output_index = u64::from(output_index), "forwarded");
            } else {
                debug!("filtered");
            }
            consumer.ack().await.map_err(BridgeError::Consumer)?;
            Ok::<_, BridgeError<C::Error, A::Error>>(())
        }
        .instrument(span)
        .await?;
    }
    Ok(())
}
