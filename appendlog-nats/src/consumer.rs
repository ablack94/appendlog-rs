use appendlog_traits::{AsyncConsumer, Index, Record};
use async_nats::jetstream::consumer::pull;
use async_nats::jetstream::stream::Stream;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

#[cfg(feature = "otel")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

enum ConsumerState<T> {
    Empty,
    Fetched(async_nats::jetstream::Message),
    Parsed(Record<T>, async_nats::jetstream::Message),
}

pub struct NatsConsumer<T> {
    messages: pull::Stream,
    state: ConsumerState<T>,
    #[cfg(feature = "otel")]
    receive_span: Option<tracing::Span>,
    _marker: PhantomData<T>,
}

impl<T> NatsConsumer<T> {
    pub(crate) async fn new(stream: &Stream, consumer_name: &str) -> Self {
        let consumer = stream
            .create_consumer(pull::Config {
                durable_name: Some(consumer_name.to_string()),
                ..Default::default()
            })
            .await
            .expect("failed to create consumer");

        let messages = consumer.messages().await.expect("failed to get messages");

        NatsConsumer {
            messages,
            state: ConsumerState::Empty,
            #[cfg(feature = "otel")]
            receive_span: None,
            _marker: PhantomData,
        }
    }
}

struct ParsedMessage<T> {
    record: Record<T>,
    #[cfg(feature = "otel")]
    parent_context: Option<opentelemetry::Context>,
    #[cfg(feature = "otel")]
    subject: String,
}

fn try_parse<T: DeserializeOwned>(
    msg: &async_nats::jetstream::Message,
) -> Option<ParsedMessage<T>> {
    let info = msg.info().ok()?;
    let index = Index::from(info.stream_sequence);
    let data: T = serde_json::from_slice(&msg.payload).ok()?;

    #[cfg(feature = "otel")]
    let parent_context = msg.headers.as_ref().map(|headers| {
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&crate::otel::HeaderExtractor(headers))
        })
    });

    Some(ParsedMessage {
        record: Record::new(index, data),
        #[cfg(feature = "otel")]
        parent_context,
        #[cfg(feature = "otel")]
        subject: msg.subject.to_string(),
    })
}

#[derive(Debug)]
pub enum NatsConsumerError {
    Messages(Box<dyn std::error::Error + Send + Sync>),
    Ack(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for NatsConsumerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatsConsumerError::Messages(e) => write!(f, "consumer messages error: {e}"),
            NatsConsumerError::Ack(e) => write!(f, "consumer ack error: {e}"),
        }
    }
}

impl std::error::Error for NatsConsumerError {}

impl<T: DeserializeOwned + Send + Sync> AsyncConsumer for NatsConsumer<T> {
    type Item = T;
    type Error = NatsConsumerError;

    async fn next(&mut self) -> Result<Option<Record<Self::Item>>, Self::Error> {
        let msg = match std::mem::replace(&mut self.state, ConsumerState::Empty) {
            ConsumerState::Parsed(record, msg) => {
                self.state = ConsumerState::Parsed(record.clone(), msg);
                return Ok(Some(record));
            }
            ConsumerState::Fetched(msg) => msg,
            ConsumerState::Empty => {
                let Some(result) = self.messages.next().await else {
                    return Ok(None);
                };
                result.map_err(|e| NatsConsumerError::Messages(Box::new(e)))?
            }
        };

        if let Some(parsed) = try_parse(&msg) {
            #[cfg(feature = "otel")]
            {
                let index = parsed.record.index;
                let span = tracing::info_span!(
                    "receive",
                    subject = %parsed.subject,
                    index = u64::from(index),
                );
                if let Some(cx) = parsed.parent_context {
                    let _ = span.set_parent(cx);
                }
                // Enter briefly so the OTel layer registers the span
                span.in_scope(|| {});
                self.receive_span = Some(span);
            }

            self.state = ConsumerState::Parsed(parsed.record.clone(), msg);
            Ok(Some(parsed.record))
        } else {
            self.state = ConsumerState::Fetched(msg);
            Ok(None)
        }
    }

    async fn ack(&mut self) -> Result<(), Self::Error> {
        let prev = std::mem::replace(&mut self.state, ConsumerState::Empty);
        if let ConsumerState::Parsed(_, msg) | ConsumerState::Fetched(msg) = prev {
            msg.ack().await.map_err(NatsConsumerError::Ack)?;
        }
        // Drop the receive span on ack, ending it
        #[cfg(feature = "otel")]
        {
            self.receive_span.take();
        }
        Ok(())
    }

    fn record_metadata(&self) -> Option<&(dyn std::any::Any + Send + Sync)> {
        #[cfg(feature = "otel")]
        {
            self.receive_span.as_ref().map(|span| {
                // Return the span itself — the run loop extracts its OTel context
                span as &(dyn std::any::Any + Send + Sync)
            })
        }
        #[cfg(not(feature = "otel"))]
        {
            None
        }
    }
}
