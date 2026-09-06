use super::{
    DiskEncode,
    codec::DiskDecode,
    format::{
        DiskEnumData, DiskFnData, DiskModData, DiskNativeTypeAliasData, DiskStructData,
        DiskSymbolKind, DiskVariantData,
    },
};
use crate::error::DepMetadataError;

/// デコード済みシンボルボディ。
/// LazyDiskVec のキャッシュエントリとして保持される。
#[derive(Debug)]
pub enum SymbolBody {
    Mod(DiskModData),
    Struct(DiskStructData),
    Fn(DiskFnData),
    NativeTypeAlias(DiskNativeTypeAliasData),
    Enum(DiskEnumData),
    Variant(DiskVariantData),
}

impl SymbolBody {
    /// sym_body_data の body bytes を kind に従ってデコードする。
    pub(crate) fn decode(kind: u32, bytes: &[u8]) -> Result<Self, DepMetadataError> {
        match DiskSymbolKind::try_from(kind)? {
            DiskSymbolKind::Mod => {
                let (data, _) = DiskModData::decode(bytes)?;
                Ok(SymbolBody::Mod(data))
            }
            DiskSymbolKind::Struct => {
                let (data, _) = DiskStructData::decode(bytes)?;
                Ok(SymbolBody::Struct(data))
            }
            DiskSymbolKind::Fn => {
                let (data, _) = DiskFnData::decode(bytes)?;
                Ok(SymbolBody::Fn(data))
            }
            DiskSymbolKind::NativeTypeAlias => {
                let (data, _) = DiskNativeTypeAliasData::decode(bytes)?;
                Ok(SymbolBody::NativeTypeAlias(data))
            }
            DiskSymbolKind::Enum => {
                let (data, _) = DiskEnumData::decode(bytes)?;
                Ok(SymbolBody::Enum(data))
            }
            DiskSymbolKind::Variant => {
                let (data, _) = DiskVariantData::decode(bytes)?;
                Ok(SymbolBody::Variant(data))
            }
        }
    }
}

impl DiskEncode for SymbolBody {
    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Self::Mod(module) => module.encode(buf),
            Self::Struct(struct_) => struct_.encode(buf),
            Self::Fn(fn_) => fn_.encode(buf),
            Self::NativeTypeAlias(alias) => alias.encode(buf),
            Self::Enum(enum_) => enum_.encode(buf),
            Self::Variant(variant) => variant.encode(buf),
        }
    }
}
