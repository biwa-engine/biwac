use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{AbsolutePathHeader, symbols::globals::GenArgsDecl};
use biwac_base::InternedIdent;
use biwac_hir::{DefinedTy, Ty, TyKind};
use biwac_span::{DefIdKind, GenDefId, Span, TyDefId};

use crate::{
    ResolveError,
    resolving::{context::ResolveCtx, def_collector::DefCollector},
};

#[derive(Debug)]
pub struct TyDefResolveCtx<'ctx, C: ResolveCtx> {
    ctx: &'ctx C,
    genargs: HashMap<InternedIdent, GenDefId>,
    genarg_list: Vec<(GenDefId, Span)>,
    def_id: TyDefId,
}

impl<'ctx, C: ResolveCtx> ResolveCtx for TyDefResolveCtx<'ctx, C> {
    fn resolve_path(&self, path: &biwac_ast::Path) -> Result<(), crate::ResolveError> {
        if path.abs_header.is_none()
            && path.segments.len() == 1
            && let Some(def_id) = self.genargs.get(&path.segments[0].ident.id)
        {
            let def_id_kind = DefIdKind::Gen(*def_id);
            path.segments[0]
                .resolved_id
                .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                .unwrap();
            Ok(())
        } else if let Some(AbsolutePathHeader::SelfTyp(self_typ)) = &path.abs_header {
            if self_typ.resolved_id.get().is_none() {
                self_typ.resolved_id.set(self.def_id).unwrap();
            }

            Ok(())
        } else {
            self.ctx.resolve_path(path)
        }
    }

    fn opt_self_ty(&self) -> Option<biwac_hir::TyKind> {
        Some(TyKind::Defined(DefinedTy {
            def_id: self.def_id,
            genargs: self
                .genarg_list
                .iter()
                .map(|(def_id, span)| Ty::new(TyKind::Gen(*def_id), span.clone()))
                .collect(),
        }))
    }
}

impl<'ctx, C: ResolveCtx> TyDefResolveCtx<'ctx, C> {
    pub(crate) fn new(
        ctx: &'ctx C,
        genargs_decl: &Option<GenArgsDecl<GenDefId>>,
        def_collector: &mut DefCollector,
        def_id: TyDefId,
    ) -> Result<Self, Vec<ResolveError>> {
        let mut genargs = HashMap::new();
        let mut genarg_list = Vec::new();
        let mut errors = Vec::new();

        if let Some(genargs_decl) = genargs_decl {
            for item in &genargs_decl.genargs {
                let def_id = if let Some(def_id) = item.def_id.get() {
                    *def_id
                } else {
                    let def_id = GenDefId::new(def_collector.alloc_def_id());
                    // set def_id in AST
                    item.def_id.set(def_id).unwrap();
                    def_id
                };

                match genargs.entry(item.id.id) {
                    Entry::Vacant(e) => {
                        e.insert(def_id);
                        genarg_list.push((def_id, item.id.span.clone()));
                    }
                    Entry::Occupied(e) => {
                        let first = *e.get();
                        let span1 = genarg_list
                            .iter()
                            .find(|(id, _)| *id == first)
                            .map(|(_, span)| span.clone())
                            .unwrap_or_else(|| item.id.span.clone());

                        errors.push(ResolveError::DuplicatedGenName {
                            name: item.id.id,
                            span1,
                            span2: item.id.span.clone(),
                        });
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(Self {
                ctx,
                genargs,
                genarg_list,
                def_id,
            })
        } else {
            Err(errors)
        }
    }
}
