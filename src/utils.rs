use std::hash::Hasher;

const DJB2_MAGIC: u64 = 5381;

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
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for i in bytes {
            self.0 = ((self.0 << 5) + self.0) + *i as u64;
        }
    }
}
