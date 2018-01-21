use std::io;

// pub type ClusterResult<T> = Result<T, ClusterError>;

#[derive(Debug, Error)]
pub enum ClusterError {
    Io(io::Error),
}
