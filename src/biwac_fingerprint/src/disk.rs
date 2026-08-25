//! `.biwafp` のディスク形式。
//!
//! `.biwameta` と別ファイルにしてあるのは 2 つの理由による:
//!
//! - 鮮度判定は `.biwameta` をデコードせずに済ませたい
//! - ソースのハッシュはインタフェースではないので `.biwameta` に混ぜたくない
//!   (混ぜると SVH がソースの変更で必ず変わり、伝播を打ち切れなくなる)
//!
//! cargo が `.fingerprint/` を成果物と分けているのと同じ理由である。
//!
//! 物理レイアウト:
//!   [MAGIC: 4B][version: u32 LE]
//!   [compiler: u64][manifest: u64][own_svh: u64]
//!   [dep_count: u32][(pkg_id: u32, svh: u64); dep_count]
//!   [src_count: u32][(path_len: u32, path bytes, len: u64, hash: u64); src_count]

use biwac_base::PackageId;
use biwac_hash::Hash64;

use crate::{BIWAC_FINGERPRINT_FORMAT_VERSION, Fingerprint, SourceEntry};

pub const BIWAC_FINGERPRINT_MAGIC: &[u8; 4] = b"bwfp";

#[derive(Debug)]
pub enum FingerprintDecodeError {
    UnexpectedEnd,
    InvalidMagic,
    /// 形式が変わった。破損ではないので、単に「前回の情報は使えない」と扱う。
    InvalidVersion {
        got: u32,
    },
    InvalidUtf8,
}

impl std::fmt::Display for FingerprintDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEnd => write!(f, "unexpected end of data"),
            Self::InvalidMagic => write!(f, "invalid magic bytes (not a .biwafp file)"),
            Self::InvalidVersion { got } => {
                write!(f, "unsupported .biwafp format version: {got}")
            }
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in path"),
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], FingerprintDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(FingerprintDecodeError::UnexpectedEnd);
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, FingerprintDecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, FingerprintDecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn hash(&mut self) -> Result<Hash64, FingerprintDecodeError> {
        Ok(Hash64::from_u64(self.u64()?))
    }

    fn str(&mut self) -> Result<String, FingerprintDecodeError> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| FingerprintDecodeError::InvalidUtf8)
    }
}

fn push_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

impl Fingerprint {
    pub fn encode_file(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(BIWAC_FINGERPRINT_MAGIC.as_ref());
        buf.extend_from_slice(&BIWAC_FINGERPRINT_FORMAT_VERSION.to_le_bytes());

        buf.extend_from_slice(&self.compiler.as_u64().to_le_bytes());
        buf.extend_from_slice(&self.manifest.as_u64().to_le_bytes());
        buf.extend_from_slice(&self.own_svh.as_u64().to_le_bytes());

        buf.extend_from_slice(&(self.deps.len() as u32).to_le_bytes());
        for (pkg_id, svh) in &self.deps {
            buf.extend_from_slice(&pkg_id.value().to_le_bytes());
            buf.extend_from_slice(&svh.as_u64().to_le_bytes());
        }

        buf.extend_from_slice(&(self.sources.len() as u32).to_le_bytes());
        for s in &self.sources {
            push_str(&mut buf, &s.path);
            buf.extend_from_slice(&s.len.to_le_bytes());
            buf.extend_from_slice(&s.hash.as_u64().to_le_bytes());
        }

        buf
    }

    pub fn decode_file(data: &[u8]) -> Result<Self, FingerprintDecodeError> {
        let mut r = Reader { data, pos: 0 };

        if r.take(4)? != BIWAC_FINGERPRINT_MAGIC.as_ref() {
            return Err(FingerprintDecodeError::InvalidMagic);
        }
        let version = r.u32()?;
        if version != BIWAC_FINGERPRINT_FORMAT_VERSION {
            return Err(FingerprintDecodeError::InvalidVersion { got: version });
        }

        let compiler = r.hash()?;
        let manifest = r.hash()?;
        let own_svh = r.hash()?;

        let dep_count = r.u32()?;
        let mut deps = Vec::with_capacity(dep_count as usize);
        for _ in 0..dep_count {
            let pkg_id = PackageId::new(r.u32()?);
            deps.push((pkg_id, r.hash()?));
        }

        let src_count = r.u32()?;
        let mut sources = Vec::with_capacity(src_count as usize);
        for _ in 0..src_count {
            let path = r.str()?;
            let len = r.u64()?;
            let hash = r.hash()?;
            sources.push(SourceEntry { path, len, hash });
        }

        Ok(Self {
            compiler,
            manifest,
            own_svh,
            deps,
            sources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let fp = Fingerprint {
            compiler: Hash64::from_u64(1),
            manifest: Hash64::from_u64(2),
            own_svh: Hash64::from_u64(3),
            deps: vec![
                (PackageId::new(7), Hash64::from_u64(70)),
                (PackageId::new(9), Hash64::from_u64(90)),
            ],
            sources: vec![
                SourceEntry {
                    path: "lib.biwa".into(),
                    len: 10,
                    hash: Hash64::from_u64(11),
                },
                SourceEntry {
                    path: "math/pos.biwa".into(),
                    len: 20,
                    hash: Hash64::from_u64(22),
                },
            ],
        };

        let decoded = Fingerprint::decode_file(&fp.encode_file()).unwrap();
        assert_eq!(fp, decoded);
    }

    #[test]
    fn rejects_foreign_data() {
        assert!(matches!(
            Fingerprint::decode_file(b"not a fingerprint"),
            Err(FingerprintDecodeError::InvalidMagic)
        ));
    }
}
