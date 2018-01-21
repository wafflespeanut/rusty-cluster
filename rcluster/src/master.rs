use config::CLIENT_CONFIG;
use futures::Future;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::{io as async_io, AsyncRead};
use tokio_rustls::ClientConfigExt;
use webpki::DNSNameRef;

use std::io::{self, Write};

pub struct Master;

impl Master {
    pub fn test_connection() {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let addr = "0.0.0.0:2753".parse().unwrap();
        let domain = DNSNameRef::try_from_ascii_str("snoop.fetch").unwrap();
        let stream_async = TcpStream::connect(&addr, &handle);

        let connection = stream_async
            .and_then(|stream| CLIENT_CONFIG.connect_async(domain, stream))
            .and_then(|stream| async_io::write_all(stream, &b"ping\n"[..]))
            .and_then(|(stream, _)| {
                let (reader, _) = stream.split();
                async_io::read_to_end(reader, Vec::new())
                         .and_then(|(_, output)| io::stdout().write_all(&output))
            });

        core.run(connection).unwrap();
    }
}
