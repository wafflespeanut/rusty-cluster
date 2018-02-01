use buffered::StreamingBuffer;
use byteorder::{BigEndian, ByteOrder};
use connection::Connection;
use errors::ClusterFuture;
use futures::{Future, future};
use tokio_io::{AsyncRead, AsyncWrite};
use walkdir::WalkDir;

use std::path::{Path, PathBuf};

pub struct PathSync<R: AsyncRead, W: AsyncWrite>(pub Connection<R, W>);

enum_from_primitive! {
    /// Flag to represent the type of file.
    #[repr(u8)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum FileType {
        Directory = 0,
        File      = 1,
    }
}

impl Into<u8> for FileType {
    fn into(self) -> u8 { self as u8 }
}

impl<R, W> PathSync<R, W>
    where R: AsyncRead + 'static, W: AsyncWrite + 'static
{
    pub fn source_to_stream<P, Q>(self, source: P, dest: Q) -> ClusterFuture<Connection<R, W>>
        where P: AsRef<Path>, Q: AsRef<Path>
    {
        let walker = WalkDir::new(source.as_ref());
        let source_path = PathBuf::from(source.as_ref());
        let dest = dest.as_ref().to_string_lossy().into_owned();

        let async_stream = self.0.write_bytes(dest.into_bytes())
            .and_then(|c| c.write_bytes(&[b'\n']))
            .and_then(move |c| -> ClusterFuture<Connection<R, W>> {
                let mut conn = c;
                let mut parent = source_path;
                // Since all paths are relative to the tip of source, don't trim the tip.
                parent.pop();

                for entry in walker {
                    let entry = future_try!(entry);
                    let path = PathBuf::from(entry.path());
                    let entry_type = entry.file_type();

                    // only files and dirs - no symlinks
                    if entry_type.is_symlink() {
                        info!("Ignoring symlink: {}", path.display());
                        continue
                    }

                    let rel_path = PathBuf::from(path.strip_prefix(&parent).unwrap());
                    let rel_path_str = rel_path.to_string_lossy().into_owned();
                    let mut size_buf = [0; 8];
                    if entry_type.is_dir() {
                        let async_conn = conn.write_bytes(size_buf)
                            .and_then(|c| c.write_flag(FileType::Directory))
                            .and_then(move |c| c.write_bytes(rel_path_str.into_bytes()))
                            .and_then(|c| c.write_bytes(&[b'\n']));
                        conn = future_try_wait!(async_conn);
                        println!("{}", rel_path.display());
                        continue
                    }

                    let metadata = future_try!(entry.metadata());
                    BigEndian::write_u64(&mut size_buf, metadata.len());
                    let async_conn = conn.write_bytes(size_buf)
                        .and_then(|c| c.write_flag(FileType::File))
                        .and_then(move |c| c.write_bytes(rel_path_str.into_bytes()))
                        .and_then(|c| c.write_bytes(&[b'\n']))
                        .and_then(move |c| {
                            let (r, w, m) = c.into();
                            StreamingBuffer::file_to_stream(path, w)
                                            .and_then(|s| s.stream())
                                            .map(move |(_fd, w)| Connection::from((r, w, m)))
                        }).and_then(|c| c.write_magic());

                    conn = future_try_wait!(async_conn);
                    println!("{}: {}", rel_path.display(), metadata.len());
                }

                Box::new(future::ok(conn))
            });

        Box::new(async_stream) as ClusterFuture<_>
    }
}

/* Tests */

#[cfg(test)]
mod tests {
    use byteorder::{BigEndian, ByteOrder};
    use connection::Connection;
    use futures::Future;
    use rand::{self, Rng};
    use super::{FileType, PathSync};
    use walkdir::WalkDir;

    use std::fs::File;
    use std::io::{BufReader, BufWriter, Cursor, Read};
    use std::path::PathBuf;

    #[test]
    fn test_single_file_to_stream() {
        let mut magic = [0; 16];
        let mut rng = rand::thread_rng();
        let buf = Cursor::new(vec![]);
        rng.fill_bytes(&mut magic);

        let parts = (BufReader::new(buf.clone()), BufWriter::new(buf), magic);
        let sync = PathSync(Connection::from(parts));
        let mut test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_path.push("..");
        test_path.push("tests");
        test_path.push("test_path");
        test_path.push("foobar");

        let conn = sync.source_to_stream(&test_path, "/tmp/foo").wait().unwrap();
        let (_, writer, _) = conn.into();
        let buf = writer.into_inner().unwrap().into_inner();

        let mut out = vec![];
        // begins with destination path
        out.extend_from_slice(&b"/tmp/foo\n"[..]);

        let mut fd = File::open(&test_path).unwrap();
        let metadata = fd.metadata().unwrap();
        let mut size_buf = [0; 8];
        BigEndian::write_u64(&mut size_buf, metadata.len());
        out.extend_from_slice(&size_buf[..]);   // file size in big endian

        out.push(FileType::File as u8);         // flag for file
        out.extend_from_slice(&b"foobar"[..]);  // file path (in this case, just the name)
        out.push(10);
        fd.read_to_end(&mut out).unwrap();      // file contents
        out.extend_from_slice(&magic[..]);      // magic ends the stream

        assert_eq!(buf, out);
    }

    #[test]
    fn test_recursive_path_to_stream() {
        let mut magic = [0; 16];
        let mut rng = rand::thread_rng();
        let buf = Cursor::new(vec![]);
        rng.fill_bytes(&mut magic);

        let parts = (BufReader::new(buf.clone()), BufWriter::new(buf), magic);
        let sync = PathSync(Connection::from(parts));
        let mut test_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_dir_path.push("..");
        test_dir_path.push("tests");
        let test_parent = test_dir_path.clone();
        test_dir_path.push("test_path");

        let conn = sync.source_to_stream(&test_dir_path, "/tmp/foo").wait().unwrap();
        let (_, writer, _) = conn.into();
        let buf = writer.into_inner().unwrap().into_inner();

        let mut out = vec![];
        out.extend_from_slice(&b"/tmp/foo\n"[..]);      // destination path

        let walker = WalkDir::new(&test_dir_path);
        for entry in walker {
            let entry = entry.expect("entry");
            let entry_type = entry.file_type();
            if entry_type.is_symlink() {
                unreachable!();
            }

            let metadata = entry.metadata().expect("metadata");
            let path = entry.path().strip_prefix(&test_parent).unwrap();
            let path_str = path.to_string_lossy();

            let mut size_buf = [0; 8];
            let flag = if entry_type.is_file() {
                BigEndian::write_u64(&mut size_buf, metadata.len());
                FileType::File
            } else {
                FileType::Directory
            };

            // size of entry node
            out.extend_from_slice(&size_buf[..]);
            out.push(flag as u8);
            // finally, the path itself
            out.extend(path_str.as_bytes());
            // ... terminated by newline.
            out.push(10);

            if entry_type.is_file() {   // write contents if it's a file.
                let mut fd = File::open(entry.path()).unwrap();
                fd.read_to_end(&mut out).unwrap();
                out.extend_from_slice(&magic[..]);
            }
        }

        assert_eq!(buf, out);
    }
}
