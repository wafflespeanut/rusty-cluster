use connection::Connection;
use config::SERVER_CONFIG;
use errors::{ClusterError, ClusterResult};
use futures::{Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_rustls::ServerConfigExt;

use std::net::SocketAddr;

/// A slave represents a server that can be connected only by the master.
/// (Ideally, the master has the right signed cert).
pub struct Slave {
    address: SocketAddr,
}

impl Slave {
    /// Create a new slave that should be bound to the given address.
    pub fn new(addr: SocketAddr) -> Self {
        Slave {
            address: addr,
        }
    }
}

impl Slave {
    /// Start listening for incoming requests from master.
    pub fn start_listening(self) -> ClusterResult<()> {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let listener = TcpListener::bind(&self.address, &handle).unwrap();
        let listen = listener.incoming().for_each(|(stream, addr)| {
            info!("Incoming stream from {:?}", addr);
            handle.spawn({
                SERVER_CONFIG.accept_async(stream)
                    .map_err(ClusterError::from)
                    .and_then(|stream| Connection::create_for_stream(stream, true))
                    .and_then(|c| c.handle_flags())
                    .map(|_| ())
                    .map_err(move |e| error!("Error in stream from {}: {:?}", addr, e))
            });

            Ok(())
        });

        core.run(listen)?;
        Ok(())
    }
}
