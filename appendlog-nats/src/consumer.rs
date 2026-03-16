use async_nats::jetstream::consumer::pull;
use async_nats::jetstream::stream::Stream;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use appendlog_traits::{AsyncConsumer, Index, Record};
use std::marker::PhantomData;

enum ConsumerState<T> {
    Empty,
    Fetched(async_nats::jetstream::Message),
    Parsed(Record<T>, async_nats::jetstream::Message),
}

pub struct NatsConsumer<T> {
    messages: pull::Stream,
    state: ConsumerState<T>,
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
            _marker: PhantomData,
        }
    }
}

fn try_parse<T: DeserializeOwned>(msg: &async_nats::jetstream::Message) -> Option<Record<T>> {
    let info = msg.info().ok()?;
    let index = Index::from(info.stream_sequence);
    let data: T = serde_json::from_slice(&msg.payload).ok()?;
    Some(Record::new(index, data))
}

impl<T: DeserializeOwned + Send + Sync> AsyncConsumer for NatsConsumer<T> {
    type Item = T;

    async fn next(&mut self) -> Option<Record<Self::Item>> {
        let msg = match std::mem::replace(&mut self.state, ConsumerState::Empty) {
            ConsumerState::Parsed(record, msg) => {
                self.state = ConsumerState::Parsed(record.clone(), msg);
                return Some(record);
            }
            ConsumerState::Fetched(msg) => msg,
            ConsumerState::Empty => {
                let msg = self.messages.next().await?;
                msg.ok()?
            }
        };

        if let Some(record) = try_parse(&msg) {
            self.state = ConsumerState::Parsed(record.clone(), msg);
            Some(record)
        } else {
            self.state = ConsumerState::Fetched(msg);
            None
        }
    }

    async fn ack(&mut self) {
        let prev = std::mem::replace(&mut self.state, ConsumerState::Empty);
        if let ConsumerState::Parsed(_, msg) | ConsumerState::Fetched(msg) = prev {
            msg.ack().await.ok();
        }
    }
}
