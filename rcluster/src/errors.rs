use futures::Future;
use walkdir::Error as WalkError;

use std::io;
use std::net::AddrParseError;

macro_rules! future_try {
    ($res:expr) => {
        match $res {
            Ok(t) => t,
            Err(e) => return Box::new(future::err(e.into())) as ClusterFuture<_>,
        }
    };
}

macro_rules! future_try_wait {
    ($res:expr) => {
        future_try!($res.wait())
    };
}

/// Future type used throughout the library.
pub type ClusterFuture<T> = Box<Future<Item=T, Error=ClusterError>>;
/// Result type used throughout the library.
pub type ClusterResult<T> = Result<T, ClusterError>;

#[derive(Debug, Error)]
pub enum ClusterError {
    Io(io::Error),
    AddrParse(AddrParseError),
    Walk(WalkError),
    /// Unknown flag in stream.
    UnknownFlag,
    /// No such connection exists for ID.
    InvalidConnectionId,
}
