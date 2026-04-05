//! Pipeline example: actor + bridge working together.
//!
//! 1. Raw text messages are appended to an event log
//! 2. An actor on the event log analyzes each message, emitting Analyzed events
//!    back to the same log
//! 3. A bridge consumes from the event log, picks out Analyzed events, and
//!    forwards them as Summary records to a separate output log
//! 4. A consumer reads summaries from the output log

use serde::{Deserialize, Serialize};
use tracing::info;

use appendlog_actor::{Actor, ActorHandler};
use appendlog_traits::{AsyncAppender, AsyncConsumer};

// -- Event log types (single log, shared by actor + bridge) --

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Event {
    Raw { text: String },
    Analyzed { word_count: usize, char_count: usize },
}

// -- Output log type (bridge destination) --

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Summary {
    word_count: usize,
    char_count: usize,
}

// -- Actor: Raw -> Analyzed on the same log --

struct AnalyzerActor;

impl Actor for AnalyzerActor {
    type Event = Event;
    type State = ();
    type Outputs = Option<Event>;

    fn handle(&self, event: Self::Event, state: Self::State) -> (Self::Outputs, Self::State) {
        match event {
            Event::Raw { ref text } => {
                let word_count = text.split_whitespace().count();
                let char_count = text.chars().count();
                info!(word_count, char_count, "analyzed message");
                (Some(Event::Analyzed { word_count, char_count }), state)
            }
            // Ignore our own output
            Event::Analyzed { .. } => (None, state),
        }
    }
}

impl<'a> TryFrom<&'a Event> for Event {
    type Error = std::convert::Infallible;

    fn try_from(event: &'a Event) -> Result<Self, Self::Error> {
        Ok(event.clone())
    }
}

// -- State store that stores nothing (stateless actor) --

struct NoopStateStore;

impl appendlog_actor::AsyncStateStore for NoopStateStore {
    type State = ();
    type Error = std::convert::Infallible;

    async fn load(&self) -> Result<Option<((), appendlog_traits::Index)>, Self::Error> {
        Ok(None)
    }

    async fn save(&self, _state: &(), _index: appendlog_traits::Index) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = async_nats::connect("localhost:4222").await?;
    let jetstream = async_nats::jetstream::new(client.clone());

    // Clean up from previous runs
    let _ = jetstream.delete_stream("pipeline-events").await;
    let _ = jetstream.delete_stream("pipeline-summaries").await;

    // Event log (actor reads and writes here)
    let event_log =
        appendlog_nats::NatsLog::<Event>::new(client.clone(), "pipeline-events", "pipeline.events")
            .await?;

    // Output log (bridge writes here)
    let summary_log = appendlog_nats::NatsLog::<Summary>::new(
        client.clone(),
        "pipeline-summaries",
        "pipeline.summaries",
    )
    .await?;

    // Publisher handle for sending raw messages
    let publisher =
        appendlog_nats::NatsLog::<Event>::new(client.clone(), "pipeline-events", "pipeline.events")
            .await?;

    // Actor consumer (processes events on the event log)
    let actor_consumer = event_log.consumer("analyzer-actor").await;

    // Bridge consumer (reads from event log, writes to summary log)
    let bridge_consumer = event_log.consumer("summary-bridge").await;

    // Summary consumer (reads from output log)
    let summary_consumer = summary_log.consumer("summary-reader").await;

    // Spawn the actor
    let actor_handle = tokio::spawn(async move {
        let handler = (ActorHandler::new(AnalyzerActor, NoopStateStore),);
        appendlog_actor::run(actor_consumer, event_log, handler).await
    });

    // Spawn the bridge: Event log -> Summary log (only forwards Analyzed events)
    let bridge_handle = tokio::spawn(async move {
        appendlog_actor::bridge_map(bridge_consumer, summary_log, |event: Event| match event {
            Event::Analyzed {
                word_count,
                char_count,
            } => Some(Summary {
                word_count,
                char_count,
            }),
            _ => None,
        })
        .await
    });

    // Spawn the publisher
    let messages = vec![
        "hello world",
        "the quick brown fox jumps over the lazy dog",
        "rust is great",
    ];
    let expected = messages.len();
    let publisher_handle = tokio::spawn(async move {
        for text in messages {
            info!(text, "publishing raw message");
            publisher
                .append(Event::Raw {
                    text: text.to_string(),
                })
                .await
                .expect("failed to append");
        }
        info!("all messages published");
    });

    // Spawn the summary reader
    let reader_handle = tokio::spawn(async move {
        let mut consumer = summary_consumer;
        for i in 0..expected {
            match consumer.next().await {
                Ok(Some(record)) => {
                    info!(
                        index = i + 1,
                        word_count = record.data.word_count,
                        char_count = record.data.char_count,
                        "received summary"
                    );
                    consumer.ack().await.expect("failed to ack");
                }
                Ok(None) => break,
                Err(e) => {
                    info!("consumer error: {e}");
                    break;
                }
            }
        }
        info!("all summaries received");
    });

    publisher_handle.await?;
    reader_handle.await?;

    actor_handle.abort();
    bridge_handle.abort();
    info!("done");

    Ok(())
}
