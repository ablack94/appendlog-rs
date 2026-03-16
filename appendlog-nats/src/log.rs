use async_nats::jetstream::{self, stream::Stream};
use serde::{de::DeserializeOwned, Serialize};
use appendlog_traits::{AsyncAppender, AsyncLookup, Index, Record};
use std::marker::PhantomData;

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

impl<T: Serialize + Send + Sync> AsyncAppender for NatsLog<T> {
    type Item = T;

    async fn append(&self, item: Self::Item) -> Index {
        let payload = serde_json::to_vec(&item).expect("failed to serialize item");
        let ack = self
            .context
            .publish(self.subject.clone(), payload.into())
            .await
            .expect("failed to publish");
        let ack = ack.await.expect("failed to get publish ack");
        Index::from(ack.sequence)
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
