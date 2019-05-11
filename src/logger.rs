use log::{Record, Metadata};

pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("[{}] {} - {}", record.target(), record.level(), record.args());
        }
    }

    fn flush(&self) {}
}