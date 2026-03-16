use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use appendlog_actor::Actor;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Command {
    Set { key: String, value: String },
    Get { key: String },
    Delete { key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Response {
    Ok,
    Value(Option<String>),
    Deleted(bool),
}

struct KvActor;

impl Actor for KvActor {
    type Input = Command;
    type Output = Response;
    type State = HashMap<String, String>;

    fn handle(&self, input: Self::Input, mut state: Self::State) -> (Vec<Self::Output>, Self::State) {
        let response = match input {
            Command::Set { key, value } => {
                state.insert(key, value);
                Response::Ok
            }
            Command::Get { key } => {
                let value = state.get(&key).cloned();
                Response::Value(value)
            }
            Command::Delete { key } => {
                let existed = state.remove(&key).is_some();
                Response::Deleted(existed)
            }
        };
        (vec![response], state)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = async_nats::connect("localhost:4222").await?;
    let jetstream = async_nats::jetstream::new(client.clone());

    // Create input and output logs
    let input_log =
        appendlog_nats::NatsLog::<Command>::new(client.clone(), "kv-commands", "kv.commands")
            .await?;
    let output_log =
        appendlog_nats::NatsLog::<Response>::new(client, "kv-responses", "kv.responses").await?;

    // Create a consumer for the input log
    let consumer = input_log.consumer("kv-actor").await;

    // Create state store
    jetstream
        .create_key_value(async_nats::jetstream::kv::Config {
            bucket: "kv-actor-state".to_string(),
            ..Default::default()
        })
        .await?;
    let state_store =
        appendlog_nats::NatsStateStore::new(jetstream, "kv-actor-state", "state").await;

    // Run the actor
    let final_state = appendlog_actor::run(KvActor, consumer, output_log, state_store).await;
    println!("Actor finished with state: {final_state:?}");

    Ok(())
}
