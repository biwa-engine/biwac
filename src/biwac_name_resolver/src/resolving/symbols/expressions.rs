use crate::{
    ResolveErrorHandler,
    resolving::{LocalNameResolve, context::LocalResolveCtx},
};

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::Primary {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        match self {
            biwac_ast::Primary::Literal(literal) => literal.resolve(ctx),
            biwac_ast::Primary::Variable(v) => match v {
                biwac_ast::Variable::Path(path) => {
                    ctx.resolve_path(path).map(|_| ()).map_err(|e| vec![e])
                }
                biwac_ast::Variable::SelfVar(span) => {
                    // TODO: store resolved id
                    ctx.resolve_self_var(span).map(|_| ()).map_err(|e| vec![e])
                }
            },
            biwac_ast::Primary::FnCall(fn_call) => fn_call.resolve(ctx),
            biwac_ast::Primary::MemberAccess(member_access) => member_access.resolve(ctx),
            biwac_ast::Primary::MethodCall(method_call) => method_call.resolve(ctx),
            biwac_ast::Primary::IfExpr(if_expr) => if_expr.resolve(ctx),
            biwac_ast::Primary::Block(block) => block.resolve(ctx),
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::IfExpr {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        self.cond.resolve(ctx).handle(&mut errors);
        self.then.resolve(ctx).handle(&mut errors);
        self.els.resolve(ctx).handle(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::BlockExpr {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        for stmt in &self.stmts {
            stmt.resolve(ctx).handle(&mut errors);
        }

        self.expr.resolve(ctx).handle(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::FnCall {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        ctx.resolve_path(&self.path).handle(&mut errors);

        for arg in &self.args {
            arg.resolve(ctx).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::MemberAccess {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        self.left.resolve(ctx)
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::MethodCall {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        self.left.resolve(ctx).handle(&mut errors);

        for arg in &self.args {
            arg.resolve(ctx).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::Literal {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        match self {
            biwac_ast::Literal::Integer(_)
            | biwac_ast::Literal::String(_)
            | biwac_ast::Literal::Bool(_) => Ok(()),

            biwac_ast::Literal::Struct(struct_) => {
                let mut errors = Vec::new();

                ctx.resolve_path(&struct_.path).handle(&mut errors);

                for (_, expr) in &struct_.members {
                    expr.resolve(ctx).handle(&mut errors);
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::UnaryExpr {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        self.right.resolve(ctx)
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::BinaryExpr {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        let mut errors = Vec::new();

        self.left.resolve(ctx).handle(&mut errors);
        self.right.resolve(ctx).handle(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for biwac_ast::Exprs {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<crate::ResolveError>> {
        match self {
            biwac_ast::Exprs::Primary(prim) => prim.resolve(ctx),
            biwac_ast::Exprs::Unary(u) => u.resolve(ctx),
            biwac_ast::Exprs::Binary(b) => b.resolve(ctx),
        }
    }
}
