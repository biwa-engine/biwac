use crate::{
    context::{ResolveCtx, fn_level::FnResolveCtx},
    symbols::NameResolve,
};

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::Stmt {
    fn resolve(
        &self,
        ctx: &C,
        def_collector: &mut crate::DefCollector,
    ) -> Result<(), Vec<crate::ResolveError>> {
        match self {
            biwac_ast::Stmt::If(i) => i.resolve(ctx, def_collector),
            biwac_ast::Stmt::While(w) => w.resolve(ctx, def_collector),
            biwac_ast::Stmt::Block(b) => b.resolve(ctx, def_collector),
            biwac_ast::Stmt::Expr(expr) => expr.expr.resolve(ctx, def_collector),
            biwac_ast::Stmt::Return(ret) => ret.expr.resolve(ctx, def_collector),
            biwac_ast::Stmt::VarDecl(var_decl) => var_decl.resolve(ctx, def_collector),
            biwac_ast::Stmt::Assign(assign) => assign.resolve(ctx, def_collector),
        }
    }
}

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::BlockStmt {
    fn resolve(
        &self,
        ctx: &C,
        def_collector: &mut crate::DefCollector,
    ) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();
        // ブロック文はスコープを作る
        fctx.enter_scope();

        for stmt in &self.stmts {
            if let Err(errs) = stmt.resolve(ctx, def_collector) {
                errors.extend(errs);
            }
        }

        fctx.exit_scope();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::IfStmt {
    fn resolve(
        &self,
        ctx: &C,
        def_collector: &mut crate::DefCollector,
    ) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        if let Err(errs) = self.cond.resolve(ctx, def_collector) {
            errors.extend(errs);
        }

        if let Err(errs) = self.then.resolve(ctx, def_collector) {
            errors.extend(errs);
        }

        if let Some(els) = &self.els
            && let Err(errs) = els.resolve(ctx, def_collector)
        {
            errors.extend(errs);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::WhileStmt {
    fn resolve(
        &self,
        ctx: &C,
        def_collector: &mut crate::DefCollector,
    ) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        if let Err(errs) = self.cond.resolve(ctx, def_collector) {
            errors.extend(errs);
        }

        if let Err(errs) = self.stmts.resolve(ctx, def_collector) {
            errors.extend(errs);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<'ctx, C: ResolveCtx> NameResolve<FnResolveCtx<'ctx, C>> for biwac_ast::VarDecl {
    fn resolve(
        &self,
        ctx: &FnResolveCtx<'ctx, C>,
        def_collector: &mut crate::DefCollector,
    ) -> Result<(), Vec<crate::ResolveError>> {
        if let biwac_ast::TypDecl::Typ(typ) = &self.typ {
            ctx.resolve_typ(typ)?;
        }

        // TODO:
        // 宣言した変数を今のスコープに詰んで解決できるようにしたい
        // let id = fctx.declare_variable(&value.id.clone().into(), ty)?;
        todo!();

        Ok(())
    }
}

impl<'ctx, C: ResolveCtx> NameResolve<FnResolveCtx<'ctx, C>> for biwac_ast::AssignStmt {
    fn resolve(
        &self,
        ctx: &FnResolveCtx<'ctx, C>,
        def_collector: &mut crate::DefCollector,
    ) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        if let Err(errs) = self.dst.resolve(ctx, def_collector) {
            errors.extend(errs);
        }

        if let Err(errs) = self.src.resolve(ctx, def_collector) {
            errors.extend(errs);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
