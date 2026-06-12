use std::collections::HashMap;

use biwac_ast::ImplBlock;
use biwac_base::InternedIdent;
use biwac_hir::TyKind;
use biwac_span::{DefIdKind, LocalGenDefId};

use crate::{
    DefCollector, ResolveError,
    context::{ResolveCtx, module_level::ModuleResolveCtx},
};

#[derive(Debug)]
pub struct ImplResolveCtx<'mctx> {
    mctx: &'mctx ModuleResolveCtx<'mctx>,
    genargs: HashMap<InternedIdent, LocalGenDefId>,
    self_ty: TyKind,
}

impl ResolveCtx for ImplResolveCtx<'_> {
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
            self.mctx.resolve_path(path)
        }
    }

    fn opt_self_ty(&self) -> Option<TyKind> {
        Some(self.self_ty.clone())
    }
}

impl<'mctx> ImplResolveCtx<'mctx> {
    pub(crate) fn new(
        mctx: &'mctx ModuleResolveCtx<'mctx>,
        impl_block: &ImplBlock,
        def_collector: &mut DefCollector,
    ) -> Result<Self, Vec<ResolveError>> {
        let self_ty = mctx.resolve_typ(&impl_block.self_typ)?;

        let genargs = match &impl_block.genargs_decl {
            Some(genargs) => genargs
                .genargs
                .iter()
                .map(|item| {
                    let def_id = if let Some(def_id) = item.def_id.get() {
                        *def_id
                    } else {
                        let def_id = LocalGenDefId::new(def_collector.alloc_def_id());
                        // set def_id in AST
                        item.def_id.set(def_id);
                        def_id
                    };

                    (item.id.id, def_id)
                })
                .collect(),
            None => HashMap::new(),
        };

        Ok(Self {
            mctx,
            genargs,
            self_ty,
        })
    }
}
