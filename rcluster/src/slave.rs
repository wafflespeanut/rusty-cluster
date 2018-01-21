use config::SERVER_CONFIG;
use futures::{Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::{io as async_io, AsyncRead};
use tokio_rustls::ServerConfigExt;
use utils::DEFAULT_PORT;

use std::io::{self, BufReader, Write};
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
                    let reader = BufReader::new(reader);
                    async_io::read_until(reader, b'\n', Vec::new())
                             .and_then(|(_, output)| io::stdout().write_all(&output))
                             .and_then(|_| async_io::write_all(writer, &b"pong\n"[..]))
                })
                .map(move |_| debug!("Finished serving {:?}", addr))
                .map_err(move |e| error!("Error in stream from {}: {:?}", addr, e));

            handle.spawn(job);
            Ok(())
        });

        core.run(listen).unwrap();
    }
}
