use chrono::offset::Utc;
use env_logger::LogBuilder;
use log::{LogRecord, LogLevelFilter};

use std::env;

pub const DEFAULT_PORT: u16 = 2753;

/// Prepare the logger with the universal datetime format and INFO level.
pub fn prepare_logger() {
    let mut builder = LogBuilder::new();
    builder.format(|record: &LogRecord| format!("{:?}: {}: {}", Utc::now(), record.level(), record.args()))
           .filter(None, LogLevelFilter::Off);
    if let Ok(v) = env::var("LOG_LEVEL") {
       builder.parse(&v);
    }

    builder.init().expect("failed to prepare logger");
}
