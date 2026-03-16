use async_nats::jetstream::consumer::pull;
use async_nats::jetstream::stream::Stream;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use appendlog_traits::{AsyncConsumer, Index, Record};
use std::marker::PhantomData;

pub struct NatsConsumer<T> {
    messages: pull::Stream,
    pending: Option<(Index, T, async_nats::jetstream::Message)>,
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

impl<T: DeserializeOwned + Clone + Send> AsyncConsumer for NatsConsumer<T> {
    type Item = T;

    async fn next(&mut self) -> Option<Record<Self::Item>> {
        if let Some((index, data, _msg)) = &self.pending {
            return Some(Record { index: *index, data: data.clone() });
        }

        let msg = self.messages.next().await?;
        let msg = msg.ok()?;
        let info = msg.info().ok()?;
        let index = Index::from(info.stream_sequence);
        let data: T = serde_json::from_slice(&msg.payload).ok()?;
        self.pending = Some((index, data.clone(), msg));
        Some(Record { index, data })
    }

    async fn ack(&mut self) {
        if let Some((_index, _data, msg)) = self.pending.take() {
            msg.ack().await.ok();
        }
    }
}
