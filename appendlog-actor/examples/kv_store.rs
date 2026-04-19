use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use appendlog_actor::Actor;
use appendlog_traits::{AsyncAppender, AsyncConsumer};

type Cid = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum KvEvent {
    // Commands
    Set {
        cid: Cid,
        key: String,
        value: String,
    },
    Get {
        cid: Cid,
        key: String,
    },
    Delete {
        cid: Cid,
        key: String,
    },
    // Responses
    Ok {
        cid: Cid,
    },
    Value {
        cid: Cid,
        value: Option<String>,
    },
    Deleted {
        cid: Cid,
        existed: bool,
    },
}

struct KvActor;

impl Actor for KvActor {
    type Event = KvEvent;
    type State = HashMap<String, String>;
    type Outputs = Option<KvEvent>;

    fn handle(&self, event: Self::Event, mut state: Self::State) -> (Self::Outputs, Self::State) {
        let response = match event {
            KvEvent::Set { cid, key, value } => {
                info!(cid, key, value, "SET");
                state.insert(key, value);
                Some(KvEvent::Ok { cid })
            }
            KvEvent::Get { cid, key } => {
                let value = state.get(&key).cloned();
                info!(cid, key, ?value, "GET");
                Some(KvEvent::Value { cid, value })
            }
            KvEvent::Delete { cid, key } => {
                let existed = state.remove(&key).is_some();
                info!(cid, key, existed, "DELETE");
                Some(KvEvent::Deleted { cid, existed })
            }
            // Ignore response events
            _ => None,
        };
        (response, state)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = async_nats::connect("localhost:4222").await?;
    let jetstream = async_nats::jetstream::new(client.clone());

    // Clean up state from previous runs
    let _ = jetstream.delete_stream("kv-events").await;
    let _ = jetstream.delete_key_value("kv-actor-state").await;

    let log =
        appendlog_nats::NatsLog::<KvEvent>::new(client.clone(), "kv-events", "kv.events").await?;
    let actor_consumer = log.consumer("kv-actor").await;
    let mut response_consumer = log.consumer("kv-response-reader").await;

    let actor_handle = tokio::spawn(appendlog_actor::run(
        KvActor,
        actor_consumer,
        log.clone(),
        appendlog_nats::NatsStateStore::new(jetstream, "kv-actor-state", "state").await,
    ));

    let publisher_handle = tokio::spawn(async move {
        let commands = vec![
            KvEvent::Set {
                cid: 1,
                key: "name".into(),
                value: "Alice".into(),
            },
            KvEvent::Set {
                cid: 2,
                key: "age".into(),
                value: "30".into(),
            },
            KvEvent::Get {
                cid: 3,
                key: "name".into(),
            },
            KvEvent::Delete {
                cid: 4,
                key: "age".into(),
            },
            KvEvent::Get {
                cid: 5,
                key: "age".into(),
            },
        ];

        for cmd in &commands {
            info!(?cmd, "sending command");
            log.append(cmd.clone()).await.expect("failed to append command");
        }
        info!(count = commands.len(), "all commands sent");
    });

    let reader_handle = tokio::spawn(async move {
        let expected_responses = 5;
        let mut response_count = 0;
        while response_count < expected_responses {
            match response_consumer.next().await {
                Ok(Some(record)) => {
                    match &*record.data {
                        KvEvent::Ok { cid }
                        | KvEvent::Value { cid, .. }
                        | KvEvent::Deleted { cid, .. } => {
                            response_count += 1;
                            info!(
                                cid,
                                response = ?record.data,
                                "received response"
                            );
                        }
                        _ => {
                            // Skip command events
                        }
                    }
                    response_consumer.ack().await.expect("failed to ack");
                }
                Ok(None) => break,
                Err(e) => {
                    info!("consumer error: {e}");
                    break;
                }
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
