# appendlog

Append-only indexed log abstraction for Rust. Workspace with four crates.

## Crate structure

- `appendlog-traits` — core traits (`Appender`, `Lookup`, `AsyncAppender`, `AsyncLookup`, `AsyncConsumer`) and shared types (`Index`, `Record`)
- `appendlog-mem` — in-memory backend using `parking_lot`
- `appendlog-nats` — NATS JetStream backend (streams, consumers, state store). Has an `actor` feature flag that brings in `appendlog-actor` types
- `appendlog-actor` — actor framework: consumes from one log, processes with persistent state, writes to another. Depends only on `appendlog-traits`

## Building and testing

```sh
cargo build --workspace
cargo test --workspace
cargo build --examples
```

Examples require a running NATS server with JetStream enabled (`nats-server -js`):

```sh
cargo run --example simple -p appendlog-mem
cargo run --example kv_store -p appendlog-actor
```

## Key conventions

- Traits live in `appendlog-traits`; backends implement them
- `Record<T>` wraps data in `Arc` for cheap cloning
- `Index` is a newtype over `u64`
- Async consumers use deferred acking: call `next()` then `ack()` after processing
- The actor `run` loop saves state before acking to guarantee at-least-once delivery

## Important

- **Keep `README.md` up to date** when adding or changing crates, traits, or public API
