use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use appendlog_mem::{Log, LogConsumer};
use appendlog_traits::Appender;

#[derive(Debug, Clone)]
enum Event {
    Tick(u64),
}

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Begin");

    let is_running = Arc::new(AtomicBool::new(true));
    let log = Log::<Event>::new();
    let consumer = LogConsumer::new(&log);

    let pub_thread = {
        let is_running = Arc::clone(&is_running);
        std::thread::spawn(move || {
            tracing::info!("publisher started");
            let mut counter = 1;
            while is_running.load(Ordering::SeqCst) {
                tracing::info!("tick {}", counter);
                log.append(Event::Tick(counter));
                counter += 1;
                std::thread::sleep(Duration::from_secs(1));
            }
            tracing::info!("publisher stopped");
            // log is dropped here, signaling consumers to stop
        })
    };

    let consumer_thread = std::thread::spawn(move || {
        tracing::info!("consumer started");
        for item in consumer {
            tracing::info!("got {:?}", item);
        }
        tracing::info!("consumer stopped");
    });

    tracing::info!("Letting simulation run for a bit...");
    std::thread::sleep(Duration::from_secs(15));
    tracing::info!("Stopping...");
    is_running.store(false, Ordering::SeqCst);

    pub_thread.join().unwrap();
    consumer_thread.join().unwrap();
}
