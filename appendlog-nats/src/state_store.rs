use async_nats::jetstream::{self, kv};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

pub struct NatsStateStore<S> {
    store: kv::Store,
    key: String,
    _marker: PhantomData<S>,
}

impl<S> NatsStateStore<S> {
    pub async fn new(context: jetstream::Context, bucket: &str, key: &str) -> Self {
        let store = context
            .get_key_value(bucket)
            .await
            .expect("failed to open KV bucket");
        Self {
            store,
            key: key.to_string(),
            _marker: PhantomData,
        }
    }
}

impl<S: Serialize + DeserializeOwned + Send + Sync> appendlog_actor::AsyncStateStore
    for NatsStateStore<S>
{
    type State = S;

    async fn load(&self) -> Option<Self::State> {
        let entry = self.store.get(&self.key).await.ok()?;
        let bytes = entry?;
        serde_json::from_slice(&bytes).ok()
    }

    async fn save(&self, state: &Self::State) {
        let bytes = serde_json::to_vec(state).expect("failed to serialize state");
        if let Err(err) = self.store.put(&self.key, bytes.into()).await {
            eprintln!("warning: failed to save state: {err}");
        }
    }
}
