use errors::ClusterResult;
use futures::{Future, Stream};
use rustls::{NoClientAuth, ServerConfig, ServerSession};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;
use tokio_rustls::{ServerConfigExt, TlsStream};
use utils::DEFAULT_PORT;

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;

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
    fn handle_stream(stream: &mut TlsStream<TcpStream, ServerSession>) -> ClusterResult<()> {
        {
            let mut vec = Vec::new();
            stream.read_to_end(&mut vec)?;
            ::std::io::stdout().write_all(&vec)?;
        }

        {
            stream.write_all("Booya!".as_bytes())?;
        }

        Ok(())
    }

    pub fn start_listening(self) {
        let mut config = ServerConfig::new(NoClientAuth::new());
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let server_config = Arc::new(config);

        let listener = TcpListener::bind(&self.address, &handle).unwrap();
        let listen = listener.incoming().for_each(|(stream, addr)| {
            info!("Incoming stream from {:?}", addr);
            let job = server_config.accept_async(stream).and_then(|mut stream| {
                if let Err(e) = Self::handle_stream(&mut stream) {
                    error!("{:?}", e);
                    let _ = stream.write_all(e.to_string().as_bytes());
                }

                Ok(())
            }).map_err(|e| {
                error!("Stream: {:?}", e);
            });

            handle.spawn(job);
            Ok(())
        });

        core.run(listen).unwrap();
    }
}
