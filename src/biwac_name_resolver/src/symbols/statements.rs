use biwac_base::Span;
use biwac_parser::{Ident, types::TypDecl};

use crate::{
    Exprs,
    Primary,
    RsvResult,
    TryResolve,
    Typ, // resolver::{ResolveError, TryResolve},
};

#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub cond: Exprs,
    pub then: BlockStmt,
    pub els: Option<BlockStmt>,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub cond: Exprs,
    pub stmts: BlockStmt,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub typ: Option<Typ>,
    pub id: Ident,
    pub init: Exprs,
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub dst: Primary,
    pub src: Exprs,
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

impl TryResolve<biwac_parser::Stmt> for Stmt {
    fn try_resolve<'mctx>(
        value: biwac_parser::Stmt,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            biwac_parser::Stmt::If(i) => Ok(Self::If(IfStmt::try_resolve(i, fctx)?)),
            biwac_parser::Stmt::While(w) => Ok(Self::While(WhileStmt::try_resolve(w, fctx)?)),
            biwac_parser::Stmt::Block(b) => Ok(Self::Block(BlockStmt {
                span: b.span,
                stmts: b
                    .stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, fctx))
                    .collect::<RsvResult<Vec<Stmt>>>()?,
            })),
            biwac_parser::Stmt::Expr(expr) => Ok(Self::Expr(ExprStmt {
                span: expr.span,
                expr: Exprs::try_resolve(expr.expr, fctx)?,
            })),
            biwac_parser::Stmt::Return(expr) => Ok(Self::Return(ReturnStmt {
                span: expr.span,
                expr: Exprs::try_resolve(expr.expr, fctx)?,
            })),
            biwac_parser::Stmt::VarDecl(var) => Ok(Self::VarDecl(VarDecl::try_resolve(var, fctx)?)),
            biwac_parser::Stmt::Assign(assign) => Ok(Self::Assign(AssignStmt {
                span: assign.span,
                dst: Primary::try_resolve(assign.dst, fctx)?,
                src: Exprs::try_resolve(assign.src, fctx)?,
            })),
        }
    }
}

impl TryResolve<biwac_parser::IfStmt> for IfStmt {
    fn try_resolve<'mctx>(
        value: biwac_parser::IfStmt,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        Ok(Self {
            cond: Exprs::try_resolve(value.cond, fctx)?,
            then: BlockStmt {
                stmts: value
                    .then
                    .stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, fctx))
                    .collect::<RsvResult<Vec<Stmt>>>()?,
                span: value.then.span,
            },
            els: match value.els {
                Some(els) => Some(BlockStmt {
                    stmts: els
                        .stmts
                        .into_iter()
                        .map(|stmt| Stmt::try_resolve(stmt, fctx))
                        .collect::<RsvResult<_>>()?,
                    span: els.span,
                }),
                None => None,
            },
        })
    }
}

impl TryResolve<biwac_parser::WhileStmt> for WhileStmt {
    fn try_resolve<'mctx>(
        value: biwac_parser::WhileStmt,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        Ok(Self {
            cond: Exprs::try_resolve(value.cond, fctx)?,
            stmts: BlockStmt {
                stmts: value
                    .stmts
                    .stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, fctx))
                    .collect::<RsvResult<Vec<Stmt>>>()?,
                span: value.span,
            },
        })
    }
}

impl TryResolve<biwac_parser::VarDecl> for VarDecl {
    fn try_resolve<'mctx>(
        value: biwac_parser::VarDecl,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        // TODO:
        // LocVarIdを発行し、
        // fctxの現在の変数スコープに名前 -> LocVarIdのマップを保存
        // fctxの現在の変数スコープに同じ名前があればエラー
        //  -> そのエラーを上手く出すため、 LocVarId -> Identのマップも持っていたほうが良い
        Ok(Self {
            typ: match value.typ {
                TypDecl::Typ(typ) => Some(Typ::try_resolve(typ, fctx)?),
                TypDecl::Any => None,
            },
            id: value.id,
            init: Exprs::try_resolve(value.init, fctx)?,
        })
    }
}
