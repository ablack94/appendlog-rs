use std::fmt;

use appendlog_traits::{AsyncAppender, AsyncConsumer};
use tracing::{debug, info_span, Instrument};

use crate::Handler;

pub enum RunError<CE, AE> {
    Consumer(CE),
    Appender(AE),
    Handler(Box<dyn std::error::Error + Send + Sync>),
}

impl<CE: fmt::Debug, AE: fmt::Debug> fmt::Debug for RunError<CE, AE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Consumer(e) => f.debug_tuple("RunError::Consumer").field(e).finish(),
            RunError::Appender(e) => f.debug_tuple("RunError::Appender").field(e).finish(),
            RunError::Handler(e) => f.debug_tuple("RunError::Handler").field(e).finish(),
        }
    }
}

impl<CE: fmt::Display, AE: fmt::Display> fmt::Display for RunError<CE, AE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Consumer(e) => write!(f, "consumer error: {e}"),
            RunError::Appender(e) => write!(f, "appender error: {e}"),
            RunError::Handler(e) => write!(f, "handler error: {e}"),
        }
    }
}

impl<CE: fmt::Debug + fmt::Display, AE: fmt::Debug + fmt::Display> std::error::Error
    for RunError<CE, AE>
{
}

pub async fn run<E, C, A, H>(
    mut consumer: C,
    appender: A,
    mut handler: H,
) -> Result<(), RunError<C::Error, A::Error>>
where
    C: AsyncConsumer<Item = E>,
    A: AsyncAppender<Item = E>,
    H: Handler<E>,
{
    let last_index = handler.init().await.map_err(RunError::Handler)?;
    while let Some(record) = consumer.next().await.map_err(RunError::Consumer)? {
        let index = record.index;

        // Skip events already processed (state is ahead of consumer after crash)
        if let Some(last) = last_index {
            if index <= last {
                debug!(index = u64::from(index), "skipping already-processed event");
                consumer.ack().await.map_err(RunError::Consumer)?;
                continue;
            }
        }

        async {
            let outputs = handler.handle(&record.data);
            for (seq, output) in outputs.into_iter().enumerate() {
                let output_index = appender.append(output).await.map_err(RunError::Appender)?;
                debug!(seq, output_index = u64::from(output_index), "appended");
            }
            handler.save_state(index).await.map_err(RunError::Handler)?;
            consumer.ack().await.map_err(RunError::Consumer)?;
            Ok::<_, RunError<C::Error, A::Error>>(())
        }
        .instrument(info_span!("process", index = u64::from(index)))
        .await?;
    }
    Ok(())
}
