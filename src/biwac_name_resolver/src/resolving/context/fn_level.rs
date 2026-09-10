use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{ArgDecl, symbols::globals::GenArgsDecl};
use biwac_base::InternedIdent;
use biwac_span::{DefIdKind, LocalGenDefId, Span, TraitDefId, VarId};

use crate::resolving::{
    ResolveError,
    context::{LocalResolveCtx, ResolveCtx, resolved_bound_traits},
    def_collector::DefCollector,
};

/// 宣言されたジェネリック引数 1 つ。
#[derive(Debug)]
pub(crate) struct GenArgEntry {
    pub(crate) def_id: LocalGenDefId,
    /// 宣言位置。重複の報告に使う。
    pub(crate) span: Span,
    /// 制限に書かれた trait。`T::guee()` の解決に使う。
    pub(crate) bounds: Vec<TraitDefId>,
}

/// 1 つのブロックが持つ変数の名前空間。
///
/// VarId の採番は [`FnResolveCtx`] が一元的に行う。
/// スコープごとにカウンタを持たせると、内側のブロックが
/// 古いカウンタから採番して外側の変数と番号が衝突する。
#[derive(Debug)]
struct VariableScope {
    vars: HashMap<InternedIdent, (VarId, Span)>,
}

impl VariableScope {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    fn declare_variable(
        &mut self,
        ident: &biwac_ast::Ident,
        var_id: VarId,
    ) -> Result<VarId, ResolveError> {
        match self.vars.entry(ident.id) {
            Entry::Vacant(e) => {
                e.insert((var_id, ident.span.clone()));

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
    /// 宣言されたジェネリック引数。
    genargs: HashMap<InternedIdent, GenArgEntry>,
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

            if let Some(entry) = self.genargs.get(interned_ident) {
                let def_id_kind = DefIdKind::LocalGen(entry.def_id);
                path.segments[0]
                    .resolved_id
                    .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                    .unwrap();
                Ok(())
            } else {
                self.ctx.resolve_path(path)
            }
        } else if path.abs_header.is_none()
            && path.segments.len() == 2
            && let Some(entry) = self.genargs.get(&path.segments[0].ident.id)
        {
            // `T::guee()`。制限にある trait から名前を引く。
            //
            // 実装は単相化まで決まらないので、
            // `Val` ではなく `TraitAssoc` に解決する。
            resolve_genarg_assoc_path(path, entry, self.ctx)
        } else {
            self.ctx.resolve_path(path)
        }
    }

    fn lookup_trait_assoc(
        &self,
        bounds: &[biwac_span::TraitDefId],
        segment: &biwac_ast::PathSegment,
    ) -> Result<biwac_span::TraitAssocDefId, ResolveError> {
        self.ctx.lookup_trait_assoc(bounds, segment)
    }

    fn opt_self_ty(&self) -> Option<biwac_hir::TyKind> {
        self.ctx.opt_self_ty()
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
                        e.insert(GenArgEntry {
                            def_id,
                            span: item.id.span.clone(),
                            bounds: Vec::new(),
                        });
                    }
                    Entry::Occupied(e) => {
                        errors.push(ResolveError::DuplicatedLocalGenName {
                            name: item.id.id,
                            span1: e.get().span.clone(),
                            span2: item.id.span.clone(),
                        });
                    }
                }
            }
        }

        // 制限は、宣言された引数がすべて見えるようになってから解決する
        // (`impl[T, U: Conv[T]]` のように、後ろの制限が前の引数を参照できる)。
        let prectx = Self {
            ctx,
            genargs,
            scopes: vec![VariableScope::new()],
            next_var_id: 1,
            self_var: None,
        };
        if let Err(errs) = prectx.resolve_genarg_bounds(genargs_decl) {
            errors.extend(errs);
        }
        let mut genargs = prectx.genargs;

        if let Some(genargs_decl) = genargs_decl {
            for item in &genargs_decl.genargs {
                if let Some(entry) = genargs.get_mut(&item.id.id) {
                    entry.bounds = resolved_bound_traits(item);
                }
            }
        }

        let self_var = if has_self_var {
            Some(VarId::SELF_VARIABLE)
        } else {
            None
        };

        // because 0 is reserved for `self`, normal variable must be 1 or bigger.
        let mut base_scope = VariableScope::new();
        let mut next_var_id = 1;

        for arg in args {
            match base_scope.declare_variable(&arg.id, VarId::new(next_var_id)) {
                Ok(var_id) => {
                    next_var_id += 1;
                    arg.var_id.set(var_id).unwrap();
                }
                Err(e) => {
                    errors.push(e);
                }
            }
        }

        if errors.is_empty() {
            Ok(Self {
                ctx,
                genargs,
                next_var_id,
                scopes: vec![base_scope],
                self_var,
            })
        } else {
            Err(errors)
        }
    }
}

impl<'ctx, C: ResolveCtx> LocalResolveCtx for FnResolveCtx<'ctx, C> {
    fn declare_variable(&mut self, ident: &biwac_ast::Ident) -> Result<VarId, ResolveError> {
        let var_id = VarId::new(self.next_var_id);
        let declared = self
            .scopes
            .last_mut()
            .unwrap()
            .declare_variable(ident, var_id)?;
        self.next_var_id += 1;

        Ok(declared)
    }

    fn inner_scope<F: FnOnce(&mut Self) -> Result<(), Vec<ResolveError>>>(
        &mut self,
        f: F,
    ) -> Result<(), Vec<ResolveError>> {
        // 番号は関数全体で通し。ブロックを抜けても戻さない
        // (戻すと、内側で使った番号を外側の変数が再び使ってしまう)。
        self.scopes.push(VariableScope::new());

        let res = f(self);

        self.scopes.pop().unwrap();

        res
    }

    fn resolve_self_var(&self, span: &Span) -> Result<biwac_span::VarId, ResolveError> {
        self.self_var
            .ok_or(ResolveError::UnexpectedSelfVariable { span: span.clone() })
    }
}

/// `T::guee` のように、ジェネリック引数の制限を通して関連アイテムを引く。
///
/// 実装は単相化まで決まらないので [`DefIdKind::TraitAssoc`] に解決する。
/// 制限が付いていなければ、その名前はどこからも来ないのでエラーである。
pub(crate) fn resolve_genarg_assoc_path<C: ResolveCtx + ?Sized>(
    path: &biwac_ast::Path,
    entry: &GenArgEntry,
    ctx: &C,
) -> Result<(), ResolveError> {
    if path.segments[0].resolved_id.get().is_none() {
        path.segments[0]
            .resolved_id
            .set(biwac_ast::PathSegmentResolution::Ok(DefIdKind::LocalGen(
                entry.def_id,
            )))
            .unwrap();
    }

    match ctx.lookup_trait_assoc(&entry.bounds, &path.segments[1]) {
        Ok(assoc) => {
            path.segments[1]
                .resolved_id
                .set(biwac_ast::PathSegmentResolution::Ok(DefIdKind::TraitAssoc(
                    assoc,
                )))
                .unwrap();
            Ok(())
        }
        Err(e) => {
            path.segments[1]
                .resolved_id
                .set(biwac_ast::PathSegmentResolution::Err)
                .unwrap();
            Err(e)
        }
    }
}
