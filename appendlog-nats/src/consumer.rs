use async_nats::jetstream::consumer::pull;
use async_nats::jetstream::stream::Stream;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use appendlog_traits::{AsyncConsumer, Index, Record};
use std::marker::PhantomData;

pub struct NatsConsumer<T> {
    messages: pull::Stream,
    pending: Option<(Record<T>, async_nats::jetstream::Message)>,
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
            pending: None,
            _marker: PhantomData,
        }
    }
}

impl<T: DeserializeOwned + Send + Sync> AsyncConsumer for NatsConsumer<T> {
    type Item = T;

    async fn next(&mut self) -> Option<Record<Self::Item>> {
        if let Some((record, _msg)) = &self.pending {
            return Some(record.clone());
        }

        let msg = self.messages.next().await?;
        let msg = msg.ok()?;
        let info = msg.info().ok()?;
        let index = Index::from(info.stream_sequence);
        let data: T = serde_json::from_slice(&msg.payload).ok()?;
        let record = Record::new(index, data);
        self.pending = Some((record.clone(), msg));
        Some(record)
    }

    async fn ack(&mut self) {
        if let Some((_, msg)) = self.pending.take() {
            msg.ack().await.ok();
        }
    }
}
