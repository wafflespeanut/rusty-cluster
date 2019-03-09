use chrono::offset::Utc;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;
use webpki::DNSNameRef;

use std::env;

/// Default address for the listener.
pub const DEFAULT_ADDRESS: &'static str = "0.0.0.0:2753";

lazy_static! {
    /// Domain name used for verifying the connection. This should match
    /// the DNS in the openssl configuration.
    pub static ref DOMAIN: DNSNameRef<'static> =
        DNSNameRef::try_from_ascii_str("snoop.fetch").unwrap();
}

/// Prepare the logger with the universal datetime format and INFO level.
pub fn prepare_logger() {
    let mut builder = Builder::new();
    builder.format(|buf, record| write!(buf, "{:?}: {}: {}", Utc::now(), record.level(), record.args()))
           .filter_level(LevelFilter::Off);
    if let Ok(v) = env::var("LOG_LEVEL") {
       builder.parse(&v);
    }

    builder.init();
}
