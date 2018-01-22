use config::CLIENT_CONFIG;
use connection::{Connection, ConnectionFlag, OutgoingStream};
use errors::{ClusterError, ClusterResult};
use futures::Future;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_rustls::ClientConfigExt;
use utils::DOMAIN;

use std::net::SocketAddr;

pub struct Master {
    event_loop: Core,
    slave: Option<Connection<OutgoingStream>>,
}

impl Master {
    pub fn connect(addr: SocketAddr) -> ClusterResult<Self> {
        let mut core = Core::new().expect("event loop creation");
        let handle = core.handle();
        let stream_async = TcpStream::connect(&addr, &handle)
            .and_then(|stream| CLIENT_CONFIG.connect_async(DOMAIN.clone(), stream))
            .map_err(ClusterError::from)
            .and_then(|stream| Connection::create_for_stream(stream, false));

        let stream = core.run(stream_async)?;
        Ok(Master {
            event_loop: core,
            slave: Some(stream),
        })
    }

    pub fn ping(&mut self) -> ClusterResult<()> {
        let conn = self.slave.take().unwrap();
        let async_conn = conn.write_flag(ConnectionFlag::MasterPing)
            .and_then(|c| c.read_magic())
            .and_then(|c| c.read_flag());

        let (conn, flag) = self.event_loop.run(async_conn)?;
        if flag != ConnectionFlag::SlavePong {
            info!("Expected pong, but got {:?}", flag);
        }

        self.slave = Some(conn);
        Ok(())
    }
}
