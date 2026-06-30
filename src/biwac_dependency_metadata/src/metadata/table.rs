use std::sync::OnceLock;

use super::{
    body::SymbolBody,
    format::{DiskSourceInfo, DiskStringOffset, DiskSymbolHeader},
};
use crate::error::DepMetadataError;

// --- StringTable ---
//
// 文字列テーブル: null 終端 UTF-8 文字列の連続バイト列。
// 読み側は DiskStringOffset (バイトオフセット) で参照する。
// encode 時は push() でオフセットを得る。

pub struct StringTable {
    data: Vec<u8>,
}

impl StringTable {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn empty() -> Self {
        Self { data: Vec::new() }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn get(&self, offset: DiskStringOffset) -> Result<&str, DepMetadataError> {
        let start = offset.0 as usize;
        if start >= self.data.len() {
            return Err(DepMetadataError::StringOffsetOutOfBounds {
                offset: offset.0,
                table_len: self.data.len() as u32,
            });
        }
        let len = self.data[start..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(DepMetadataError::NulTerminatorNotFound { offset: offset.0 })?;
        std::str::from_utf8(&self.data[start..start + len])
            .map_err(|_| DepMetadataError::InvalidUtf8)
    }

    /// 文字列を追加し、そのオフセットを返す。
    pub fn push(&mut self, s: &str) -> DiskStringOffset {
        let offset = DiskStringOffset(self.data.len() as u32);
        self.data.extend_from_slice(s.as_bytes());
        self.data.push(0);
        offset
    }
}

// --- SourceFileTable ---

pub struct SourceFileTable {
    entries: Vec<DiskSourceInfo>,
}

impl SourceFileTable {
    pub fn new(entries: Vec<DiskSourceInfo>) -> Self {
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, idx: u32) -> Result<&DiskSourceInfo, DepMetadataError> {
        self.entries
            .get(idx as usize)
            .ok_or(DepMetadataError::FileIndexOutOfBounds {
                index: idx,
                file_count: self.entries.len() as u32,
            })
    }

    pub fn entries(&self) -> &[DiskSourceInfo] {
        &self.entries
    }
}

// --- LazyDiskVec ---
//
// sym_body_table の lazy-load 実装。
// 生バイト列 (`bytes`) を保持し、初回アクセス時にデコードしてキャッシュする。
//
// 各ボディのレイアウト (bytes 内):
//   offset → [body_size: u32][body: body_size B]
//
// DiskSymbolHeader.offset がこの先頭 (body_size プレフィックス) を指す。
// OnceCell<SymbolBody> により、初期化は一度だけ行われ、以降は &SymbolBody を返す。

pub struct LazyDiskVec {
    bytes: Vec<u8>,
    /// OnceLock: 初回アクセス時にデコード・キャッシュ。以降は &SymbolBody を返す。
    /// get_or_try_init は未安定のため、get() + set() の組み合わせで実装する。
    pub(crate) cache: Vec<OnceLock<SymbolBody>>,
}

impl LazyDiskVec {
    pub fn new(bytes: Vec<u8>, sym_count: usize) -> Self {
        let cache = (0..sym_count).map(|_| OnceLock::new()).collect();
        Self { bytes, cache }
    }

    /// sym_idx 番目のシンボルのボディを取得する。
    /// 初回呼び出し時のみデコードを行い、以降はキャッシュを返す。
    pub fn get(
        &self,
        sym_idx: usize,
        hdr: &DiskSymbolHeader,
    ) -> Result<&SymbolBody, DepMetadataError> {
        if let Some(cached) = self.cache[sym_idx].get() {
            return Ok(cached);
        }
        let body = self.decode_at(hdr)?;
        // 競合しても安全: 同じボディをセットするだけで、既セット済みならset()は失敗するだけ
        let _ = self.cache[sym_idx].set(body);
        Ok(self.cache[sym_idx].get().unwrap())
    }

    fn decode_at(&self, hdr: &DiskSymbolHeader) -> Result<SymbolBody, DepMetadataError> {
        let offset = hdr.offset.0 as usize;
        if offset + 4 > self.bytes.len() {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: offset + 4,
                available: self.bytes.len(),
            });
        }
        let body_size = u32::from_le_bytes(self.bytes[offset..offset + 4].try_into().unwrap());
        let data_start = offset + 4;
        let data_end = data_start + body_size as usize;
        if data_end > self.bytes.len() {
            return Err(DepMetadataError::BodySizeMismatch {
                declared: body_size,
                available: self.bytes.len() - data_start,
            });
        }

        SymbolBody::decode(hdr.kind, &self.bytes[data_start..data_end])
    }
}
