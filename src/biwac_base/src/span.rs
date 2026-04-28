use crate::{BIWA_EXTENSION, ModId, PackageName};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModPath {
    Main,
    Lib,
    Mod(Vec<String>),
}

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

/// SSpan
/// Semi span
/// Span has position (line index and character index range).
/// But external package symbols do not have position information,
/// only have a package name and a module path.
/// So, Semi span provides both probabilities with enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SSpan {
    Span { span: Span },
    External { pkg: PackageName, modu: ModPath },
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

impl ModPath {
    pub fn file_name(&self) -> String {
        match self {
            Self::Main => format!("main.{BIWA_EXTENSION}"),
            Self::Lib => format!("lib.{BIWA_EXTENSION}"),
            Self::Mod(path) => format!("{}.{BIWA_EXTENSION}", path.join("/")),
        }
    }

    pub fn push(self, child: String) -> Self {
        match self {
            Self::Main => Self::Mod([child].into()),
            Self::Lib => Self::Mod([child].into()),
            Self::Mod(mut m) => {
                m.push(child);

                Self::Mod(m)
            }
        }
    }

    pub fn extend(self, path: Vec<String>) -> Self {
        match self {
            Self::Main => Self::Mod(path),
            Self::Lib => Self::Mod(path),
            Self::Mod(mut m) => {
                m.extend(path);

                Self::Mod(m)
            }
        }
    }
}

impl From<ModPath> for Vec<String> {
    fn from(value: ModPath) -> Self {
        match value {
            ModPath::Main => vec![],
            ModPath::Lib => vec![],
            ModPath::Mod(m) => m,
        }
    }
}

impl From<Span> for SSpan {
    fn from(value: Span) -> Self {
        Self::Span { span: value }
    }
}
