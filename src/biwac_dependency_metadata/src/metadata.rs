mod body;
mod codec;
mod format;
mod table;

use crate::error::DepMetadataError;
use body::SymbolBody;
use codec::{DiskDecode, DiskEncode};
use format::{
    BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION, BIWAC_DEPENDENCY_METADATA_MAGIC, DiskBodyOffset,
    DiskSourceInfo, DiskSymbolHeader,
};
use table::{LazyDiskVec, SourceFileTable, StringTable};

/// ロード済みの依存パッケージメタデータ。
/// sym_hdr_table と source_file_table は常にフルロード (固定長・小サイズ)。
/// sym_body_table は LazyDiskVec による遅延デコード。
/// string_table は常にフルロード (参照が頻繁なため)。
pub struct DepMetadata {
    pub sym_hdrs: Vec<DiskSymbolHeader>,
    pub sym_bodies: LazyDiskVec,
    pub source_files: SourceFileTable,
    pub strings: StringTable,
    /// ルートモジュール (lib モジュール) のシンボルインデックス
    pub root_sym_idx: u32,
}

impl DepMetadata {
    pub fn new() -> Self {
        todo!()
    }

    // --- Decode ---

    pub fn decode_file(data: &[u8]) -> Result<Self, DepMetadataError> {
        let mut pos = 0;

        // ヘッダ (magic + version)
        if data.len() < 8 {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: 8,
                available: data.len(),
            });
        }
        if &data[0..4] != BIWAC_DEPENDENCY_METADATA_MAGIC.as_ref() {
            return Err(DepMetadataError::InvalidMagic);
        }
        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if version != BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION {
            return Err(DepMetadataError::InvalidVersion { got: version });
        }
        pos += 8;

        // sym_hdr_table: [count: u32][DiskSymbolHeader; count]
        let (sym_hdr_count, n) = u32::decode(&data[pos..])?;
        pos += n;
        let mut sym_hdrs = Vec::with_capacity(sym_hdr_count as usize);
        for _ in 0..sym_hdr_count {
            let (hdr, n) = DiskSymbolHeader::decode(&data[pos..])?;
            pos += n;
            sym_hdrs.push(hdr);
        }

        // root_sym_idx
        let (root_sym_idx, n) = u32::decode(&data[pos..])?;
        pos += n;

        // sym_body_table: [total_bytes: u32][body_data: total_bytes B]
        let (sym_body_total, n) = u32::decode(&data[pos..])?;
        pos += n;
        let body_end = pos + sym_body_total as usize;
        if body_end > data.len() {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: body_end,
                available: data.len(),
            });
        }
        let sym_body_bytes = data[pos..body_end].to_vec();
        pos = body_end;
        let sym_bodies = LazyDiskVec::new(sym_body_bytes, sym_hdrs.len());

        // source_file_table: [count: u32][DiskSourceInfo; count]
        let (file_count, n) = u32::decode(&data[pos..])?;
        pos += n;
        let mut source_file_entries = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let (info, n) = DiskSourceInfo::decode(&data[pos..])?;
            pos += n;
            source_file_entries.push(info);
        }
        let source_files = SourceFileTable::new(source_file_entries);

        // string_table: [total_bytes: u32][data: total_bytes B]
        let (str_total, n) = u32::decode(&data[pos..])?;
        pos += n;
        let str_end = pos + str_total as usize;
        if str_end > data.len() {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: str_end,
                available: data.len(),
            });
        }
        let strings = StringTable::new(data[pos..str_end].to_vec());

        Ok(Self {
            sym_hdrs,
            sym_bodies,
            source_files,
            strings,
            root_sym_idx,
        })
    }

    // --- Encode ---

    pub fn encode_file(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // ヘッダ
        buf.extend_from_slice(BIWAC_DEPENDENCY_METADATA_MAGIC.as_ref());
        BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION.encode(&mut buf);

        // sym_hdr_table
        (self.sym_hdrs.len() as u32).encode(&mut buf);
        for hdr in &self.sym_hdrs {
            hdr.encode(&mut buf);
        }

        // root_sym_idx
        self.root_sym_idx.encode(&mut buf);

        // sym
        let mut builder = BodyBuilder::new();
        for b in &self.sym_bodies.cache {
            builder.push(b.get().unwrap());
        }
        let sym_body_bytes = builder.finish();

        // sym_body_table
        (sym_body_bytes.len() as u32).encode(&mut buf);
        buf.extend_from_slice(&sym_body_bytes);

        // source_file_table
        (self.source_files.len() as u32).encode(&mut buf);
        for info in self.source_files.entries() {
            info.encode(&mut buf);
        }

        // string_table
        let str_data = self.strings.data();
        (str_data.len() as u32).encode(&mut buf);
        buf.extend_from_slice(str_data);

        buf
    }

    pub fn get_symbol_body(&self, sym_idx: usize) -> Result<&SymbolBody, DepMetadataError> {
        if sym_idx >= self.sym_hdrs.len() {
            return Err(DepMetadataError::SymbolIndexOutOfBounds {
                index: sym_idx as u32,
                sym_count: self.sym_hdrs.len() as u32,
            });
        }
        self.sym_bodies.get(sym_idx, &self.sym_hdrs[sym_idx])
    }

    pub fn get_str(&self, offset: format::DiskStringOffset) -> Result<&str, DepMetadataError> {
        self.strings.get(offset)
    }
}

// --- BodyBuilder: encode 時にシンボルボディを sym_body_bytes に追記するヘルパー ---
//
// 使い方:
//  ```
//  let mut builder = BodyBuilder::new();
//  let offset = builder.push(fn_data);  // DiskBodyOffset を返す
//  // ... 全シンボルを push した後
//  let body_bytes = builder.finish();
//  ```
struct BodyBuilder {
    buf: Vec<u8>,
}

impl BodyBuilder {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// ボディをエンコードして追記し、そのオフセット (body_size プレフィックスの位置) を返す。
    fn push(&mut self, body: &impl DiskEncode) -> DiskBodyOffset {
        let offset = DiskBodyOffset(self.buf.len() as u32);

        // body bytes を一旦別バッファにエンコードしてサイズを確定させてから追記
        let mut body_buf = Vec::new();
        body.encode(&mut body_buf);

        (body_buf.len() as u32).encode(&mut self.buf); // body_size prefix
        self.buf.extend_from_slice(&body_buf);

        offset
    }

    fn finish(self) -> Vec<u8> {
        self.buf
    }
}
