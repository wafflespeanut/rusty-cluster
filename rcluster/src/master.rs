use config::CLIENT_CONFIG;
use connection::{Connection, ConnectionFlag, OutgoingStream};
use errors::{ClusterError, ClusterResult};
use futures::Future;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_rustls::ClientConfigExt;
use utils::DOMAIN;

use std::net::SocketAddr;

/// Master (i.e., client) which connects to slave machines. As long as this struct exists,
/// the sockets added will be kept alive, and so we can re-use it for further messages.
pub struct Master {
    event_loop: Core,
    slaves: Vec<Option<Connection<OutgoingStream>>>,
    addrs: Vec<SocketAddr>,
}

impl Master {
    /// Create a new instance of master.
    pub fn new() -> Self {
        Master {
            event_loop: Core::new().expect("event loop creation"),
            slaves: vec![],
            addrs: vec![],
        }
    }

    /// List of addresses to which we've successfully connected.
    pub fn addrs(&self) -> &[SocketAddr] {
        &self.addrs
    }

    /// Connect to an address and push the socket to the list of slave sockets.
    /// Once the connection has been established, this returns an ID for the connection,
    /// which should be used for future actions.
    pub fn add_slave(&mut self, addr: SocketAddr) -> ClusterResult<usize> {
        let handle = self.event_loop.handle();
        let stream_async = TcpStream::connect(&addr, &handle)
            .and_then(|stream| CLIENT_CONFIG.connect_async(DOMAIN.clone(), stream))
            .map_err(ClusterError::from)
            .and_then(|stream| Connection::create_for_stream(stream, false));

        let stream = self.event_loop.run(stream_async)?;
        self.slaves.push(Some(stream));
        self.addrs.push(addr);
        Ok(self.slaves.len() - 1)
    }

    /// Ping the connection belonging to a given ID (if it exists).
    pub fn ping(&mut self, conn_id: usize) -> ClusterResult<()> {
        if conn_id > self.slaves.len() {
            return Err(ClusterError::InvalidConnectionId)
        }

        let conn = self.slaves[conn_id].take().unwrap();
        let async_conn = conn.write_flag(ConnectionFlag::MasterPing)
            .and_then(|c| c.read_magic())
            .and_then(|c| c.read_flag());

        let (conn, flag) = self.event_loop.run(async_conn)?;
        if flag != ConnectionFlag::SlavePong {
            info!("Expected pong, but got {:?}", flag);
        }

        self.slaves[conn_id] = Some(conn);
        Ok(())
    }
}
