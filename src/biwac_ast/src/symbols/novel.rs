use biwac_span::Span;

use crate::{AssignStmt, ExprStmt, Exprs, VarDecl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NovelStmt {
    Expr(ExprStmt),
    If(NovelIfStmt),
    VarDecl(VarDecl),
    Assign(AssignStmt),
    /// Message Window に積む内容。生テキストか `$` の埋め込み式。
    ContentPush(NovelContent),
    /// `>>`。積んだ内容をまとめて出し、クリックを待つ。
    ContentFlushAndWait(NovelFlush),
    NovelEndScene(NovelEndSceneStmt),
}

/// Message Window に積む 1 つの内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NovelContent {
    /// 行に直接書かれたテキスト。
    Text { text: String, span: Span },
    /// `$...` の埋め込み式。
    Expr { expr: Exprs, span: Span },
}

impl NovelContent {
    pub fn span(&self) -> &Span {
        match self {
            Self::Text { span, .. } | Self::Expr { span, .. } => span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelFlush {
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelIfStmt {
    pub cond: Exprs,
    pub then: NovelBlockStmt,
    pub els: Option<NovelBlockStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelBlockStmt {
    pub stmts: Vec<NovelStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelEndSceneStmt {
    pub expr: Exprs,
    pub span: Span,
}
