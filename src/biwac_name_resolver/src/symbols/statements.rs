use biwac_hir::{
    AssignStmt, BlockStmt, Expr, ExprStmt, IfStmt, InferTy, Primary, ReturnStmt, Stmt, Ty, TyKind,
    VarDecl, WhileStmt,
};
use biwac_parser::types::TypDecl;

use crate::{RsvResult, TryResolve};

impl TryResolve<&biwac_parser::Stmt> for Stmt {
    fn try_resolve<'mctx>(
        value: &biwac_parser::Stmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match &value {
            biwac_parser::Stmt::If(i) => Ok(Self::If(IfStmt::try_resolve(i, fctx, hir)?)),
            biwac_parser::Stmt::While(w) => Ok(Self::While(WhileStmt::try_resolve(w, fctx, hir)?)),
            biwac_parser::Stmt::Block(b) => {
                // ブロック文はスコープを作る
                fctx.enter_scope();

                let stmts = b
                    .stmts
                    .iter()
                    .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
                    .collect::<RsvResult<Vec<Stmt>>>()?;

                fctx.exit_scope();

                Ok(Self::Block(BlockStmt {
                    span: b.span.clone(),
                    stmts,
                }))
            }
            biwac_parser::Stmt::Expr(expr) => Ok(Self::Expr(ExprStmt {
                span: expr.span.clone(),
                expr: Expr::try_resolve(&expr.expr, fctx, hir)?,
            })),
            biwac_parser::Stmt::Return(expr) => Ok(Self::Return(ReturnStmt {
                span: expr.span.clone(),
                expr: Expr::try_resolve(&expr.expr, fctx, hir)?,
            })),
            biwac_parser::Stmt::VarDecl(var) => {
                Ok(Self::VarDecl(VarDecl::try_resolve(var, fctx, hir)?))
            }
            biwac_parser::Stmt::Assign(assign) => Ok(Self::Assign(AssignStmt {
                span: assign.span.clone(),
                dst: Primary::try_resolve(&assign.dst, fctx, hir)?,
                src: Expr::try_resolve(&assign.src, fctx, hir)?,
            })),
        }
    }
}

impl TryResolve<&biwac_parser::IfStmt> for IfStmt {
    fn try_resolve<'mctx>(
        value: &biwac_parser::IfStmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            cond: Expr::try_resolve(&value.cond, fctx, hir)?,
            then: BlockStmt {
                stmts: value
                    .then
                    .stmts
                    .iter()
                    .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
                    .collect::<RsvResult<Vec<Stmt>>>()?,
                span: value.then.span.clone(),
            },
            els: match &value.els {
                Some(els) => Some(BlockStmt {
                    stmts: els
                        .stmts
                        .iter()
                        .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
                        .collect::<RsvResult<_>>()?,
                    span: els.span.clone(),
                }),
                None => None,
            },
        })
    }
}

impl TryResolve<&biwac_parser::WhileStmt> for WhileStmt {
    fn try_resolve<'mctx>(
        value: &biwac_parser::WhileStmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            cond: Expr::try_resolve(&value.cond, fctx, hir)?,
            stmts: BlockStmt {
                stmts: value
                    .stmts
                    .stmts
                    .iter()
                    .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
                    .collect::<RsvResult<Vec<Stmt>>>()?,
                span: value.span.clone(),
            },
        })
    }
}

impl TryResolve<&biwac_parser::VarDecl> for VarDecl {
    fn try_resolve<'mctx>(
        value: &biwac_parser::VarDecl,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        // 変数の宣言をcontextに登録
        let ty = match &value.typ {
            TypDecl::Typ(typ) => fctx.try_resolve_ty(typ, hir)?,
            TypDecl::Any => Ty::new(TyKind::Infer(InferTy::Unknown), value.id.span.clone()), // 型が不明で推論を要する
        };
        let id = fctx.declare_variable(&value.id, ty)?;

        Ok(Self {
            id,
            init: Expr::try_resolve(&value.init, fctx, hir)?,
        })
    }
}
