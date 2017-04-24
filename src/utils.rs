use std::hash::Hasher;

const DJB2_MAGIC: u64 = 5381;
const SINGLE_QUOTE: u8 = 39;
const DOUBLE_QUOTE: u8 = 34;
const SPACE: u8 = 32;

/// Based on DJB2 - http://www.cse.yorku.ca/~oz/hash.html
///
/// This is very simple and very fast. For its magic number (5381), it produces
/// unique hashes for all numbers in the range 0..8448 (regardless of their types)
/// Anything beyond that range will be a collision.
pub struct DjB2(u64);

impl DjB2 {
    pub fn new() -> DjB2 {
        DjB2(DJB2_MAGIC)
    }
}

impl Hasher for DjB2 {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for i in bytes {
            self.0 = ((self.0 << 5) + self.0) + *i as u64;
        }
    }
}

pub fn split_args(bytes: Vec<u8>) -> Vec<String> {
    let mut vec = vec![String::new()];
    let mut last_open: Option<u8> = None;
    for byte in bytes {
        match byte {
            SINGLE_QUOTE | DOUBLE_QUOTE => match last_open {
                Some(b) if byte == b => {
                    last_open = None;
                    continue
                },
                _ => (),
            },
            SPACE if last_open.is_none() => {
                vec.push(String::new());
            },
            _ => (),
        }

        vec.last_mut().unwrap().push(byte as char);
    }

    if vec.len() == 1 && !vec[0].is_empty() {
        vec.push(String::new());
    }

    vec
}
