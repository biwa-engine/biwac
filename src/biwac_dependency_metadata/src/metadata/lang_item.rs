use biwac_lang_item::LangItem;

use super::{
    codec::{DiskDecode, DiskEncode},
    format::DiskSymbolIndex,
};
use crate::error::DepMetadataError;

/// lang item 1 件の記録。
///
/// これがあることで、std を依存に持つパッケージのビルド時に
/// メタデータから lang item テーブルを復元できる。
///
/// `lang_item` は [`LangItem`] の discriminant であり、
/// `LangItem` の追加・並べ替えで値が変わる。
/// そのためメタデータのフォーマットバージョンと歩調を合わせる必要がある。
#[derive(Debug, Clone, Copy)]
pub struct DiskLangItem {
    pub lang_item: u32,
    pub sym_idx: DiskSymbolIndex,
}

impl DiskLangItem {
    pub const BYTE_SIZE: usize = 8;

    pub fn new(lang_item: LangItem, sym_idx: DiskSymbolIndex) -> Self {
        Self {
            lang_item: lang_item.as_u32(),
            sym_idx,
        }
    }

    /// 未知の discriminant の場合 `None`。
    /// フォーマットバージョンが一致していれば起こらない。
    pub fn lang_item(&self) -> Option<LangItem> {
        LangItem::from_u32(self.lang_item)
    }
}

impl DiskDecode for DiskLangItem {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (lang_item, n) = u32::decode(&bytes[pos..])?;
        pos += n;
        let (sym_idx, n) = DiskSymbolIndex::decode(&bytes[pos..])?;
        pos += n;
        Ok((Self { lang_item, sym_idx }, pos))
    }
}

impl DiskEncode for DiskLangItem {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.lang_item.encode(buf);
        self.sym_idx.encode(buf);
    }
}
