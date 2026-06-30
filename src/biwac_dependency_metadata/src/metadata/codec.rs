use crate::error::DepMetadataError;

/// ディスク上のバイト列からデコードする。
/// 戻り値は `(値, 消費バイト数)` の組。
/// size_of::<Self>() に依存しないため、再帰的・可変長な型にも適用できる。
pub trait DiskDecode: Sized {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError>;
}

/// 値をバイト列に追記エンコードする。
/// `Vec<u8>` への追記方式により、可変長型でも余分なアロケーションが生じない。
pub trait DiskEncode {
    fn encode(&self, buf: &mut Vec<u8>);
}

// --- u32 (little endian) ---

impl DiskDecode for u32 {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        if bytes.len() < 4 {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: 4,
                available: bytes.len(),
            });
        }
        Ok((u32::from_le_bytes(bytes[..4].try_into().unwrap()), 4))
    }
}

impl DiskEncode for u32 {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_le_bytes());
    }
}

// --- DiskVec<T>: [len: u32][T; len] ---
//
// 固定長・可変長どちらの T にも使える。
// デコードは T::decode を len 回呼ぶだけで、size_of::<T>() に依存しない。

#[derive(Debug)]
pub struct DiskVec<T>(pub Vec<T>);

impl<T: DiskDecode> DiskDecode for DiskVec<T> {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let (len, mut pos) = u32::decode(bytes)?;
        let mut values = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let (val, consumed) = T::decode(&bytes[pos..])?;
            pos += consumed;
            values.push(val);
        }
        Ok((DiskVec(values), pos))
    }
}

impl<T: DiskEncode> DiskEncode for DiskVec<T> {
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.0.len() as u32).encode(buf);
        for val in &self.0 {
            val.encode(buf);
        }
    }
}

// --- マクロ: u32 ラッパー型の DiskDecode/DiskEncode ---

macro_rules! impl_u32_newtype_codec {
    ($t:ty) => {
        impl DiskDecode for $t {
            fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
                let (v, n) = u32::decode(bytes)?;
                Ok((Self(v), n))
            }
        }
        impl DiskEncode for $t {
            fn encode(&self, buf: &mut Vec<u8>) {
                self.0.encode(buf);
            }
        }
    };
}

pub(crate) use impl_u32_newtype_codec;
