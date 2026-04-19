# appendlog-actor

Actor framework for stateful event processing on append-only logs. An actor consumes events from one log, maintains persistent state, and appends output events to a log (which can be the same one).

## Core concepts

### Actor trait

Pure synchronous handler — no I/O, no async:

```rust
impl Actor for MyActor {
    type Event = MyEvent;
    type State = MyState;   // must impl Default
    type Outputs = Option<MyEvent>;  // anything IntoIterator works

    fn handle(&self, event: MyEvent, state: MyState) -> (Self::Outputs, MyState) {
        // return (outputs, new_state)
    }
}
```

`State` must implement `Default` — that's the initial state on first run.

`Outputs` can be `Option<E>` (zero or one), `Vec<E>` (zero or many), or `()` (never emits).

### run loop

Wires together actor + consumer + appender + state store:

```rust
let consumer = log.consumer("my-actor").await;
tokio::spawn(appendlog_actor::run(MyActor, consumer, log.clone(), state_store));
```

On each event: call `handle` → append outputs → save state with the current index → ack. State is saved before acking, so a crash before ack replays the event — the saved index is used to skip already-processed events on restart (at-least-once delivery).

The actor writes outputs to the appender, which can be the same log it reads from (see kv_store example) or a different one (see pipeline example).

### State store

Persists `(State, Index)` between runs. `Index` is the last successfully processed event; the run loop uses it to skip events on restart.

For stateless actors (`State = ()`), pass `()` as the state store — `AsyncStateStore` is implemented for `()`.

### bridge / bridge_map

Stateless forwarding without an actor. Use when you don't need state — just forwarding or filtering between logs:

```rust
// Forward all events unchanged
appendlog_actor::bridge(consumer, appender).await;

// Filter/transform: return None to drop, Some(out) to forward
appendlog_actor::bridge_map(consumer, appender, |event: Event| match event {
    Event::Analyzed { word_count, char_count } => Some(Summary { word_count, char_count }),
    _ => None,
}).await;
```

## Patterns

### Same log for input and output (command/response)

Actor reads from the log and also writes responses back to it. All readers see both commands and responses; they filter by variant:

```rust
// Actor emits response events back to the same log
fn handle(&self, event: KvEvent, mut state: ...) -> (Option<KvEvent>, ...) {
    match event {
        KvEvent::Set { cid, key, value } => { ...; (Some(KvEvent::Ok { cid }), state) }
        KvEvent::Ok { .. } => (None, state),  // ignore own responses
    }
}
```

### Chained pipeline (actor → bridge)

Actor enriches events on log A; bridge filters and forwards to log B:

```
Raw events → event_log → AnalyzerActor (appends Analyzed back to event_log)
                       → bridge_map (filters Analyzed → Summary) → summary_log
```

Two consumers on the same log, different consumer names, process independently.

### Spawning

`run(...)` and `bridge_map(...)` return futures — pass them directly to `tokio::spawn`, no `async {}` wrapper needed. Await any async setup (like `consumer(...)`) before the spawn:

```rust
let actor_consumer = log.consumer("my-actor").await;
let actor_handle = tokio::spawn(appendlog_actor::run(MyActor, actor_consumer, log.clone(), store));
```

Actors run indefinitely. Abort them after the work you're waiting on is done:

```rust
publisher_handle.await?;
reader_handle.await?;
actor_handle.abort();
```
