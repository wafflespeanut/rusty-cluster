use errors::{ClusterError, ClusterFuture};
use futures::{Future, future};
use num::FromPrimitive;
use rand::{self, Rng};
use rustls::{ClientSession, ServerSession};
use tokio_core::net::TcpStream;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::io::{self as async_io, ReadHalf, WriteHalf};
use tokio_rustls::TlsStream;

use std::io::{BufReader, BufWriter};

pub const MAGIC_LENGTH: usize = 16;

enum_from_primitive! {
    #[repr(u8)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ConnectionFlag {
        MasterPing,
        SlavePong,
        MasterWantsFile,
        MasterSendsFile,
        MasterWantsExecution,
    }
}

pub type OutgoingStream = TlsStream<TcpStream, ClientSession>;
type IncomingStream = TlsStream<TcpStream, ServerSession>;
type ConnectionParts<S> = (BufReader<ReadHalf<S>>, BufWriter<WriteHalf<S>>, [u8; MAGIC_LENGTH]);

pub struct Connection<S: AsyncRead + AsyncWrite> {
    reader: BufReader<ReadHalf<S>>,
    writer: BufWriter<WriteHalf<S>>,
    magic: [u8; MAGIC_LENGTH],
}

impl<S> From<ConnectionParts<S>> for Connection<S>
    where S: AsyncRead + AsyncWrite
{
    fn from(v: ConnectionParts<S>) -> Connection<S> {
        Connection {
            reader: v.0,
            writer: v.1,
            magic: v.2,
        }
    }
}

impl<S> Into<ConnectionParts<S>> for Connection<S>
    where S: AsyncRead + AsyncWrite
{
    #[inline]
    fn into(self) -> ConnectionParts<S> {
        (self.reader, self.writer, self.magic)
    }
}

impl<S> Connection<S>
    where S: AsyncRead + AsyncWrite + 'static
{
    pub fn create_for_stream(stream: S, expect_magic: bool) -> ClusterFuture<Self> {
        let (r, w) = stream.split();
        let (reader, writer) = (BufReader::new(r), BufWriter::new(w));
        let mut magic = [0; MAGIC_LENGTH];

        if expect_magic {
            Connection { reader, writer, magic }.read_magic()
        } else {
            let mut rng = rand::thread_rng();
            rng.fill_bytes(&mut magic);
            Connection { reader, writer, magic }.write_magic()
        }
    }

    #[inline]
    pub fn write_bytes<B>(self, bytes: B) -> ClusterFuture<Self>
        where B: AsRef<[u8]> + 'static
    {
        let (r, w, m) = self.into();
        let async_write = async_io::write_all(w, bytes)
            .and_then(|(w, _)| async_io::flush(w))
            .map(move |w| Connection::from((r, w, m)))
            .map_err(ClusterError::from);
        Box::new(async_write) as ClusterFuture<Self>
    }

    #[inline]
    pub fn read_magic(self) -> ClusterFuture<Self> {
        let (reader, writer, _) = self.into();
        let async_read = async_io::read_exact(reader, [0; MAGIC_LENGTH])
            .map(|(reader, magic)| Connection { reader, writer, magic })
            .map_err(ClusterError::from);
        Box::new(async_read) as ClusterFuture<Self>
    }

    #[inline]
    pub fn write_magic(self) -> ClusterFuture<Self> {
        let m = self.magic;
        self.write_bytes(m)
    }

    #[inline]
    pub fn read_flag(self) -> ClusterFuture<(Self, ConnectionFlag)> {
        let (r, w, m) = self.into();
        let async_handle = async_io::read_exact(r, [0; 1])
            .map_err(ClusterError::from)
            .and_then(move |(r, flag_byte)| {
                let flag = ConnectionFlag::from_u8(flag_byte[0])
                                           .ok_or(ClusterError::UnknownFlag);
                info!("Got flag: {:?}", flag);
                future::result(flag.map(move |f| ((r, w, m).into(), f)))
            });
        Box::new(async_handle) as ClusterFuture<(Self, ConnectionFlag)>
    }

    #[inline]
    pub fn write_flag(self, flag: ConnectionFlag) -> ClusterFuture<Self> {
        let flag: [u8; 1] = [flag as u8];
        self.write_bytes(flag)
    }

    #[inline]
    fn handle_flags(self) -> ClusterFuture<Self> {
        let async_handle = self.read_flag().and_then(|(conn, flag)| {
            conn.write_magic().and_then(move |conn| match flag {
                ConnectionFlag::MasterPing => conn.write_flag(ConnectionFlag::SlavePong),
                _ => {
                    error!("Dunno how to handle {:?}", flag);
                    Box::new(future::ok(conn)) as ClusterFuture<Self>
                }
            })
        });

        Box::new(async_handle) as ClusterFuture<Self>
    }
}

#[inline]
pub fn handle_incoming(stream: IncomingStream) -> ClusterFuture<()> {
    let async_conn = Connection::create_for_stream(stream, true)
                                .and_then(|c| c.handle_flags())
                                .map(|_| ());
    Box::new(async_conn) as ClusterFuture<()>
}
