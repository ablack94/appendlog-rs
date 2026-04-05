use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use appendlog_actor::Actor;
use appendlog_traits::{AsyncAppender, AsyncConsumer};

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

    fn handle(
        &self,
        input: Self::Input,
        mut state: Self::State,
    ) -> (Vec<Self::Output>, Self::State) {
        let response = match input {
            Command::Set { key, value } => {
                info!(key, value, "SET");
                state.insert(key, value);
                Response::Ok
            }
            Command::Get { key } => {
                let value = state.get(&key).cloned();
                info!(key, ?value, "GET");
                Response::Value(value)
            }
            Command::Delete { key } => {
                let existed = state.remove(&key).is_some();
                info!(key, existed, "DELETE");
                Response::Deleted(existed)
            }
        };
        (vec![response], state)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = async_nats::connect("localhost:4222").await?;
    let jetstream = async_nats::jetstream::new(client.clone());

    // Clean up state from previous runs
    let _ = jetstream.delete_stream("kv-commands").await;
    let _ = jetstream.delete_stream("kv-responses").await;
    let _ = jetstream.delete_key_value("kv-actor-state").await;

    // Create input and output logs for the actor
    let input_log =
        appendlog_nats::NatsLog::<Command>::new(client.clone(), "kv-commands", "kv.commands")
            .await?;
    let output_log =
        appendlog_nats::NatsLog::<Response>::new(client.clone(), "kv-responses", "kv.responses")
            .await?;

    // Separate log handle for the publisher to send commands
    let command_publisher =
        appendlog_nats::NatsLog::<Command>::new(client.clone(), "kv-commands", "kv.commands")
            .await?;

    // Consumer for reading responses
    let response_consumer = output_log.consumer("kv-response-reader").await;

    // Consumer and state store for the actor
    let actor_consumer = input_log.consumer("kv-actor").await;
    jetstream
        .create_key_value(async_nats::jetstream::kv::Config {
            bucket: "kv-actor-state".to_string(),
            ..Default::default()
        })
        .await?;
    let state_store =
        appendlog_nats::NatsStateStore::new(jetstream, "kv-actor-state", "state").await;

    // Spawn the actor
    let actor_handle = tokio::spawn(async move {
        appendlog_actor::run(KvActor, actor_consumer, output_log, state_store).await
    });

    // Spawn the publisher
    let publisher_handle = tokio::spawn(async move {
        let commands = vec![
            Command::Set {
                key: "name".into(),
                value: "Alice".into(),
            },
            Command::Set {
                key: "age".into(),
                value: "30".into(),
            },
            Command::Get {
                key: "name".into(),
            },
            Command::Delete {
                key: "age".into(),
            },
            Command::Get {
                key: "age".into(),
            },
        ];

        for cmd in &commands {
            info!(?cmd, "sending command");
            command_publisher.append(cmd.clone()).await;
        }
        info!(count = commands.len(), "all commands sent");
    });

    // Spawn the response reader
    let reader_handle = tokio::spawn(async move {
        let mut consumer = response_consumer;
        let expected = 5;
        for i in 0..expected {
            if let Some(record) = consumer.next().await {
                info!(index = i + 1, response = ?record.data, "received response");
                consumer.ack().await;
            }
        }
        info!("all responses received");
    });

    // Wait for publisher and reader to finish
    publisher_handle.await?;
    reader_handle.await?;

    // Actor runs indefinitely; abort it once we're done
    actor_handle.abort();
    info!("done");

    Ok(())
}
