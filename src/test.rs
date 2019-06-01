use log::{info, warn};
use std::fs::File;
use std::io::{BufWriter, Write};

use ::mod_logger::{Level, LogDestination, Logger, NO_STREAM};

mod test_mod {
    use log::warn;
    pub fn test_func() {
        warn!("test_func: This is a test_func");
    }
}

fn main() {
    if let Err(_why) = Logger::initialise(Some("info")) {
        println!("Logger failed to initialize");
        std::process::exit(1);
    }

    info!("Logger initialized1");

    Logger::set_default_level(&Level::Warn);

    info!("Logger initialized2");
    warn!("Logger initialized3");

    Logger::set_log_dest(
        &LogDestination::Stream,
        Some(BufWriter::new(File::create("log.txt").unwrap())),
    )
    .unwrap();

    info!("Logger initialized4");
    warn!("Logger initialized5");

    Logger::flush();

    Logger::set_log_dest(&LogDestination::Buffer, NO_STREAM).unwrap();

    info!("Logger initialized6");
    warn!("Logger initialized7");

    test_mod::test_func();

    if let Some(buffer) = Logger::get_buffer() {
        File::create("log_buf.txt")
            .unwrap()
            .write(buffer.as_ref())
            .unwrap();
    }
}
