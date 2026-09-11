use biwac_span::Span;

use crate::{AssignStmt, ExprStmt, Exprs, VarDecl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NovelStmt {
    Expr(ExprStmt),
    If(NovelIfStmt),
    VarDecl(VarDecl),
    Assign(AssignStmt),
    NovelWrite(NovelMessage),
    /// `$...` の埋め込み式。
    ///
    /// 生テキストと同じく Message Window への出力になるが、
    /// 出す値が式で決まる点だけが違う。
    NovelWriteExpr(NovelExprMessage),
    NovelWait(NovelWait),
    NovelEndScene(NovelEndSceneStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelMessage {
    pub msg: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelExprMessage {
    pub expr: Exprs,
    /// `$` から式の終わりまで。
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelWait {
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
