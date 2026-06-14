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
            biwac_ast::NovelStmt::NovelWrite(msg) => msg.resolve(ctx),
            biwac_ast::NovelStmt::NovelWait(wait) => wait.resolve(ctx),
            biwac_ast::NovelStmt::NovelEndScene(end) => end.expr.resolve(ctx),
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::NovelMessage {
    fn resolve(&self, _ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        // TODO:
        // 将来的にはインライン式
        //  ```biwa
        //  言葉の#red("間")に式が挿入される
        //  ```
        // に対応する際に実装を追加
        // NOTE:
        // std::game::base_engine::write("message")
        // に解決される
        Ok(())
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::NovelWait {
    fn resolve(&self, _ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        // NOTE:
        // std::game::base_engine::wait()
        // に解決される
        Ok(())
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
