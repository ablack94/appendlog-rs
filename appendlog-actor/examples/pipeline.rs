//! Pipeline example with OpenTelemetry tracing.
//!
//! Requires:
//!   - NATS server with JetStream: `nats-server -js`
//!   - Jaeger: `docker run -p 16686:16686 -p 4317:4317 jaegertracing/all-in-one`
//!
//! Run: `cargo run --example pipeline -p appendlog-actor --features otel`
//! Then open http://localhost:16686 to see traces.
//!
//! Flow:
//! 1. Raw text messages are appended to an event log
//! 2. An actor analyzes each message, emitting Analyzed events back to the same log
//! 3. A bridge picks out Analyzed events and forwards them as Summaries to an output log
//! 4. A consumer reads summaries from the output log

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use serde::{Deserialize, Serialize};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use appendlog_actor::Actor;
use appendlog_traits::{AsyncAppender, AsyncConsumer};

// -- Event log types (single log, shared by actor + bridge) --

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Event {
    Raw {
        text: String,
    },
    Analyzed {
        word_count: usize,
        char_count: usize,
    },
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
                (
                    Some(Event::Analyzed {
                        word_count,
                        char_count,
                    }),
                    state,
                )
            }
            Event::Analyzed { .. } => (None, state),
        }
    }
}


fn init_tracing() -> opentelemetry_sdk::trace::SdkTracerProvider {
    // Enable W3C Trace Context propagation through NATS headers
    global::set_text_map_propagator(opentelemetry_sdk::propagation::TraceContextPropagator::new());

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("failed to create OTLP exporter");

    let resource = opentelemetry_sdk::Resource::builder()
        .with_service_name("pipeline-example")
        .build();

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    let tracer = provider.tracer("appendlog-pipeline");

    let stdout_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(stdout_filter))
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    provider
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = init_tracing();

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
        appendlog_actor::run(AnalyzerActor, actor_consumer, event_log, ()).await
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

    // Publish some messages
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

    // Read summaries from the output log
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

    // Flush traces before exit
    provider.shutdown()?;
    info!("done");

    Ok(())
}
