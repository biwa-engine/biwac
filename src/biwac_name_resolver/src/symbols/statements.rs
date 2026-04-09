use biwac_ast::TypDecl;
use biwac_hir::{
    AssignStmt, BlockStmt, Expr, ExprStmt, IfStmt, InferTy, Primary, ReturnStmt, Stmt, Ty, TyKind,
    VarDecl, WhileStmt,
};

use crate::{RsvResult, TryResolve};

impl TryResolve<&biwac_ast::Stmt> for Stmt {
    fn try_resolve<'mctx>(
        value: &biwac_ast::Stmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match &value {
            biwac_ast::Stmt::If(i) => Ok(Self::If(IfStmt::try_resolve(i, fctx, hir)?)),
            biwac_ast::Stmt::While(w) => Ok(Self::While(WhileStmt::try_resolve(w, fctx, hir)?)),
            biwac_ast::Stmt::Block(b) => Ok(Self::Block(BlockStmt::try_resolve(b, fctx, hir)?)),
            biwac_ast::Stmt::Expr(expr) => Ok(Self::Expr(ExprStmt {
                span: expr.span.clone(),
                expr: Expr::try_resolve(&expr.expr, fctx, hir)?,
            })),
            biwac_ast::Stmt::Return(expr) => Ok(Self::Return(ReturnStmt {
                span: expr.span.clone(),
                expr: Expr::try_resolve(&expr.expr, fctx, hir)?,
            })),
            biwac_ast::Stmt::VarDecl(var) => {
                Ok(Self::VarDecl(VarDecl::try_resolve(var, fctx, hir)?))
            }
            biwac_ast::Stmt::Assign(assign) => Ok(Self::Assign(AssignStmt {
                span: assign.span.clone(),
                dst: Primary::try_resolve(&assign.dst, fctx, hir)?,
                src: Expr::try_resolve(&assign.src, fctx, hir)?,
            })),
        }
    }
}

impl TryResolve<&biwac_ast::BlockStmt> for BlockStmt {
    fn try_resolve<'mctx>(
        value: &biwac_ast::BlockStmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        // ブロック文はスコープを作る
        fctx.enter_scope();

        let stmts = value
            .stmts
            .iter()
            .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
            .collect::<RsvResult<Vec<Stmt>>>()?;

        fctx.exit_scope();

        Ok(BlockStmt {
            span: value.span.clone(),
            stmts,
        })
    }
}

impl TryResolve<&biwac_ast::IfStmt> for IfStmt {
    fn try_resolve<'mctx>(
        value: &biwac_ast::IfStmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            cond: Expr::try_resolve(&value.cond, fctx, hir)?,
            then: BlockStmt::try_resolve(&value.then, fctx, hir)?,
            els: match &value.els {
                Some(els) => Some(BlockStmt::try_resolve(els, fctx, hir)?),
                None => None,
            },
        })
    }
}

impl TryResolve<&biwac_ast::WhileStmt> for WhileStmt {
    fn try_resolve<'mctx>(
        value: &biwac_ast::WhileStmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            cond: Expr::try_resolve(&value.cond, fctx, hir)?,
            stmts: BlockStmt::try_resolve(&value.stmts, fctx, hir)?,
        })
    }
}

impl TryResolve<&biwac_ast::VarDecl> for VarDecl {
    fn try_resolve<'mctx>(
        value: &biwac_ast::VarDecl,
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
