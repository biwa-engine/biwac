use biwac_base::Span;
use biwac_parser::types::TypDecl;

use crate::{Expr, LocVarId, Primary, RsvResult, TryResolve, Typ};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStmt {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub cond: Expr,
    pub then: BlockStmt,
    pub els: Option<BlockStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStmt {
    pub cond: Expr,
    pub stmts: BlockStmt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarDecl {
    pub id: LocVarId,
    pub init: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStmt {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignStmt {
    pub dst: Primary,
    pub src: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
            biwac_parser::Stmt::Block(b) => {
                // ブロック文はスコープを作る
                fctx.enter_scope();

                let stmts = b
                    .stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, fctx))
                    .collect::<RsvResult<Vec<Stmt>>>()?;

                fctx.exit_scope();

                Ok(Self::Block(BlockStmt {
                    span: b.span,
                    stmts,
                }))
            }
            biwac_parser::Stmt::Expr(expr) => Ok(Self::Expr(ExprStmt {
                span: expr.span,
                expr: Expr::try_resolve(expr.expr, fctx)?,
            })),
            biwac_parser::Stmt::Return(expr) => Ok(Self::Return(ReturnStmt {
                span: expr.span,
                expr: Expr::try_resolve(expr.expr, fctx)?,
            })),
            biwac_parser::Stmt::VarDecl(var) => Ok(Self::VarDecl(VarDecl::try_resolve(var, fctx)?)),
            biwac_parser::Stmt::Assign(assign) => Ok(Self::Assign(AssignStmt {
                span: assign.span,
                dst: Primary::try_resolve(assign.dst, fctx)?,
                src: Expr::try_resolve(assign.src, fctx)?,
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
            cond: Expr::try_resolve(value.cond, fctx)?,
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
            cond: Expr::try_resolve(value.cond, fctx)?,
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
        // 変数の宣言をcontextに登録
        let typ = match value.typ {
            TypDecl::Typ(typ) => Some(Typ::try_resolve(&typ, fctx)?),
            TypDecl::Any => None,
        };
        let id = fctx.declare_variable(&value.id, typ)?;

        Ok(Self {
            id,
            init: Expr::try_resolve(value.init, fctx)?,
        })
    }
}
