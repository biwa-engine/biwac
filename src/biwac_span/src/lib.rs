mod def_path;
mod ident;
mod package_hash;

use biwac_base::ModId;

pub use def_path::{DefPath, DefPathHash, DefPathSegment};
pub use ident::{IdentInterner, InternedIdent};
pub use package_hash::PackageHashId;

/// `struct Span` represents span of any form of symbols,
/// such as lexer token, AST node, and other IR node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    file: ModId,

    /// UTF-8 byte index
    /// can be used in &str slice as `&src[span.begin..span.end]`
    begin: usize,
    end: usize,
}

impl Span {
    pub fn new(file: ModId, begin: usize, end: usize) -> Self {
        Self { file, begin, end }
    }

    pub fn merge(begin: &Self, end: &Self) -> Self {
        Self {
            file: begin.file,
            begin: begin.begin,
            end: end.end,
        }
    }

    pub fn module(&self) -> ModId {
        self.file
    }

    pub fn begin(&self) -> usize {
        self.begin
    }

    pub fn end(&self) -> usize {
        self.end
    }
}
