use connection::{Connection, ConnectionFlag};
use futures::Future;
use tokio_io::{AsyncRead, AsyncWrite};
use walkdir::WalkDir;

pub struct PathSync<S: AsyncRead + AsyncWrite> {
    source: String,
    dest: String,
    connection: Connection<S>,
}

impl<S> PathSync<S>
    where S: AsyncRead + AsyncWrite + 'static
{
    pub fn new<P>(source: P, dest: P, connection: Connection<S>) -> Self
        where P: AsRef<str>
    {
        PathSync {
            source: String::from(source.as_ref()),
            dest: String::from(dest.as_ref()),
            connection,
        }
    }

    pub fn from_source_to_stream(self) -> ClusterFuture<Connection<S>> {
        let (source, dest, conn) = (self.source, self.dest, self.connection);
        let walker = WalkDir::new(&source);

        conn.write_flag(ConnectionFlag::MasterSendsPath)
            .and_then(move |c| c.write_bytes(dest.into_bytes()))
            .and_then(|c| c.write_bytes(&[b'\n']))
            .and_then(move |c| {
                for entry in walker {
                    //
                }
            })
    }

    pub fn from_stream_to_dest(self) -> ClusterFuture<Connection<S>> {
        let (source, dest, conn) = (self.source, self.dest, self.connection);
    }
}
