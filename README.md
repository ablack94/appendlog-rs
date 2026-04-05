# appendlog

Append-only indexed log abstraction for Rust.

Provides a simple trait-based interface for append-only logs where each entry gets a sequential `Index`. Includes both synchronous and async trait variants, with pluggable backends.

## Crates

| Crate | Description |
|-------|-------------|
| `appendlog-traits` | Core `Appender`/`Lookup` traits (sync) and `AsyncAppender`/`AsyncLookup`/`AsyncConsumer` (async) |
| `appendlog-mem` | In-memory backend using `parking_lot` — good for testing and prototyping |
| `appendlog-nats` | [NATS JetStream](https://docs.nats.io/nats-concepts/jetstream) backend — persistent, distributed |
| `appendlog-actor` | Actor framework that consumes from one log, processes with persistent state, and writes to another |

## Quick start

```rust
use appendlog_mem::Log;
use appendlog_traits::{Appender, Lookup};

let log = Log::new();
let idx = log.append("hello");
assert_eq!(log.get(idx), Some("hello"));
```

### Async (NATS JetStream)

```rust
use appendlog_nats::NatsLog;
use appendlog_traits::{AsyncAppender, AsyncLookup};

let client = async_nats::connect("localhost:4222").await?;
let log = NatsLog::<String>::new(client, "my-stream", "my.subject").await?;

let idx = log.append("hello".to_string()).await?;
let item = log.get(idx).await; // Some("hello")
```

Requires a NATS server with JetStream enabled (`nats-server -js`).

### Actor

The `appendlog-actor` crate provides a framework for building stateful stream processors. An actor consumes messages from an input log, processes them with persistent state, and appends results to an output log.

```rust
use appendlog_actor::{Actor, AsyncStateStore};

struct MyActor;

impl Actor for MyActor {
    type Input = String;
    type Output = String;
    type State = Vec<String>;

    fn handle(&self, input: Self::Input, mut state: Self::State) -> (Vec<Self::Output>, Self::State) {
        let output = format!("processed: {input}");
        state.push(input);
        (vec![output], state)
    }
}
```

Wire it up with any `AsyncConsumer` + `AsyncAppender` + `AsyncStateStore`:

```rust
let final_state = appendlog_actor::run(MyActor, consumer, output_log, state_store).await?;
```

The run loop handles message consumption, output appending, state persistence, and acknowledgment automatically. State is saved before each ack to prevent message loss on crash (at-least-once delivery).

See [`appendlog-actor/examples/kv_store.rs`](appendlog-actor/examples/kv_store.rs) for a full example using NATS.

## Traits

```rust
// Synchronous
trait Appender {
    fn append(&self, item: Self::Item) -> Index;
}
trait Lookup {
    fn get(&self, index: Index) -> Option<Record<Self::Item>>;
}

// Async (all I/O traits return Result with an associated Error type)
trait AsyncAppender  {
    async fn append(&self, item: Self::Item) -> Result<Index, Self::Error>;
}
trait AsyncLookup {
    async fn get(&self, index: Index) -> Option<Record<Self::Item>>;
}
trait AsyncConsumer {
    async fn next(&mut self) -> Result<Option<Record<Self::Item>>, Self::Error>;
    async fn ack(&mut self) -> Result<(), Self::Error>;
}

// Actor
trait Actor {
    fn handle(&self, input: Self::Input, state: Self::State) -> (Vec<Self::Output>, Self::State);
}
trait AsyncStateStore {
    async fn load(&self) -> Result<Option<Self::State>, Self::Error>;
    async fn save(&self, state: &Self::State) -> Result<(), Self::Error>;
}
```

## License

MIT
