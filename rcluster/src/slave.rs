use config::SERVER_CONFIG;
use futures::{Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::{io as async_io, AsyncRead};
use tokio_rustls::ServerConfigExt;
use utils::DEFAULT_PORT;

use std::net::SocketAddr;

pub struct Slave {
    pub address: SocketAddr,
}

impl Default for Slave {
    fn default() -> Self {
        Slave {
            address: SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)),
        }
    }
}

impl Slave {
    pub fn start_listening(self) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let listener = TcpListener::bind(&self.address, &handle).unwrap();
        let listen = listener.incoming().for_each(|(stream, addr)| {
            info!("Incoming stream from {:?}", addr);
            let job = SERVER_CONFIG.accept_async(stream)
                .and_then(|stream| {
                    let (reader, writer) = stream.split();
                    async_io::copy(reader, writer)
                })
                .and_then(|(_, _reader, writer)| async_io::flush(writer))
                .map(move |_| info!("Accepted connection from {:?}", addr))
                .map_err(move |e| error!("Error in stream from {}: {:?}", addr, e));

            handle.spawn(job);
            Ok(())
        });

        core.run(listen).unwrap();
    }
}
