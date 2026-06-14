use biwac_ast::symbols::novel::{NovelBlockStmt, NovelStmt};
use biwac_hir::{
    AssignStmt, BlockStmt, DecledVar, ExprStmt, FnBody, Hir, Ident, NovelSceneDef, NovelWaitStmt,
    NovelWriteStmt, ReturnStmt, Stmt, Ty, TyKind, ValDefKind, VarDecl,
};
use biwac_span::VarId;

use crate::ResolveError;

use super::{
    expressions::{ExprLowerCtx, lower_expr, lower_primary},
    ty_from_typ_repr,
};

pub(super) fn lower_novel_scene(
    hir: &mut Hir,
    scene_def: &biwac_ast::NovelScene,
    errors: &mut Vec<ResolveError>,
) {
    let val_def_id = match scene_def.def_id.get() {
        Some(id) => *id,
        None => return,
    };

    let signature = super::globals::build_fn_signature(
        &scene_def.args,
        &scene_def.rtype,
        None,
        &None,
        scene_def.span.clone(),
    );

    let body = build_novel_body(&scene_def.args, &scene_def.stmts, &signature, errors);

    hir.vals.insert(
        val_def_id,
        ValDefKind::NovelScene(Box::new(NovelSceneDef::new(
            scene_def.id.span.clone(),
            signature,
            body,
        ))),
    );
}

fn build_novel_body(
    args: &biwac_ast::ArgDeclList,
    stmts: &[NovelStmt],
    signature: &biwac_hir::FnSignature,
    errors: &mut Vec<ResolveError>,
) -> FnBody {
    let arg_var_ids: Vec<VarId> = (0..args.args.len()).map(|i| VarId::new(i as u32)).collect();

    let mut ctx = ExprLowerCtx::new();

    for (i, arg) in args.args.iter().enumerate() {
        let vid = VarId::new(i as u32);
        let ty = signature.args[i].1.clone();
        ctx.declare_var(
            vid,
            DecledVar {
                id: Ident::from(arg.id.clone()),
                ty,
            },
        );
    }

    let lowered_stmts = stmts
        .iter()
        .filter_map(|s| lower_novel_stmt(&mut ctx, s, errors))
        .collect();

    FnBody {
        stmts: lowered_stmts,
        expr: None,
        arg_var_ids,
        self_var_id: None,
        vars: ctx.into_vars(),
    }
}

fn lower_novel_stmt(
    ctx: &mut ExprLowerCtx,
    stmt: &NovelStmt,
    errors: &mut Vec<ResolveError>,
) -> Option<Stmt> {
    match stmt {
        NovelStmt::Expr(expr_stmt) => {
            let expr = lower_expr(ctx, &expr_stmt.expr, errors)?;
            Some(Stmt::Expr(ExprStmt {
                expr,
                span: expr_stmt.span.clone(),
            }))
        }

        NovelStmt::VarDecl(var_decl) => {
            let var_id = match var_decl.var_id.get() {
                Some(id) => *id,
                None => return None,
            };
            let init = lower_expr(ctx, &var_decl.init, errors)?;
            let ty = lower_novel_var_decl_ty(&var_decl.typ, &var_decl.id.span);
            ctx.declare_var(
                var_id,
                DecledVar {
                    id: Ident::from(var_decl.id.clone()),
                    ty,
                },
            );
            Some(Stmt::VarDecl(VarDecl { id: var_id, init }))
        }

        NovelStmt::Assign(assign_stmt) => {
            let dst = lower_primary(ctx, &assign_stmt.dst, errors)?;
            let src = lower_expr(ctx, &assign_stmt.src, errors)?;
            Some(Stmt::Assign(AssignStmt {
                dst,
                src,
                span: assign_stmt.span.clone(),
            }))
        }

        NovelStmt::If(if_stmt) => {
            let cond = lower_expr(ctx, &if_stmt.cond, errors)?;
            let then = lower_novel_block_stmt(ctx, &if_stmt.then, errors);
            let els = if_stmt
                .els
                .as_ref()
                .map(|e| lower_novel_block_stmt(ctx, e, errors));
            Some(Stmt::If(biwac_hir::IfStmt { cond, then, els }))
        }

        NovelStmt::NovelWrite(msg) => Some(Stmt::NovelWrite(NovelWriteStmt {
            msg: msg.msg.clone(),
            span: msg.span.clone(),
        })),

        NovelStmt::NovelWait(wait) => Some(Stmt::NovelWait(NovelWaitStmt {
            span: wait.span.clone(),
        })),

        NovelStmt::NovelEndScene(end) => {
            let expr = lower_expr(ctx, &end.expr, errors)?;
            Some(Stmt::Return(ReturnStmt {
                expr,
                span: end.span.clone(),
            }))
        }
    }
}

fn lower_novel_block_stmt(
    ctx: &mut ExprLowerCtx,
    block: &NovelBlockStmt,
    errors: &mut Vec<ResolveError>,
) -> BlockStmt {
    let stmts = block
        .stmts
        .iter()
        .filter_map(|s| lower_novel_stmt(ctx, s, errors))
        .collect();
    BlockStmt {
        stmts,
        span: block.span.clone(),
    }
}

fn lower_novel_var_decl_ty(typ: &biwac_ast::TypDecl, var_span: &biwac_span::Span) -> Ty {
    match typ {
        biwac_ast::TypDecl::Any => {
            Ty::new(TyKind::Infer(biwac_hir::InferTy::Unknown), var_span.clone())
        }
        biwac_ast::TypDecl::Typ(typ_repr) => ty_from_typ_repr(typ_repr, None),
    }
}
