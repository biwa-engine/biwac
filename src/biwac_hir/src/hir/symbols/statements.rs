use biwac_span::Span;

use crate::{Expr, Ident, Primary, Ty};

// LocVarId
// function local variable id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocVarId(usize);

impl LocVarId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn value(&self) -> &usize {
        &self.0
    }
}

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

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub cond: Expr,
    pub stmts: BlockStmt,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub id: LocVarId,
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

#[derive(Debug, Clone)]
pub enum Stmt {
    Block(BlockStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    If(IfStmt),
    While(WhileStmt),
    VarDecl(VarDecl),
    Assign(AssignStmt),
}
