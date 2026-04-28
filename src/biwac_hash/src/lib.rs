#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash64(u64);

pub trait AsHash64 {
    fn as_hash64(&self) -> Hash64;
}

impl AsHash64 for u64 {
    fn as_hash64(&self) -> Hash64 {
        Hash64(*self)
    }
}

pub struct StableHasher64 {}

impl StableHasher64 {
    pub fn new() -> Self {
        Self {}
    }

    pub fn hash(&mut self, bytes: &[u8]) {
        todo!()
    }

    pub fn finish(self) -> Hash64 {
        todo!()
    }
}
