# appendlog

Append-only indexed log abstraction for Rust.

Provides a simple trait-based interface for append-only logs where each entry gets a sequential `Index`. Includes both synchronous and async trait variants, with pluggable backends.

## Crates

| Crate | Description |
|-------|-------------|
| `appendlog-traits` | Core `Appender`/`Lookup` traits (sync) and `AsyncAppender`/`AsyncLookup` (async) |
| `appendlog-mem` | In-memory backend using `parking_lot` — good for testing and prototyping |
| `appendlog-nats` | [NATS JetStream](https://docs.nats.io/nats-concepts/jetstream) backend — persistent, distributed |

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

let idx = log.append("hello".to_string()).await;
let item = log.get(idx).await; // Some("hello")
```

Requires a NATS server with JetStream enabled (`nats-server -js`).

## Traits

```rust
// Synchronous
trait Appender { fn append(&self, item: Self::Item) -> Index; }
trait Lookup   { fn get(&self, index: Index) -> Option<Self::Item>; }

// Async
trait AsyncAppender { async fn append(&self, item: Self::Item) -> Index; }
trait AsyncLookup   { async fn get(&self, index: Index) -> Option<Self::Item>; }
```

## License

MIT
