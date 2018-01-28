use errors::{ClusterError, ClusterFuture};
use futures::future;

use std::cell::Cell;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Write};
use std::path::Path;

/// Buffer size used throughout the library.
pub const BUFFER_SIZE: usize = 8 * 1024;

/// A "streamer" used on read/write halves for streaming stuff (like files, command output, etc.).
/// It works much like multipart data. In the read end of the stream, this checks for magic
/// bytes - once it encounters them, it consumes those bytes and stops reading. Hence, the reader's
/// cursor will be positioned just after the magic bytes.
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
    /// Initialize this struct for reading file onto a stream.
    #[inline]
    pub fn file_to_stream<P>(path: P, stream: BufWriter<W>)
                            -> ClusterFuture<Self>
        where P: AsRef<Path>
    {
        info!("Reading from {}", path.as_ref().display());
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
    /// Initialize this struct for writing to file from a stream. Note that this requires
    /// the magic bytes after which the streaming should be stopped. If the magic bytes are
    /// empty, then the entire stream (until EOF) is written to file.
    #[inline]
    pub fn stream_to_file<P>(stream: BufReader<R>, stop_bytes: &[u8], path: P)
                            -> ClusterFuture<Self>
        where P: AsRef<Path>
    {
        info!("Writing to {}", path.as_ref().display());
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

/// Status of this streamer. This determines whether the streaming should continue/stop.
#[derive(Copy, Clone, Debug, PartialEq)]
enum StreamerStatus {
    /// Some of the bytes in the end of the current chunk match with the stopper's prefix.
    /// This variant carries "N" - check the first 'N' bytes from the next chunk against
    /// stopper's suffix.
    StopperPrefixFound(usize),
    /// Stopper byte found at Nth "index"
    StopperFound(usize),
    /// Stopper's suffix (of length "N" in this variant) matches with the first "N" bytes in chunk.
    StopperExtendsFromPrevious(usize),
    /// All bytes from previous chunk are wasted - they have to passed to the writer.
    PreviousBytesWasted,
    /// Stopper not found in this chunk.
    StopperNotFound,
}

impl<R, W> StreamingBuffer<R, W>
    where R: Read, W: Write
{
    /// Check the previously unwritten bytes (with the new chunk) for possible stopper.
    fn check_previous_bytes_with(&mut self, parent_bytes: &[u8]) {
        let stop_len = self.stop_bytes.len();
        if let StreamerStatus::StopperPrefixFound(len) = self.status.get() {
            if parent_bytes.starts_with(&self.stop_bytes[(stop_len - len)..]) {
                self.status.set(StreamerStatus::StopperExtendsFromPrevious(len));
                return
            }
        }

        self.status.set(StreamerStatus::PreviousBytesWasted);
    }

    /// Isolate some bytes for checking against magic during the incoming of next chunk from buffer.
    fn check_suffix_bytes<'a>(&mut self, parent_bytes: &'a [u8]) {
        if parent_bytes.is_empty() {
            return
        }

        if let StreamerStatus::StopperExtendsFromPrevious(_) = self.status.get() {
            // Stopper found - we no longer need to stream, so we don't care now!
            return
        }

        let parent_len = parent_bytes.len();
        let stop_len = self.stop_bytes.len();

        self.prev_bytes_unwritten = if parent_bytes.len() >= stop_len {
            parent_bytes[(parent_len - stop_len)..].into()
        } else {
            parent_bytes.into()
        };

        let prev_suffix = &self.prev_bytes_unwritten[..];
        let stopper_prefix = &self.stop_bytes[..];
        for i in 0..prev_suffix.len() {
            let prev = &prev_suffix[i..];
            let stopper = &stopper_prefix[..prev.len()];
            if prev.starts_with(stopper) {
                let remaining = stop_len - prev.len();
                if remaining == 0 {
                    self.status.set(StreamerStatus::StopperFound(parent_len - stop_len));
                    return
                } else {
                    self.status.set(StreamerStatus::StopperPrefixFound(remaining));
                    return
                }
            }
        }

        self.status.set(StreamerStatus::StopperNotFound);
    }

    /// Get the suffix bytes from previous chunk (which haven't been written yet)
    #[inline]
    fn get_unwritten_bytes(&self) -> Option<&[u8]> {
        match self.status.get() {
            StreamerStatus::StopperExtendsFromPrevious(len) =>
                Some(&self.prev_bytes_unwritten[..len]),
            StreamerStatus::PreviousBytesWasted =>
                Some(&self.prev_bytes_unwritten),
            _ => None,
        }
    }
}

impl<R, W> StreamingBuffer<R, W>
    where R: Read + 'static, W: Write + 'static
{
    /// Start streaming. This returns a future that resolves to the read/write halves.
    pub fn stream(mut self) -> ClusterFuture<(BufReader<R>, BufWriter<W>)> {
        let (mut r, mut w) = (self.reader.take().unwrap(), self.writer.take().unwrap());

        let mut content_ended = false;
        loop {
            let consumed = {
                let mut call = || -> Result<_, _> {
                    let bytes = r.fill_buf()?;

                    let (consume_amt, write_amt) = if self.stop_bytes.is_empty() {
                        // If there's no stopper, then we consume and write everything.
                        (bytes.len(), bytes.len())
                    } else {
                        self.check_previous_bytes_with(bytes);
                        if let Some(prev_bytes) = self.get_unwritten_bytes() {
                            w.write_all(&prev_bytes)?;
                        }

                        self.check_suffix_bytes(bytes);
                        match self.status.get() {
                            StreamerStatus::StopperFound(idx) => {
                                content_ended = true;
                                (idx + self.stop_bytes.len(), idx)
                            },
                            StreamerStatus::StopperExtendsFromPrevious(suffix_len) => {
                                content_ended = true;
                                (suffix_len, 0)
                            },
                            _ => (bytes.len(), bytes.len().saturating_sub(self.stop_bytes.len())),
                        }
                    };

                    if bytes.is_empty() {
                        content_ended = true;
                        return Ok(0)
                    }

                    if write_amt > 0 {
                        w.write_all(&bytes[..write_amt])?;
                    }

                    Ok(consume_amt)
                };

                call()
            };

            match consumed {
                Ok(len) => {
                    r.consume(len);
                    if content_ended {
                        break
                    }
                },
                Err(ClusterError::Io(ref e)) if e.kind() == ErrorKind::WouldBlock => (),
                Err(e) => return Box::new(future::err(e)) as ClusterFuture<_>,
            }
        }

        Box::new(future::ok((r, w))) as ClusterFuture<_>
    }
}

/* Tests */

#[cfg(test)]
mod tests {
    use futures::Future;
    use rand::{self, Rng};
    use super::{StreamingBuffer, StreamerStatus};

    use std::cell::Cell;
    use std::io::{BufRead, BufReader, BufWriter, Cursor};

    impl StreamingBuffer<Cursor<Vec<u8>>, Vec<u8>> {
        fn new(bytes: Vec<u8>, write_bytes: Vec<u8>, cap: usize, stop: &[u8]) -> Self {
            StreamingBuffer {
                reader: Some(BufReader::with_capacity(cap, Cursor::new(bytes))),
                writer: Some(BufWriter::with_capacity(cap, write_bytes)),
                stop_bytes: stop.into(),
                prev_bytes_unwritten: Box::new([]),
                status: Cell::new(StreamerStatus::StopperNotFound),
            }
        }
    }

    /// Test that the streamer successfully streams when the stream is more than its capacity.
    #[test]
    fn test_stream_more_than_capacity() {
        let mut buf = [0; 32 * 1024];       // 32 kB stream
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut buf);

        let buf = Vec::from(&buf[..]);
        let streamer = StreamingBuffer::new(buf, Vec::new(), 256, &[]);     // 256 byte buffer
        let (r, w) = streamer.stream().wait().unwrap();
        let (buf, out) = (r.into_inner().into_inner(), w.into_inner().unwrap());
        assert_eq!(&buf[..], &out[..]);
    }

    /// Test that the streamer stops once it encounters the "stopper" bytes at EOF - when it
    /// flushes everything other than the stopper to the writer.
    #[test]
    fn test_stream_with_trailing_stopper() {
        let mut buf = [0; 256];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut buf);

        let stop_bytes = &buf[248..];       // assume that last 8 bytes indicate stopper bytes.
        let buf = Vec::from(&buf[..]);
        let streamer = StreamingBuffer::new(buf, Vec::new(), 16, stop_bytes);
        let (r, w) = streamer.stream().wait().unwrap();
        let (buf, out) = (r.into_inner().into_inner(), w.into_inner().unwrap());
        assert_eq!(&buf[..248], &out[..]);      // last 8 bytes have been ignored
    }

    /// The streamer should detect when the stopper bytes are split by buffered reading, and it
    /// should bail out successfully.
    #[test]
    fn test_stream_with_stopper_shared_between() {
        let mut buf = [0; 256];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut buf);

        // When the buffer size is 16, some of the stopper will be in the first buffer,
        // and the remaining will be in the next.
        let stop_bytes = &buf[14..22];
        let input = Vec::from(&buf[..]);
        let streamer = StreamingBuffer::new(input, Vec::new(), 16, stop_bytes);
        let (mut r, w) = streamer.stream().wait().unwrap();
        {   // Check that `BufReader` cursor is at 22 (i.e., after consuming stopper)
            let remaining = r.fill_buf().unwrap();
            assert_eq!(remaining, &buf[22..32]);
        }

        let cursor = r.into_inner();
        assert_eq!(cursor.position(), 32);      // Actual cursor moved to the end of second chunk
        let out = w.into_inner().unwrap();
        assert_eq!(&buf[..14], &out[..]);       // Writer has everything until the stopper
    }
}
