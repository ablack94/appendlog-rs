use appendlog_mem::Log;
use appendlog_traits::{Appender, Lookup};

#[derive(Debug, Clone)]
enum Event {
    Tick(u64),
}

fn main() {
    let log = Log::<Event>::new();
    log.append(Event::Tick(0));
    log.append(Event::Tick(1));

    println!("0 => {:?}", log.get(0));
    println!("1 => {:?}", log.get(1));
    println!("2 => {:?}", log.get(2));
}
