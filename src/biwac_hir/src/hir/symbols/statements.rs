use biwac_span::{Span, VarId};

use crate::{Expr, Ident, Pattern, Primary, Ty};

#[derive(Debug, Clone)]
pub struct DecledVar {
    pub id: Ident,
    pub ty: Ty, // if not type annotated, Ty::Infer(InferTy)
}

#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub cond: Expr,
    pub then: BlockStmt,
    pub els: Option<BlockStmt>,
}

/// `match` の文形。アームの本体はブロック文である。
#[derive(Debug, Clone)]
pub struct MatchStmt {
    pub scrutinee: Expr,
    pub arms: Vec<MatchStmtArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchStmtArm {
    pub pattern: Pattern,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub cond: Expr,
    pub stmts: BlockStmt,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub id: VarId,
    pub init: Expr,
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub dst: Primary,
    pub src: Expr,
    pub span: Span,
}

// novel-specific statements

#[derive(Debug, Clone)]
pub struct NovelWriteStmt {
    pub msg: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct NovelWaitStmt {
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Block(BlockStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    If(IfStmt),
    Match(MatchStmt),
    While(WhileStmt),
    VarDecl(VarDecl),
    Assign(AssignStmt),
    NovelWrite(NovelWriteStmt),
    NovelWait(NovelWaitStmt),
}
