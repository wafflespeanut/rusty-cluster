use chrono::offset::Utc;
use env_logger::LogBuilder;
use log::{LogRecord, LogLevelFilter};
use webpki::DNSNameRef;

use std::env;

pub const DEFAULT_ADDRESS: &'static str = "0.0.0.0:2753";

lazy_static! {
    pub static ref DOMAIN: DNSNameRef<'static> =
        DNSNameRef::try_from_ascii_str("snoop.fetch").unwrap();
}

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
