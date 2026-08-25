//! セッション・環境をまたいで安定な 64bit ハッシュ。
//!
//! std の `DefaultHasher` は「同じ入力に同じ値を返すこと」をリリース間で保証しておらず、
//! `RandomState` は実行ごとに鍵が変わる。
//! 差分ビルドのフィンガープリントはビルドをまたいで比較するものなので、どちらも使えない。
//!
//! rustc が `StableHasher` を自前で持っているのと同じ理由である。
//!
//! アルゴリズムは FNV-1a 64 に splitmix64 の finalizer を掛けたもの。
//! 他実装との相互運用は目的ではなく、
//! **同じ入力からは常に同じ値**であることだけを保証する。

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hash64(u64);

impl Hash64 {
    pub const ZERO: Self = Self(0);

    #[inline]
    pub fn from_u64(v: u64) -> Self {
        Self(v)
    }

    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for Hash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

pub trait AsHash64 {
    fn as_hash64(&self) -> Hash64;
}

impl AsHash64 for u64 {
    fn as_hash64(&self) -> Hash64 {
        Hash64(*self)
    }
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone)]
pub struct StableHasher64 {
    state: u64,
}

impl StableHasher64 {
    pub fn new() -> Self {
        Self {
            state: FNV_OFFSET_BASIS,
        }
    }

    /// 生バイト列をそのまま混ぜる。
    ///
    /// 長さは混ぜないので、可変長の値には [`Self::write_bytes`] か
    /// [`Self::write_str`] を使うこと。
    pub fn hash(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.state ^= *b as u64;
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    #[inline]
    pub fn write_u8(&mut self, v: u8) {
        self.hash(&[v]);
    }

    #[inline]
    pub fn write_u32(&mut self, v: u32) {
        self.hash(&v.to_le_bytes());
    }

    #[inline]
    pub fn write_u64(&mut self, v: u64) {
        self.hash(&v.to_le_bytes());
    }

    #[inline]
    pub fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }

    /// 長さを前置してからバイト列を混ぜる。
    ///
    /// 前置しないと `("ab", "c")` と `("a", "bc")` が同じ値になる。
    /// rustc の `StableCrateId` が `-Cmetadata` の各要素に対して同じことをしている。
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_usize(bytes.len());
        self.hash(bytes);
    }

    #[inline]
    pub fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    #[inline]
    pub fn write_hash(&mut self, h: Hash64) {
        self.write_u64(h.as_u64());
    }

    /// FNV-1a は上位ビットの拡散が弱いので、splitmix64 の finalizer を通す。
    pub fn finish(self) -> Hash64 {
        let mut x = self.state;
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
        Hash64(x)
    }
}

impl Default for StableHasher64 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_of(f: impl FnOnce(&mut StableHasher64)) -> Hash64 {
        let mut h = StableHasher64::new();
        f(&mut h);
        h.finish()
    }

    #[test]
    fn same_input_same_hash() {
        assert_eq!(
            hash_of(|h| h.write_str("biwa")),
            hash_of(|h| h.write_str("biwa"))
        );
    }

    #[test]
    fn different_input_different_hash() {
        assert_ne!(
            hash_of(|h| h.write_str("biwa")),
            hash_of(|h| h.write_str("biwb"))
        );
    }

    /// 長さの前置がないと ("ab","c") と ("a","bc") が衝突する。
    #[test]
    fn length_prefix_separates_concatenations() {
        let a = hash_of(|h| {
            h.write_str("ab");
            h.write_str("c");
        });
        let b = hash_of(|h| {
            h.write_str("a");
            h.write_str("bc");
        });
        assert_ne!(a, b);
    }

    /// 実行ごとに値が変わらないこと (定数として固定しておく)。
    /// アルゴリズムを変えたらこの値も変わる = キャッシュが一括無効化される。
    #[test]
    fn is_stable_across_runs() {
        assert_eq!(hash_of(|h| h.write_str("std")).as_u64(), 0xc90f1fd2cd248231);
    }
}
