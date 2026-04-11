use std::str::FromStr;

use biwac_ast::StringLiteral;
use biwac_base::PackageName;
use biwac_hir::{
    AssignStmt, BlockStmt, Callee, Expr, ExprStmt, ExprVal, FnCall, IfStmt, Literal, PkgId,
    Primary, ReturnStmt, Stmt, ValId, VarDecl,
};

use crate::{RsvResult, TryResolve};

impl TryResolve<&biwac_ast::NovelStmt> for Stmt {
    fn try_resolve<'mctx>(
        value: &biwac_ast::NovelStmt,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match &value {
            biwac_ast::NovelStmt::If(i) => Ok(Self::If(IfStmt::try_resolve(i, fctx, hir)?)),
            biwac_ast::NovelStmt::Expr(expr) => Ok(Self::Expr(ExprStmt {
                span: expr.span.clone(),
                expr: Expr::try_resolve(&expr.expr, fctx, hir)?,
            })),
            biwac_ast::NovelStmt::VarDecl(var) => {
                Ok(Self::VarDecl(VarDecl::try_resolve(var, fctx, hir)?))
            }
            biwac_ast::NovelStmt::Assign(assign) => Ok(Self::Assign(AssignStmt {
                span: assign.span.clone(),
                dst: Primary::try_resolve(&assign.dst, fctx, hir)?,
                src: Expr::try_resolve(&assign.src, fctx, hir)?,
            })),
            biwac_ast::NovelStmt::NovelWrite(msg) => Ok(Self::Expr(ExprStmt {
                span: msg.span.clone(),

                // NOTE:
                // std::game::base_engine::write("message")
                // に解決される
                expr: Expr {
                    expr: ExprVal::Primary(Primary::FnCall(FnCall {
                        callee: Callee::Fn(ValId::new(
                            PkgId::External(PackageName::from_str("std").unwrap()),
                            vec!["game".into(), "base_engine".into()],
                            "write".into(),
                        )),
                        args: vec![Expr {
                            expr: ExprVal::Primary(Primary::Literal(Literal::String(
                                StringLiteral {
                                    val: msg.msg.clone(),
                                    span: msg.span.clone(),
                                },
                            ))),
                            id: fctx.new_expr_id(),
                        }],
                        span: msg.span.clone(),
                    })),
                    id: fctx.new_expr_id(),
                },
            })),
            biwac_ast::NovelStmt::NovelWait(wait) => Ok(Self::Expr(ExprStmt {
                span: wait.span.clone(),

                // NOTE:
                // std::game::base_engine::wait()
                // に解決される
                expr: Expr {
                    expr: ExprVal::Primary(Primary::FnCall(FnCall {
                        callee: Callee::Fn(ValId::new(
                            PkgId::External(PackageName::from_str("std").unwrap()),
                            vec!["game".into(), "base_engine".into()],
                            "wait".into(),
                        )),
                        args: Vec::new(),
                        span: wait.span.clone(),
                    })),
                    id: fctx.new_expr_id(),
                },
            })),
            biwac_ast::NovelStmt::NovelEndScene(end) => Ok(Self::Return(ReturnStmt {
                expr: Expr::try_resolve(&end.expr, fctx, hir)?,
                span: end.span.clone(),
            })),
        }
    }
}

impl TryResolve<&biwac_ast::NovelIfStmt> for IfStmt {
    fn try_resolve<'mctx>(
        value: &biwac_ast::NovelIfStmt,
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

impl TryResolve<&biwac_ast::NovelBlockStmt> for BlockStmt {
    fn try_resolve<'mctx>(
        value: &biwac_ast::NovelBlockStmt,
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
