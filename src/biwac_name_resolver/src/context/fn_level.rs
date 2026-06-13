use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{ArgDecl, symbols::globals::GenArgsDecl};
use biwac_base::InternedIdent;
use biwac_span::{DefIdKind, LocalGenDefId, Span, VarId};

use crate::{
    DefCollector, ResolveError, ResolveErrorHandler,
    context::{LocalResolveCtx, ResolveCtx},
};

#[derive(Debug)]
struct VariableScope {
    vars: HashMap<InternedIdent, (VarId, Span)>,
    next_var_id: u32,
}

impl VariableScope {
    fn new(next_var_id: u32) -> Self {
        Self {
            vars: HashMap::new(),
            next_var_id,
        }
    }

    fn declare_variable(&mut self, ident: &biwac_ast::Ident) -> Result<VarId, ResolveError> {
        match self.vars.entry(ident.id) {
            Entry::Vacant(e) => {
                let var_id = VarId::new(self.next_var_id);
                e.insert((var_id, ident.span.clone()));
                self.next_var_id += 1;

                Ok(var_id)
            }
            Entry::Occupied(e) => Err(ResolveError::DuplicatedVariableName {
                id: ident.id,
                var1: e.get().1.clone(),
                var2: ident.span.clone(),
            }),
        }
    }
}

#[derive(Debug)]
pub struct FnResolveCtx<'ctx, C: ResolveCtx> {
    ctx: &'ctx C,
    genargs: HashMap<InternedIdent, LocalGenDefId>,
    scopes: Vec<VariableScope>,
    next_var_id: u32,
    self_var: Option<VarId>,
}

impl<'ctx, C: ResolveCtx> ResolveCtx for FnResolveCtx<'ctx, C> {
    fn resolve_path(&self, path: &biwac_ast::Path) -> Result<(), crate::ResolveError> {
        if path.abs_header.is_none() && path.segments.len() == 1 {
            let interned_ident = &path.segments[0].ident.id;

            for scope in self.scopes.iter().rev() {
                if let Some((var_id, _)) = scope.vars.get(interned_ident) {
                    let def_id_kind = DefIdKind::Var(*var_id);
                    path.segments[0]
                        .resolved_id
                        .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                        .unwrap();
                    return Ok(());
                }
            }

            if let Some(def_id) = self.genargs.get(interned_ident) {
                let def_id_kind = DefIdKind::LocalGen(*def_id);
                path.segments[0]
                    .resolved_id
                    .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                    .unwrap();
                Ok(())
            } else {
                self.ctx.resolve_path(path)
            }
        } else {
            self.ctx.resolve_path(path)
        }
    }
}

impl<'ctx, C: ResolveCtx> FnResolveCtx<'ctx, C> {
    pub(crate) fn new(
        ctx: &'ctx C,
        genargs_decl: &Option<GenArgsDecl<LocalGenDefId>>,
        has_self_var: bool,
        args: &[ArgDecl],
        def_collector: &mut DefCollector,
    ) -> Result<Self, Vec<ResolveError>> {
        let mut genargs = HashMap::new();
        let mut errors = Vec::new();
        let mut next_var_id = 0;

        if let Some(genargs_decl) = genargs_decl {
            for item in &genargs_decl.genargs {
                let def_id = if let Some(def_id) = item.def_id.get() {
                    *def_id
                } else {
                    let def_id = LocalGenDefId::new(def_collector.alloc_def_id());
                    // set def_id in AST
                    item.def_id.set(def_id).unwrap();
                    def_id
                };

                match genargs.entry(item.id.id) {
                    Entry::Vacant(e) => {
                        e.insert(def_id);
                    }
                    Entry::Occupied(e) => {
                        errors.push(ResolveError::DuplicatedLocalGenName {
                            name: item.id.id,
                            def_id1: *e.get(),
                            def_id2: def_id,
                        });
                    }
                }
            }
        }

        let self_var = if has_self_var {
            let self_var_id = VarId::new(next_var_id);
            next_var_id += 1;
            Some(self_var_id)
        } else {
            None
        };

        let mut base_scope = VariableScope::new(next_var_id);

        for arg in args {
            base_scope.declare_variable(&arg.id).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(Self {
                ctx,
                genargs,
                scopes: vec![base_scope],
                next_var_id,
                self_var,
            })
        } else {
            Err(errors)
        }
    }
}

impl<'ctx, C: ResolveCtx> LocalResolveCtx for FnResolveCtx<'ctx, C> {
    fn declare_variable(&mut self, ident: &biwac_ast::Ident) -> Result<VarId, ResolveError> {
        self.scopes.last_mut().unwrap().declare_variable(ident)
    }

    fn inner_scope<F: FnOnce(&mut Self) -> Result<(), Vec<ResolveError>>>(
        &mut self,
        f: F,
    ) -> Result<(), Vec<ResolveError>> {
        self.scopes.push(VariableScope::new(self.next_var_id));

        let res = f(self);

        let scope = self.scopes.pop().unwrap();
        self.next_var_id = scope.next_var_id;

        res
    }

    fn resolve_self_var(&self, span: &Span) -> Result<biwac_span::VarId, ResolveError> {
        self.self_var
            .ok_or(ResolveError::UnexpectedSelfType { span: span.clone() })
    }
}
