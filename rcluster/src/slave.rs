use {connection};
use config::SERVER_CONFIG;
use errors::{ClusterError, ClusterResult};
use futures::{Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_rustls::ServerConfigExt;

use std::net::SocketAddr;

pub struct Slave {
    address: SocketAddr,
}

impl Slave {
    pub fn new(addr: SocketAddr) -> Self {
        Slave {
            address: addr,
        }
    }
}

impl Slave {
    pub fn start_listening(self) -> ClusterResult<()> {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let listener = TcpListener::bind(&self.address, &handle).unwrap();
        let listen = listener.incoming().for_each(|(stream, addr)| {
            info!("Incoming stream from {:?}", addr);
            handle.spawn({
                SERVER_CONFIG.accept_async(stream)
                    .map_err(ClusterError::from)
                    .and_then(connection::handle_incoming)
                    .map_err(move |e| error!("Error in stream from {}: {:?}", addr, e))
            });

            Ok(())
        });

        core.run(listen)?;
        Ok(())
    }
}
