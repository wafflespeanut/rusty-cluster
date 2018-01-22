use futures::Future;

use std::io;
use std::net::AddrParseError;

pub type ClusterFuture<T> = Box<Future<Item=T, Error=ClusterError>>;
pub type ClusterResult<T> = Result<T, ClusterError>;

#[derive(Debug, Error)]
pub enum ClusterError {
    Io(io::Error),
    AddrParse(AddrParseError),
    /// Unknown flag in stream.
    UnknownFlag,
}
