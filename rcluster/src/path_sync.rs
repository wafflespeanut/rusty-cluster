use byteorder::{BigEndian, ByteOrder};
use connection::Connection;
use errors::ClusterFuture;
use futures::{Future, future};
use tokio_io::{AsyncRead, AsyncWrite};
use walkdir::WalkDir;

use std::path::PathBuf;

pub struct PathSync<S: AsyncRead + AsyncWrite>(pub Connection<S>);

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

impl<S> PathSync<S>
    where S: AsyncRead + AsyncWrite + 'static
{
    pub fn source_to_stream<P>(self, source: P, dest: P) -> ClusterFuture<Connection<S>>
        where P: AsRef<str>
    {
        let walker = WalkDir::new(source.as_ref());
        let source_path = PathBuf::from(source.as_ref());
        let dest = String::from(dest.as_ref());

        let async_stream = self.0.write_bytes(dest.into_bytes())
            .and_then(|c| c.write_bytes(&[b'\n']))
            .and_then(move |c| -> ClusterFuture<Connection<S>> {
                let mut conn = c;
                let src = source_path;
                for entry in walker {
                    let entry = future_try!(entry);
                    let path = entry.path();
                    let entry_type = entry.file_type();

                    // only files and dirs - no symlinks
                    if entry_type.is_symlink() {
                        info!("Ignoring symlink: {}", path.display());
                        continue
                    }

                    let rel_path = PathBuf::from(path.strip_prefix(&src).unwrap());
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
                        .and_then(|c| c.write_bytes(&[b'\n']));
                    conn = future_try_wait!(async_conn);
                    println!("{}: {}", rel_path.display(), metadata.len());
                }

                Box::new(future::ok(conn))
            });

        Box::new(async_stream) as ClusterFuture<_>
    }
}
