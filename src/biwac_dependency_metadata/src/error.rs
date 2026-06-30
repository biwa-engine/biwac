#[derive(Debug)]
pub enum DepMetadataError {
    UnexpectedEnd { needed: usize, available: usize },
    InvalidMagic,
    InvalidVersion { got: u32 },
    UnknownSymbolKind(u32),
    UnknownVisibility(u32),
    UnknownTyKind(u32),
    StringOffsetOutOfBounds { offset: u32, table_len: u32 },
    NulTerminatorNotFound { offset: u32 },
    InvalidUtf8,
    SymbolIndexOutOfBounds { index: u32, sym_count: u32 },
    FileIndexOutOfBounds { index: u32, file_count: u32 },
    BodySizeMismatch { declared: u32, available: usize },
}

impl std::fmt::Display for DepMetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEnd { needed, available } => {
                write!(f, "unexpected end of data: needed {needed} bytes, got {available}")
            }
            Self::InvalidMagic => write!(f, "invalid magic bytes (not a .biwameta file)"),
            Self::InvalidVersion { got } => {
                write!(f, "unsupported .biwameta format version: {got}")
            }
            Self::UnknownSymbolKind(v) => write!(f, "unknown symbol kind: {v}"),
            Self::UnknownVisibility(v) => write!(f, "unknown visibility: {v}"),
            Self::UnknownTyKind(v) => write!(f, "unknown type kind: {v}"),
            Self::StringOffsetOutOfBounds { offset, table_len } => {
                write!(f, "string offset {offset} out of bounds (table len: {table_len})")
            }
            Self::NulTerminatorNotFound { offset } => {
                write!(f, "nul terminator not found for string at offset {offset}")
            }
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in string table"),
            Self::SymbolIndexOutOfBounds { index, sym_count } => {
                write!(f, "symbol index {index} out of bounds (sym count: {sym_count})")
            }
            Self::FileIndexOutOfBounds { index, file_count } => {
                write!(f, "file index {index} out of bounds (file count: {file_count})")
            }
            Self::BodySizeMismatch { declared, available } => {
                write!(f, "body size {declared} exceeds available bytes {available}")
            }
        }
    }
}
