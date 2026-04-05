use appendlog_traits::Index;
use async_nats::jetstream::{self, kv};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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

#[derive(Serialize, Deserialize)]
struct Checkpoint<S> {
    state: S,
    index: u64,
}

#[derive(Debug)]
pub enum NatsStateStoreError {
    Serialize(serde_json::Error),
    Kv(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for NatsStateStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatsStateStoreError::Serialize(e) => write!(f, "state serialize error: {e}"),
            NatsStateStoreError::Kv(e) => write!(f, "state kv error: {e}"),
        }
    }
}

impl std::error::Error for NatsStateStoreError {}

impl<S: Serialize + DeserializeOwned + Send + Sync> appendlog_actor::AsyncStateStore
    for NatsStateStore<S>
{
    type State = S;
    type Error = NatsStateStoreError;

    async fn load(&self) -> Result<Option<(Self::State, Index)>, Self::Error> {
        let entry = self
            .store
            .get(&self.key)
            .await
            .map_err(|e| NatsStateStoreError::Kv(Box::new(e)))?;
        let Some(bytes) = entry else {
            return Ok(None);
        };
        let checkpoint: Checkpoint<S> =
            serde_json::from_slice(&bytes).map_err(NatsStateStoreError::Serialize)?;
        Ok(Some((checkpoint.state, Index::from(checkpoint.index))))
    }

    async fn save(&self, state: &Self::State, index: Index) -> Result<(), Self::Error> {
        let checkpoint = Checkpoint {
            state,
            index: u64::from(index),
        };
        let bytes = serde_json::to_vec(&checkpoint).map_err(NatsStateStoreError::Serialize)?;
        self.store
            .put(&self.key, bytes.into())
            .await
            .map_err(|e| NatsStateStoreError::Kv(Box::new(e)))?;
        Ok(())
    }
}
