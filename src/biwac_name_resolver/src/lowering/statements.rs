use biwac_hir::{
    AssignStmt, BlockStmt, DecledVar, ExprStmt, Ident, IfStmt, MatchStmt, MatchStmtArm, ReturnStmt,
    Stmt, Ty, TyKind, VarDecl, WhileStmt,
};

use crate::ResolveError;

use super::{
    expressions::{ExprLowerCtx, lower_expr, lower_primary},
    patterns::lower_pattern,
    ty_from_typ_repr,
};

pub(crate) fn lower_stmt(
    ctx: &mut ExprLowerCtx,
    stmt: &biwac_ast::Stmt,
    errors: &mut Vec<ResolveError>,
) -> Option<Stmt> {
    match stmt {
        biwac_ast::Stmt::Block(block) => {
            let stmts = block
                .stmts
                .iter()
                .filter_map(|s| lower_stmt(ctx, s, errors))
                .collect();
            Some(Stmt::Block(BlockStmt {
                stmts,
                span: block.span.clone(),
            }))
        }

        biwac_ast::Stmt::Expr(expr_stmt) => {
            let expr = lower_expr(ctx, &expr_stmt.expr, errors)?;
            Some(Stmt::Expr(ExprStmt {
                expr,
                span: expr_stmt.span.clone(),
            }))
        }

        biwac_ast::Stmt::Return(ret_stmt) => {
            let expr = lower_expr(ctx, &ret_stmt.expr, errors)?;
            Some(Stmt::Return(ReturnStmt {
                expr,
                span: ret_stmt.span.clone(),
            }))
        }

        biwac_ast::Stmt::If(if_stmt) => {
            let cond = lower_expr(ctx, &if_stmt.cond, errors)?;
            let then = lower_block_stmt(ctx, &if_stmt.then, errors);
            let els = if_stmt
                .els
                .as_ref()
                .map(|e| lower_block_stmt(ctx, e, errors));
            Some(Stmt::If(IfStmt { cond, then, els }))
        }

        biwac_ast::Stmt::Match(m) => {
            let scrutinee = lower_expr(ctx, &m.scrutinee, errors)?;
            let arms = m
                .arms
                .iter()
                .filter_map(|arm| {
                    Some(MatchStmtArm {
                        pattern: lower_pattern(&arm.pattern, errors)?,
                        body: lower_block_stmt(ctx, &arm.body, errors),
                        span: arm.span.clone(),
                    })
                })
                .collect();

            Some(Stmt::Match(MatchStmt {
                scrutinee,
                arms,
                span: m.span.clone(),
            }))
        }

        biwac_ast::Stmt::While(while_stmt) => {
            let cond = lower_expr(ctx, &while_stmt.cond, errors)?;
            let stmts = lower_block_stmt(ctx, &while_stmt.stmts, errors);
            Some(Stmt::While(WhileStmt { cond, stmts }))
        }

        biwac_ast::Stmt::VarDecl(var_decl) => {
            let var_id = match var_decl.var_id.get() {
                Some(id) => *id,
                None => return None,
            };
            let init = lower_expr(ctx, &var_decl.init, errors)?;
            let ty = lower_var_decl_ty(&var_decl.typ, &var_decl.id.span);
            ctx.declare_var(
                var_id,
                DecledVar {
                    id: Ident::from(var_decl.id.clone()),
                    ty,
                },
            );
            Some(Stmt::VarDecl(VarDecl { id: var_id, init }))
        }

        biwac_ast::Stmt::Assign(assign_stmt) => {
            let dst = lower_primary(ctx, &assign_stmt.dst, errors)?;
            let src = lower_expr(ctx, &assign_stmt.src, errors)?;
            Some(Stmt::Assign(AssignStmt {
                dst,
                src,
                span: assign_stmt.span.clone(),
            }))
        }
    }
}

pub(crate) fn lower_block_stmt(
    ctx: &mut ExprLowerCtx,
    block: &biwac_ast::BlockStmt,
    errors: &mut Vec<ResolveError>,
) -> BlockStmt {
    let stmts = block
        .stmts
        .iter()
        .filter_map(|s| lower_stmt(ctx, s, errors))
        .collect();
    BlockStmt {
        stmts,
        span: block.span.clone(),
    }
}

fn lower_var_decl_ty(typ: &biwac_ast::TypDecl, var_span: &biwac_span::Span) -> Ty {
    match typ {
        biwac_ast::TypDecl::Any => {
            Ty::new(TyKind::Infer(biwac_hir::InferTy::Unknown), var_span.clone())
        }
        biwac_ast::TypDecl::Typ(typ_repr) => ty_from_typ_repr(typ_repr, None),
    }
}
