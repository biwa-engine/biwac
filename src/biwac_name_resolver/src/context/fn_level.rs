use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::symbols::globals::GenArgsDecl;
use biwac_base::InternedIdent;
use biwac_span::{DefIdKind, LocalGenDefId};

use crate::{DefCollector, ResolveError, context::ResolveCtx};

#[derive(Debug)]
pub struct FnResolveCtx<'ctx, C: ResolveCtx> {
    ctx: &'ctx C,
    genargs: HashMap<InternedIdent, LocalGenDefId>,
}

impl<'ctx, C: ResolveCtx> ResolveCtx for FnResolveCtx<'ctx, C> {
    fn resolve_path(
        &self,
        path: &biwac_ast::Path,
    ) -> Result<biwac_span::DefIdKind, crate::ResolveError> {
        if path.abs_header.is_none()
            && path.segments.len() == 1
            && let Some(def_id) = self.genargs.get(&path.segments[0].ident.id)
        {
            let def_id_kind = DefIdKind::LocalGen(*def_id);
            path.segments[0]
                .resolved_id
                .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind));
            Ok(def_id_kind)
        } else {
            self.ctx.resolve_path(path)
        }
    }
}

impl<'ctx, C: ResolveCtx> FnResolveCtx<'ctx, C> {
    pub(crate) fn new(
        ctx: &'ctx C,
        genargs_decl: &Option<GenArgsDecl<LocalGenDefId>>,
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
                    item.def_id.set(def_id);
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

        if errors.is_empty() {
            Ok(Self { ctx, genargs })
        } else {
            Err(errors)
        }
    }
}
