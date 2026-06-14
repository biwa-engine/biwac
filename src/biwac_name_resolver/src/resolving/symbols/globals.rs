use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{ArgDecl, RetTypRepr};

use crate::{
    ResolveError, ResolveErrorHandler,
    resolving::{
        LocalNameResolve, NameResolve,
        context::{
            LocalResolveCtx, ResolveCtx, fn_level::FnResolveCtx, impl_level::ImplResolveCtx,
            module_level::ModuleResolveCtx, ty_def_level::TyDefResolveCtx,
        },
        def_collector::DefCollector,
    },
};

fn fn_signature_resolve<C: LocalResolveCtx>(
    ctx: &C,
    args: &[ArgDecl],
    rtype: &RetTypRepr,
) -> Result<(), Vec<ResolveError>> {
    let mut errors = Vec::new();

    for arg in args {
        ctx.resolve_typ(&arg.typ).handle(&mut errors);
    }

    match rtype {
        RetTypRepr::Typ(typ) => {
            ctx.resolve_typ(typ).handle(&mut errors);
        }
        RetTypRepr::Void(_) => {
            // nothing to do
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::FnDef {
    fn resolve(&self, ctx: &C, def_collector: &mut DefCollector) -> Result<(), Vec<ResolveError>> {
        let mut ctx = FnResolveCtx::new(ctx, &self.genargs, false, &self.args.args, def_collector)?;

        let mut errors = Vec::new();

        fn_signature_resolve(&ctx, &self.args.args, &self.rtype).handle(&mut errors);

        for stmt in &self.stmts {
            stmt.resolve(&mut ctx).handle(&mut errors);
        }

        if let Some(expr) = &self.expr {
            expr.resolve(&mut ctx).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl NameResolve<ImplResolveCtx<'_>> for biwac_ast::MethodDef {
    fn resolve(
        &self,
        ctx: &ImplResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let mut ctx = FnResolveCtx::new(ctx, &self.genargs, true, &self.args.args, def_collector)?;

        let mut errors = Vec::new();

        fn_signature_resolve(&ctx, &self.args.args, &self.rtype).handle(&mut errors);

        for stmt in &self.stmts {
            stmt.resolve(&mut ctx).handle(&mut errors);
        }

        if let Some(expr) = &self.expr {
            expr.resolve(&mut ctx).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::NativeFnDef {
    fn resolve(&self, ctx: &C, def_collector: &mut DefCollector) -> Result<(), Vec<ResolveError>> {
        let ctx = FnResolveCtx::new(ctx, &self.genargs, false, &self.args.args, def_collector)?;
        fn_signature_resolve(&ctx, &self.args.args, &self.rtype)
    }
}

impl NameResolve<ImplResolveCtx<'_>> for biwac_ast::NativeMethodDef {
    fn resolve(
        &self,
        ctx: &ImplResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let ctx = FnResolveCtx::new(ctx, &self.genargs, true, &self.args.args, def_collector)?;
        fn_signature_resolve(&ctx, &self.args.args, &self.rtype)
    }
}

impl NameResolve<ModuleResolveCtx<'_>> for biwac_ast::StructDef {
    fn resolve(
        &self,
        ctx: &ModuleResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let ctx = TyDefResolveCtx::new(ctx, &self.genargs, def_collector)?;
        let mut members = HashMap::new();
        let mut errors = Vec::new();

        for (ident, typ) in &self.members {
            match members.entry(ident.id) {
                Entry::Vacant(e) => {
                    e.insert(&ident.span);
                }
                Entry::Occupied(e) => {
                    errors.push(ResolveError::DuplicatedStructMember {
                        name: ident.id,
                        span1: e.get().to_owned().clone(),
                        span2: ident.span.clone(),
                    });
                }
            }

            ctx.resolve_typ(typ).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl NameResolve<ModuleResolveCtx<'_>> for biwac_ast::TypeAlias {
    fn resolve(
        &self,
        ctx: &ModuleResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let ctx = TyDefResolveCtx::new(ctx, &self.genargs, def_collector)?;
        ctx.resolve_typ(&self.right)
    }
}

impl NameResolve<ModuleResolveCtx<'_>> for biwac_ast::NativeTypeAlias {
    fn resolve(
        &self,
        _ctx: &ModuleResolveCtx<'_>,
        _def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        // nothing to do because right is native string
        Ok(())
    }
}

impl NameResolve<ModuleResolveCtx<'_>> for biwac_ast::ImplBlock {
    fn resolve(
        &self,
        ctx: &ModuleResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let ctx = ImplResolveCtx::new(ctx, self, def_collector)?;
        let mut errors = Vec::new();

        for fn_def in &self.assoc_fns {
            fn_def.resolve(&ctx, def_collector).handle(&mut errors);
        }

        for method_def in &self.methods {
            method_def.resolve(&ctx, def_collector).handle(&mut errors);
        }

        for fn_def in &self.native_assoc_fns {
            fn_def.resolve(&ctx, def_collector).handle(&mut errors);
        }

        for method_def in &self.native_methods {
            method_def.resolve(&ctx, def_collector).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::NovelScene {
    fn resolve(&self, ctx: &C, def_collector: &mut DefCollector) -> Result<(), Vec<ResolveError>> {
        let mut ctx = FnResolveCtx::new(ctx, &None, false, &self.args.args, def_collector)?;

        let mut errors = Vec::new();

        fn_signature_resolve(&ctx, &self.args.args, &self.rtype).handle(&mut errors);

        for stmt in &self.stmts {
            stmt.resolve(&mut ctx).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
