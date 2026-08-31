use std::collections::HashMap;

use crate::{ModPath, PackageId};

#[derive(Debug, Default)]
pub struct SourceHolder {
    pub mods: HashMap<ModId, ModSource>,
}

/// [`ModId`] is global scope (inter-package) module id.
///
/// Encoding (u64):
///   - Self-package module:   `(0u64 << 32) | sequential_id`  (high 32 bits = 0)
///   - External package module: `(pkg_id as u64) << 32 | module_sym_idx as u64`
///     where `module_sym_idx` is the symbol index in the external package's .biwameta.
///
/// Self-package IDs are assigned sequentially from 0 and fit in the low 32 bits.
/// External packages always have `pkg_id >= 1`, so there is no collision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModId(u64);

#[derive(Debug)]
pub struct ModSource {
    pub pkg_id: PackageId,
    pub modu: ModPath,
    pub src: String,
}

impl ModId {
    /// Create a self-package module id from a sequential index.
    #[inline]
    pub fn new_in_self(id: u32) -> Self {
        Self(id as u64)
    }

    /// Create an external-package module id from a `PackageId` and the module's symbol index.
    #[inline]
    pub fn new_ext(pkg_id: u32, module_sym_idx: u32) -> Self {
        Self((pkg_id as u64) << 32 | module_sym_idx as u64)
    }

    /// Returns true if this ModId belongs to the self-package (high 32 bits == 0).
    #[inline]
    pub fn is_self_pkg(&self) -> bool {
        (self.0 >> 32) == 0
    }

    /// The PackageId encoded in the high 32 bits (0 for self-package).
    #[inline]
    pub fn pkg_id_bits(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// The symbol index encoded in the low 32 bits.
    /// For self-package this equals the sequential id; for external packages it is the module_sym_idx.
    #[inline]
    pub fn sym_idx(&self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }
}

impl ModSource {
    /// バイトオフセットを文字数 (char) ベースのオフセットに変換する。
    ///
    /// [`biwac_span::Span`] はソース文字列のバイトオフセットを保持しているが、
    /// 診断表示に使う ariadne の [`ariadne::Source`] は文字数ベースの
    /// オフセットを期待している。
    /// そのまま渡すと、マルチバイト文字を含む行より後ろの位置が
    /// バイト数と文字数の差だけ後方にずれてしまう。
    ///
    /// 文字境界でないオフセットを渡されても panic しないように、
    /// 開始位置が `byte_offset` より前にある文字を数える形で実装している。
    pub fn char_offset(&self, byte_offset: usize) -> usize {
        self.src
            .char_indices()
            .take_while(|(i, _)| *i < byte_offset)
            .count()
    }
}
