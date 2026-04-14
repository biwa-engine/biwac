use crate::{ModId, PackageName};

const BIWA_SRC_EXT: &str = "biwa";

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
    begin: usize,
    end: usize,
}

/// `struct Pos` represents position in raw source codes.
/// NOTE: `line: usize` is 0 indexed,
///     so when we show an error message, we have to add 1 to show real line number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pos {
    line: usize,
    idx: usize,
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

impl Pos {
    pub fn new(line: usize, idx: usize) -> Self {
        Self { line, idx }
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn idx(&self) -> usize {
        self.idx
    }
}

impl ModPath {
    pub fn file_name(&self) -> String {
        match self {
            Self::Main => format!("main.{BIWA_SRC_EXT}"),
            Self::Lib => format!("lib.{BIWA_SRC_EXT}"),
            Self::Mod(path) => format!("{}.biwa", path.join("/")),
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
