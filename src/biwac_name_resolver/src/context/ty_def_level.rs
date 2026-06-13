use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::symbols::globals::GenArgsDecl;
use biwac_base::InternedIdent;
use biwac_span::{DefIdKind, GenDefId};

use crate::{DefCollector, ResolveError, context::ResolveCtx};

#[derive(Debug)]
pub struct TyDefResolveCtx<'ctx, C: ResolveCtx> {
    ctx: &'ctx C,
    genargs: HashMap<InternedIdent, GenDefId>,
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
        } else {
            self.ctx.resolve_path(path)
        }
    }
}

impl<'ctx, C: ResolveCtx> TyDefResolveCtx<'ctx, C> {
    pub(crate) fn new(
        ctx: &'ctx C,
        genargs_decl: &Option<GenArgsDecl<GenDefId>>,
        def_collector: &mut DefCollector,
    ) -> Result<Self, Vec<ResolveError>> {
        let mut genargs = HashMap::new();
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
                    }
                    Entry::Occupied(e) => {
                        errors.push(ResolveError::DuplicatedGenName {
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
