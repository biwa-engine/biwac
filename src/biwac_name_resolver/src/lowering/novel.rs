use biwac_ast::symbols::novel::{NovelBlockStmt, NovelContent, NovelStmt};
use biwac_hir::{
    AssignStmt, BlockStmt, Callee, DecledVar, Expr, ExprStmt, FnBody, FnCall, Ident, Literal,
    NovelSceneDef, NovelSyscallStmt, Primary, ReturnStmt, Stmt, Ty, TyKind, ValDefKind, VarDecl,
    VarIdKind, Variable,
};
use biwac_lang_item::{LangItem, LangItemTable};
use biwac_span::{Span, ValDefId, VarId};

use crate::ResolveError;

use super::{
    expressions::{ExprLowerCtx, lower_expr, lower_primary},
    ty_from_typ_repr,
};

pub(super) fn lower_novel_scene(
    scene_def: &biwac_ast::NovelScene,
    lang_items: &LangItemTable,
    errors: &mut Vec<ResolveError>,
) -> (ValDefId, ValDefKind) {
    let val_def_id = *scene_def
        .def_id
        .get()
        .expect("compiler bug: def_id not assigned before lowering");

    let signature = super::globals::build_fn_signature(
        &scene_def.args,
        &scene_def.rtype,
        None,
        false,
        &None,
        Vec::new(),
        scene_def.span.clone(),
    );

    let body = build_novel_body(
        &scene_def.args,
        &scene_def.stmts,
        &signature,
        lang_items,
        errors,
    );

    (
        val_def_id,
        ValDefKind::NovelScene(Box::new(NovelSceneDef::new(
            scene_def.id.clone().into(),
            signature,
            body,
        ))),
    )
}

fn build_novel_body(
    args: &biwac_ast::ArgDeclList,
    stmts: &[NovelStmt],
    signature: &biwac_hir::FnSignature,
    lang_items: &LangItemTable,
    errors: &mut Vec<ResolveError>,
) -> FnBody {
    let mut ctx = ExprLowerCtx::new();

    // novel statement の展開先は game を第 1 引数に取る。
    //
    // scene は「lang item `game` のみを引数に取り `game` を返す」という規約
    // (`biwac_scene`) があるので、唯一の引数をそのまま渡せばよい。
    // その検査は lowering の後に走るため、ここでは無い場合もありうる。
    // 無ければ novel statement を落とす。規約違反自体は scene の検査が報告する。
    let game_var = args.args.first().and_then(|a| a.var_id.get().copied());
    let novel = NovelCtx {
        game_var,
        lang_items,
    };

    for (arg, sarg) in args.args.iter().zip(signature.args.iter()) {
        let ty = sarg.ty.clone();
        ctx.declare_var(
            *arg.var_id.get().unwrap(),
            biwac_hir::DecledVar {
                id: Ident::from(arg.id.clone()),
                ty,
            },
        );
    }

    let lowered_stmts = stmts
        .iter()
        .filter_map(|s| lower_novel_stmt(&mut ctx, &novel, s, errors))
        .collect();

    FnBody {
        stmts: lowered_stmts,
        expr: None,
        self_var_id: None,
        vars: ctx.into_vars(),
    }
}

/// novel statement を展開するのに要るもの。
struct NovelCtx<'a> {
    /// scene の唯一の引数 (game)。規約違反なら `None`。
    game_var: Option<VarId>,
    lang_items: &'a LangItemTable,
}

fn lower_novel_stmt(
    ctx: &mut ExprLowerCtx,
    novel: &NovelCtx<'_>,
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
            let then = lower_novel_block_stmt(ctx, novel, &if_stmt.then, errors);
            let els = if_stmt
                .els
                .as_ref()
                .map(|e| lower_novel_block_stmt(ctx, novel, e, errors));
            Some(Stmt::If(biwac_hir::IfStmt { cond, then, els }))
        }

        // 生テキストも埋め込み式も `content_push(game, content)` になる。
        NovelStmt::ContentPush(content) => {
            let span = content.span().clone();
            let arg = match content {
                NovelContent::Text { text, span } => ctx.expr(Primary::Literal(Literal::String(
                    biwac_ast::StringLiteral {
                        val: text.clone(),
                        span: span.clone(),
                    },
                ))),
                NovelContent::Expr { expr, .. } => lower_expr(ctx, expr, errors)?,
            };

            novel.syscall(ctx, LangItem::ContentPush, vec![arg], span)
        }

        // `>>` は `content_flush_and_wait(game)` になる。
        NovelStmt::ContentFlushAndWait(flush) => novel.syscall(
            ctx,
            LangItem::ContentFlushAndWait,
            Vec::new(),
            flush.span.clone(),
        ),

        NovelStmt::NovelEndScene(end) => {
            let expr = lower_expr(ctx, &end.expr, errors)?;
            Some(Stmt::Return(ReturnStmt {
                expr,
                span: end.span.clone(),
            }))
        }
    }
}

impl NovelCtx<'_> {
    /// `<lang item>(game, ..args)` を組み立てる。
    ///
    /// 呼び出し式そのものを HIR に置くので、
    /// 型推論・MIR 構築・コード生成は普通の呼び出しとして扱える。
    /// 制限つきジェネリクス (`content_push[C: Into[Content], ..]`) の検査も
    /// そのまま働く。
    fn syscall(
        &self,
        ctx: &mut ExprLowerCtx,
        item: LangItem,
        mut args: Vec<Expr>,
        span: Span,
    ) -> Option<Stmt> {
        let def_id = ValDefId::new(self.lang_items.get(&item)?);
        let game_var = self.game_var?;

        let game = ctx.expr(Primary::Variable(Variable {
            id: VarIdKind::Local(game_var),
            span: span.clone(),
        }));
        args.insert(0, game);

        let call = ctx.expr(Primary::FnCall(FnCall {
            callee: Callee::Fn(def_id),
            args,
            span: span.clone(),
        }));

        Some(Stmt::NovelSyscall(NovelSyscallStmt { call, span }))
    }
}

fn lower_novel_block_stmt(
    ctx: &mut ExprLowerCtx,
    novel: &NovelCtx<'_>,
    block: &NovelBlockStmt,
    errors: &mut Vec<ResolveError>,
) -> BlockStmt {
    let stmts = block
        .stmts
        .iter()
        .filter_map(|s| lower_novel_stmt(ctx, novel, s, errors))
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
