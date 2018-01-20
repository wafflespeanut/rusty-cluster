use futures::{Future, Stream};
use rustls::ClientConfig;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_rustls::ClientConfigExt;
use webpki::DNSNameRef;

use std::io::{Read, Write};
use std::sync::Arc;

pub struct Master;

impl Master {
    pub fn test_connection() {
        let mut config = ClientConfig::new();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let client_config = Arc::new(config);

        let addr = "0.0.0.0:2753".parse().unwrap();
        let domain = DNSNameRef::try_from_ascii_str("tls.snoop").unwrap();
        let stream_async = TcpStream::connect(&addr, &handle);

        let connection = stream_async
            .and_then(|stream| client_config.connect_async(domain, stream))
            .and_then(|mut stream| {
                stream.write_all("Hello, world".as_bytes()).unwrap();

                {
                    let mut vec = Vec::new();
                    stream.read_to_end(&mut vec).unwrap();
                    ::std::io::stdout().write_all(&vec).unwrap();
                }

                Ok(())
            });

        core.run(connection).unwrap();
    }
}
