# appendlog

Append-only indexed log abstraction for Rust.

Provides a simple trait-based interface for append-only logs where each entry gets a sequential `Index`. Includes both synchronous and async trait variants, with pluggable backends.

## Crates

| Crate | Description |
|-------|-------------|
| `appendlog-traits` | Core `Appender`/`Lookup` traits (sync) and `AsyncAppender`/`AsyncLookup`/`AsyncConsumer` (async) |
| `appendlog-mem` | In-memory backend using `parking_lot` — good for testing and prototyping |
| `appendlog-nats` | [NATS JetStream](https://docs.nats.io/nats-concepts/jetstream) backend — persistent, distributed |
| `appendlog-actor` | Actor framework: lockstep multi-actor dispatch on a single log, plus bridges between logs |

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

The `appendlog-actor` crate provides two primitives:

- **Actor** — reads from and writes back to the **same** log. Pure state machine.
- **Bridge** — consumes from one log, appends to another. Stateless plumbing.

Multiple actors can be composed into a single lockstep dispatcher via tuple handlers:

```rust
use appendlog_actor::{Actor, ActorHandler};

struct MyActor;

impl Actor for MyActor {
    type Event = String;
    type State = Vec<String>;
    type Outputs = Vec<String>;

    fn handle(&self, event: Self::Event, mut state: Self::State) -> (Self::Outputs, Self::State) {
        let output = format!("processed: {event}");
        state.push(event);
        (vec![output], state)
    }
}
```

Wire it up with `ActorHandler` and the lockstep `run` loop:

```rust
let handler = (ActorHandler::new(MyActor, state_store),);
appendlog_actor::run(consumer, appender, handler).await?;
```

The run loop consumes from the log, dispatches to all handlers, appends outputs back to the same log, saves state, then acks. State is saved before each ack for at-least-once delivery.

For connecting separate logs, use `bridge` or `bridge_map`:

```rust
appendlog_actor::bridge(source_consumer, dest_appender).await?;
```

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
    fn handle(&self, event: Self::Event, state: Self::State) -> (Self::Outputs, Self::State);
}
trait Handler<E> {
    async fn init(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn handle(&mut self, event: &E) -> Vec<E>;
    async fn save_state(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
}
trait AsyncStateStore {
    async fn load(&self) -> Result<Option<Self::State>, Self::Error>;
    async fn save(&self, state: &Self::State) -> Result<(), Self::Error>;
}
```

## License

MIT
