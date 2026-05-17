use std::collections::HashMap;

use biwac_ast::ImplBlock;
use biwac_base::InternedIdent;
use biwac_hir::{LocGenTyId, TyKind};

use crate::{
    ResolveError,
    context::{ResolveCtx, module_level::ModuleResolveCtx},
};

#[derive(Debug)]
pub struct ImplResolveCtx<'mctx> {
    mctx: &'mctx ModuleResolveCtx<'mctx>,
    genargs: HashMap<InternedIdent, LocGenTyId>,
    self_ty: TyKind,
}

impl ResolveCtx for ImplResolveCtx<'_> {
    fn resolve_path(
        &self,
        path: &biwac_ast::Path,
    ) -> Result<biwac_span::DefIdKind, crate::ResolveError> {
        if path.segments.len() == 1
            && let Some(_) = self.genargs.get(&path.segments[0].ident.id)
        {
            todo!();
        }

        todo!()
    }

    fn opt_self_ty(&self) -> Option<TyKind> {
        Some(self.self_ty.clone())
    }
}

impl<'mctx> ImplResolveCtx<'mctx> {
    pub(crate) fn new(
        mctx: &'mctx ModuleResolveCtx<'mctx>,
        impl_block: &ImplBlock,
    ) -> Result<Self, Vec<ResolveError>> {
        let self_ty = mctx.resolve_typ(&impl_block.self_typ)?;

        // TODO: impl_block にset
        let genargs = match &impl_block.genargs_decl {
            Some(genargs) => genargs
                .genargs
                .iter()
                .enumerate()
                .map(|(i, ident)| (ident.id, LocGenTyId::new(i)))
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
