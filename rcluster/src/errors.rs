use futures::Future;

use std::io;
use std::net::AddrParseError;

/// Future type used throughout the library.
pub type ClusterFuture<T> = Box<Future<Item=T, Error=ClusterError>>;
/// Result type used throughout the library.
pub type ClusterResult<T> = Result<T, ClusterError>;

#[derive(Debug, Error)]
pub enum ClusterError {
    Io(io::Error),
    AddrParse(AddrParseError),
    /// Unknown flag in stream.
    UnknownFlag,
    /// No such connection exists for ID.
    InvalidConnectionId,
}
