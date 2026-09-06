use crate::{
    ResolveErrorHandler,
    resolving::{LocalNameResolve, context::LocalResolveCtx},
};

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::Stmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        match self {
            biwac_ast::Stmt::If(i) => i.resolve(ctx),
            biwac_ast::Stmt::Match(m) => m.resolve(ctx),
            biwac_ast::Stmt::While(w) => w.resolve(ctx),
            biwac_ast::Stmt::Block(b) => b.resolve(ctx),
            biwac_ast::Stmt::Expr(expr) => expr.expr.resolve(ctx),
            biwac_ast::Stmt::Return(ret) => ret.expr.resolve(ctx),
            biwac_ast::Stmt::VarDecl(var_decl) => var_decl.resolve(ctx),
            biwac_ast::Stmt::Assign(assign) => assign.resolve(ctx),
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::BlockStmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        // ブロック文はスコープを作る
        ctx.inner_scope(|ctx: &mut C| {
            let mut errors = Vec::new();

            for stmt in &self.stmts {
                if let Err(errs) = stmt.resolve(ctx) {
                    errors.extend(errs);
                }
            }

            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        })
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::IfStmt {
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

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::MatchStmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        self.scrutinee.resolve(ctx).handle(&mut errors);

        for arm in &self.arms {
            // パターンが束縛する変数はそのアームの中でだけ見える。
            ctx.inner_scope(|ctx: &mut C| {
                let mut arm_errors = Vec::new();
                arm.pattern.resolve(ctx).handle(&mut arm_errors);
                arm.body.resolve(ctx).handle(&mut arm_errors);

                if arm_errors.is_empty() {
                    Ok(())
                } else {
                    Err(arm_errors)
                }
            })
            .handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::WhileStmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        if let Err(errs) = self.cond.resolve(ctx) {
            errors.extend(errs);
        }

        if let Err(errs) = self.stmts.resolve(ctx) {
            errors.extend(errs);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::VarDecl {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        match ctx.declare_variable(&self.id) {
            Ok(var_id) => {
                self.var_id.set(var_id).unwrap();
            }
            Err(e) => {
                errors.push(e);
            }
        }

        if let biwac_ast::TypDecl::Typ(typ) = &self.typ {
            ctx.resolve_typ(typ).handle(&mut errors);
        }

        self.init.resolve(ctx).handle(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::AssignStmt {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        if let Err(errs) = self.dst.resolve(ctx) {
            errors.extend(errs);
        }

        if let Err(errs) = self.src.resolve(ctx) {
            errors.extend(errs);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
