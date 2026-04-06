use appendlog_traits::{AsyncAppender, AsyncLookup, Index, Record};
use async_nats::jetstream::{self, stream::Stream};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

#[cfg(feature = "otel")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::NatsConsumer;

pub struct NatsLog<T> {
    context: jetstream::Context,
    stream: Stream,
    subject: String,
    _marker: PhantomData<T>,
}

impl<T> NatsLog<T> {
    pub async fn new(
        client: async_nats::Client,
        stream_name: &str,
        subject: &str,
    ) -> Result<Self, async_nats::error::Error<jetstream::context::CreateStreamErrorKind>> {
        let context = jetstream::new(client);
        let stream = context
            .get_or_create_stream(jetstream::stream::Config {
                name: stream_name.to_string(),
                subjects: vec![subject.to_string()],
                ..Default::default()
            })
            .await?;

        Ok(Self {
            context,
            stream,
            subject: subject.to_string(),
            _marker: PhantomData,
        })
    }

    pub async fn consumer(&self, consumer_name: &str) -> NatsConsumer<T> {
        NatsConsumer::new(&self.stream, consumer_name).await
    }
}

#[derive(Debug)]
pub enum NatsAppendError {
    Serialize(serde_json::Error),
    Publish(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for NatsAppendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatsAppendError::Serialize(e) => write!(f, "serialize error: {e}"),
            NatsAppendError::Publish(e) => write!(f, "publish error: {e}"),
        }
    }
}

impl std::error::Error for NatsAppendError {}

impl<T: Serialize + Send + Sync> AsyncAppender for NatsLog<T> {
    type Item = T;
    type Error = NatsAppendError;

    async fn append(&self, item: Self::Item) -> Result<Index, Self::Error> {
        let payload = serde_json::to_vec(&item).map_err(NatsAppendError::Serialize)?;

        #[cfg(feature = "otel")]
        let ack = {
            use tracing::{info_span, Instrument};
            async {
                let mut headers = async_nats::HeaderMap::new();
                let cx = tracing::Span::current().context();
                opentelemetry::global::get_text_map_propagator(|propagator| {
                    propagator.inject_context(&cx, &mut crate::otel::HeaderInjector(&mut headers));
                });
                self.context
                    .publish_with_headers(self.subject.clone(), headers, payload.into())
                    .await
                    .map_err(|e| NatsAppendError::Publish(Box::new(e)))
            }
            .instrument(info_span!("publish", subject = %self.subject))
            .await?
        };

        #[cfg(not(feature = "otel"))]
        let ack = self
            .context
            .publish(self.subject.clone(), payload.into())
            .await
            .map_err(|e| NatsAppendError::Publish(Box::new(e)))?;

        let ack = ack
            .await
            .map_err(|e| NatsAppendError::Publish(Box::new(e)))?;
        Ok(Index::from(ack.sequence))
    }
}

impl<T: DeserializeOwned + Send + Sync> AsyncLookup for NatsLog<T> {
    type Item = T;

    async fn get(&self, index: Index) -> Option<Record<Self::Item>> {
        let sequence: u64 = index.into();
        let raw = self.stream.get_raw_message(sequence).await.ok()?;
        let data: T = serde_json::from_slice(&raw.payload).ok()?;
        Some(Record::new(index, data))
    }
}
