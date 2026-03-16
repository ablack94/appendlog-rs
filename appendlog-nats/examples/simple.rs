use serde::{Deserialize, Serialize};
use appendlog_traits::{AsyncAppender, AsyncLookup};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Event {
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = async_nats::connect("localhost:4222").await?;
    let log = appendlog_nats::NatsLog::<Event>::new(client, "events", "events.>").await?;

    let idx1 = log.append(Event { message: "hello".into() }).await;
    let idx2 = log.append(Event { message: "world".into() }).await;
    println!("appended at {idx1:?} and {idx2:?}");

    let item1 = log.get(idx1).await;
    let item2 = log.get(idx2).await;
    println!("got {item1:?} and {item2:?}");

    Ok(())
}
