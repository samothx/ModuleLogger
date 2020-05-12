use log::{info, warn};
use std::fs::File;
use std::io::{BufWriter, Write};

use ::mod_logger::{Level, LogDestination, Logger, NO_STREAM};

mod test_mod {
    use log::{debug, error, info, trace, warn};

    mod test_test {
        use log::{debug, error, info, trace, warn};

        pub fn test_func() {
            trace!("test_func: This is a test at trace level");
            debug!("test_func: This is a test at debug level");
            info!("test_func: This is a test at info level");
            warn!("test_func: This is a test  at warn level");
            error!("test_func: This is a test  at error level");
        }
    }

    pub fn test_func() {
        trace!("test_func: This is a test at trace level");
        debug!("test_func: This is a test at debug level");
        info!("test_func: This is a test at info level");
        warn!("test_func: This is a test  at warn level");
        error!("test_func: This is a test  at error level");
        test_test::test_func()
    }
}

fn main() {
    Logger::set_default_level(Level::Info);

    info!("Logger initialized1");

    Logger::set_default_level(Level::Warn);

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

    Logger::set_log_dest(&LogDestination::BufferStderr, NO_STREAM).unwrap();

    info!("Logger initialized6");
    warn!("Logger initialized7");

    Logger::set_default_level(Level::Warn);
    test_mod::test_func();

    if let Some(buffer) = Logger::get_buffer() {
        File::create("log_buf.txt")
            .unwrap()
            .write_all(buffer.as_ref())
            .unwrap();
    }


}
