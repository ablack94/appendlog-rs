use appendlog_traits::{AsyncAppender, AsyncConsumer, AsyncLookup};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Event {
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = async_nats::connect("localhost:4222").await?;
    let log = appendlog_nats::NatsLog::<Event>::new(client, "events", "events.>").await?;

    // Append some events
    let idx1 = log
        .append(Event {
            message: "hello".into(),
        })
        .await?;
    let idx2 = log
        .append(Event {
            message: "world".into(),
        })
        .await?;
    println!("appended at {idx1:?} and {idx2:?}");

    // Lookup by index
    let record1 = log.get(idx1).await;
    let record2 = log.get(idx2).await;
    println!("got {record1:?} and {record2:?}");

    // Consume via pull consumer
    let mut consumer = log.consumer("my-consumer").await;
    let first = consumer.next().await?;
    let second = consumer.next().await?;
    println!("consumed {first:?} and {second:?}");

    Ok(())
}
