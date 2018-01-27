extern crate chrono;
#[macro_use] extern crate derive_error;
#[macro_use] extern crate enum_primitive;
extern crate env_logger;
extern crate futures;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate num;
extern crate rand;
extern crate rustls;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_rustls;
extern crate webpki;

include!(concat!(env!("OUT_DIR"), "/config.rs"));

mod buffered;
mod connection;
pub mod errors;
mod master;
mod slave;
pub mod utils;

pub use master::Master;
pub use slave::Slave;
