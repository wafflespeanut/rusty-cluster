use errors::{ClusterError, ClusterFuture};
use futures::{Async, Future, Poll, future};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::io::{self as async_io};

use std::cell::Cell;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Write};
use std::path::Path;

pub const BUFFER_SIZE: usize = 8 * 1024;

pub struct StreamingBuffer<R: Read, W: Write> {
    reader: Option<BufReader<R>>,
    writer: Option<BufWriter<W>>,
    stop_bytes: Box<[u8]>,
    prev_bytes_unwritten: Box<[u8]>,
    status: Cell<StreamerStatus>,
}

impl<W> StreamingBuffer<File, W>
    where W: Write + 'static
{
    #[inline]
    pub fn file_to_stream<P>(path: P, stream: BufWriter<W>)
                            -> ClusterFuture<Self>
        where P: AsRef<Path>
    {
        let reader = File::open(path).map(|f| BufReader::with_capacity(BUFFER_SIZE, f)).map(Some);
        let async_streamer = reader.map(|reader| {
            StreamingBuffer {
                reader,
                writer: Some(stream),
                stop_bytes: Box::new([]),
                prev_bytes_unwritten: Box::new([]),
                status: Cell::new(StreamerStatus::StopperNotFound),
            }
        }).map_err(ClusterError::from);

        Box::new(future::result(async_streamer)) as ClusterFuture<Self>
    }
}

impl<R> StreamingBuffer<R, File>
    where R: Read + 'static
{
    #[inline]
    pub fn stream_to_file<P>(stream: BufReader<R>, stop_bytes: &[u8], path: P)
                            -> ClusterFuture<Self>
        where P: AsRef<Path>
    {
        let writer = File::create(path).map(|f| BufWriter::with_capacity(BUFFER_SIZE, f)).map(Some);
        let async_streamer = writer.map(|writer| {
            StreamingBuffer {
                reader: Some(stream),
                writer,
                stop_bytes: stop_bytes.into(),
                prev_bytes_unwritten: Box::new([]),
                status: Cell::new(StreamerStatus::StopperNotFound),
            }
        }).map_err(ClusterError::from);

        Box::new(future::result(async_streamer)) as ClusterFuture<Self>
    }
}

#[derive(Copy, Clone, PartialEq)]
enum StreamerStatus {
    StopperPrefixFound(usize),
    StopperFound(usize),
    StopperFoundWithPrevious(usize),
    StopperPrefixWasted,
    StopperNotFound,
}

impl<R, W> StreamingBuffer<R, W>
    where R: Read, W: Write
{
    fn check_previous_bytes_with(&self, parent_bytes: &[u8]) -> Option<StreamerStatus> {
        let stopper_prefix = &self.stop_bytes;
        let stop_len = stopper_prefix.len();
        if let StreamerStatus::StopperPrefixFound(i) = self.status.get() {
            let stopper = &stopper_prefix[(stop_len - i)..];
            if parent_bytes.starts_with(&stopper) {
                return Some(StreamerStatus::StopperFoundWithPrevious(i))
            } else {
                return Some(StreamerStatus::StopperPrefixWasted)
            }
        }

        None
    }

    /// Isolate some bytes for checking against magic during next poll.
    /// This returns the number of bytes to be consumed in the reader.
    fn check_suffix_bytes<'a>(&self, parent_bytes: &'a [u8]) -> (&'a [u8], StreamerStatus) {
        let parent_len = parent_bytes.len();
        let stop_len = self.stop_bytes.len();

        let unwritten = if parent_bytes.len() >= stop_len {
            &parent_bytes[(parent_len - stop_len)..]
        } else {
            &parent_bytes
        };

        let mut stopper_prefix = &self.stop_bytes[..];
        let mut prev_suffix = &self.prev_bytes_unwritten[..];
        for i in 0..prev_suffix.len() {
            prev_suffix = &prev_suffix[i..];
            stopper_prefix = &stopper_prefix[..prev_suffix.len()];
            if prev_suffix.starts_with(stopper_prefix) {
                let remaining = stop_len - prev_suffix.len();
                if remaining == 0 {
                    return (unwritten, StreamerStatus::StopperFound(parent_len))
                } else {
                    return (unwritten, StreamerStatus::StopperPrefixFound(remaining))
                }
            }
        }

        (unwritten, StreamerStatus::StopperNotFound)
    }

    #[inline]
    fn get_unwritten_bytes_from_status(&self, status: StreamerStatus) -> Option<&[u8]> {
        match status {
            StreamerStatus::StopperFoundWithPrevious(i) => {
                let bytes = &self.prev_bytes_unwritten;
                let remaining = bytes.len() - (self.stop_bytes.len() - i);
                Some(&bytes[..remaining])
            },
            StreamerStatus::StopperPrefixWasted => Some(&self.prev_bytes_unwritten),
            _ => None,
        }
    }
}

impl<R, W> Future for StreamingBuffer<R, W>
    where R: Read, W: Write
{
    type Item = (BufReader<R>, BufWriter<W>);
    type Error = ClusterError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let (mut r, mut w) = (self.reader.take().unwrap(), self.writer.take().unwrap());

        let mut content_ended = false;
        let consumed = {
            // Step 1 - fill the internal buffer
            let bytes = r.fill_buf()?;
            // Step 2 - Get the amount of bytes in this buffer.
            // NOTE: This will be equal to `BUFFER_SIZE` (unless previously read or if the
            // stream is about to end).
            info!("Read: {}", bytes.len());

            // This ensures that the bytes from the previous block aren't lost.
            if let Some(status) = self.check_previous_bytes_with(bytes) {
                if let Some(prev_bytes) = self.get_unwritten_bytes_from_status(status) {
                    w.write_all(prev_bytes)?;
                    info!("Wrote previous: {}", prev_bytes.len());
                    // The status is changed only on successful write.
                    self.status.set(status);
                }
            }

            let (unwritten, status) = self.check_suffix_bytes(bytes);
            let consume_amount = match status {
                StreamerStatus::StopperFound(amt)
                | StreamerStatus::StopperFoundWithPrevious(amt) => {
                    content_ended = true;
                    amt
                },
                _ => bytes.len(),
            };

            w.write_all(&bytes[..consume_amount])?;
            info!("Wrote {}!", consume_amount);
            self.prev_bytes_unwritten = unwritten.into();
            self.status.set(status);

            Ok(consume_amount)
        };

        match consumed {
            Ok(len) => {
                info!("Consumed {}!", len);
                r.consume(len);
                if content_ended {
                    info!("Ready!");
                    Ok(Async::Ready((r, w)))
                } else {
                    info!("Not ready!");
                    self.reader = Some(r);
                    self.writer = Some(w);
                    Ok(Async::NotReady)
                }
            },
            Err(ClusterError::Io(ref e)) if e.kind() == ErrorKind::WouldBlock => {
                info!("Blocking!");
                self.reader = Some(r);
                self.writer = Some(w);
                Ok(Async::NotReady)
            },
            Err(e) => Err(e),
        }
    }
}
