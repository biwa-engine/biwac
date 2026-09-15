use crate::{
    ResolveErrorHandler,
    resolving::{LocalNameResolve, context::LocalResolveCtx},
};

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::NovelStmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        match self {
            biwac_ast::NovelStmt::If(if_stmt) => if_stmt.resolve(ctx),
            biwac_ast::NovelStmt::Expr(expr) => expr.expr.resolve(ctx),
            biwac_ast::NovelStmt::VarDecl(var_decl) => var_decl.resolve(ctx),
            biwac_ast::NovelStmt::Assign(assign) => assign.resolve(ctx),
            biwac_ast::NovelStmt::ContentPush(content) => content.resolve(ctx),
            biwac_ast::NovelStmt::ContentFlushAndWait(_) => Ok(()),
            biwac_ast::NovelStmt::NovelEndScene(end) => end.expr.resolve(ctx),
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::NovelContent {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        match self {
            // 生テキストには解決するものが無い。
            biwac_ast::NovelContent::Text { .. } => Ok(()),
            biwac_ast::NovelContent::Expr { expr, .. } => expr.resolve(ctx),
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::NovelIfStmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        self.cond.resolve(ctx).handle(&mut errors);
        self.then.resolve(ctx).handle(&mut errors);
        if let Some(els) = &self.els {
            els.resolve(ctx).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::NovelBlockStmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        ctx.inner_scope(|ctx: &mut C| {
            let mut errors = Vec::new();

            for stmt in &self.stmts {
                stmt.resolve(ctx).handle(&mut errors);
            }

            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        })
    }
}
