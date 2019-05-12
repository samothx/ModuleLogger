use log::{info, warn};
use std::fs::{File};
use std::io::{BufWriter, Write};

use ::mod_logger::{Logger,Level, LogDestination, NO_STREAM};

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
        &LogDestination::STREAM,
        Some(BufWriter::new(File::create("log.txt").unwrap()))).unwrap();

    info!("Logger initialized4");
    warn!("Logger initialized5");

    Logger::flush();


    Logger::set_log_dest(&LogDestination::BUFFER,NO_STREAM).unwrap();

    info!("Logger initialized6");
    warn!("Logger initialized7");


    if let Some(buffer) = Logger::get_buffer() {
        File::create("log_buf.txt").unwrap().write(buffer.as_ref());
    }


}
