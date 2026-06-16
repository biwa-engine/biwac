use std::collections::HashMap;

use biwac_ast::ImplBlock;
use biwac_base::InternedIdent;
use biwac_hir::TyKind;
use biwac_span::{DefIdKind, LocalGenDefId};

use crate::{
    ResolveError, lowering,
    resolving::{
        context::{ResolveCtx, module_level::ModuleResolveCtx},
        def_collector::DefCollector,
    },
};

#[derive(Debug)]
pub struct ImplResolveCtx<'mctx> {
    mctx: &'mctx ModuleResolveCtx<'mctx>,
    genargs: HashMap<InternedIdent, LocalGenDefId>,
    self_ty: TyKind,
}

impl ResolveCtx for ImplResolveCtx<'_> {
    fn resolve_path(&self, path: &biwac_ast::Path) -> Result<(), crate::ResolveError> {
        if path.abs_header.is_none()
            && path.segments.len() == 1
            && let Some(def_id) = self.genargs.get(&path.segments[0].ident.id)
        {
            let def_id_kind = DefIdKind::LocalGen(*def_id);
            path.segments[0]
                .resolved_id
                .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                .unwrap();
            Ok(())
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
                        item.def_id.set(def_id).unwrap();
                        def_id
                    };

                    (item.id.id, def_id)
                })
                .collect(),
            None => HashMap::new(),
        };

        let prectx = PreImplResolveCtx { mctx, genargs };
        prectx.resolve_typ(&impl_block.self_typ)?;
        let self_ty = lowering::ty_kind_from_typ_repr(&impl_block.self_typ, None);

        Ok(Self {
            mctx,
            genargs: prectx.genargs,
            self_ty,
        })
    }
}

/// impl block の実装対象の型自体の表明での解決を行う
/// ```biwa
/// impl[T, U] Foo[T, U] {
///                ^  ^
///                これらの解決には、impl
///                blockのジェネリック引数宣言を知っている必要があるが、Selfは知らない
/// }
/// ```

#[derive(Debug)]
struct PreImplResolveCtx<'mctx> {
    mctx: &'mctx ModuleResolveCtx<'mctx>,
    genargs: HashMap<InternedIdent, LocalGenDefId>,
}

impl ResolveCtx for PreImplResolveCtx<'_> {
    fn resolve_path(&self, path: &biwac_ast::Path) -> Result<(), ResolveError> {
        if path.abs_header.is_none()
            && path.segments.len() == 1
            && let Some(def_id) = self.genargs.get(&path.segments[0].ident.id)
        {
            let def_id_kind = DefIdKind::LocalGen(*def_id);
            path.segments[0]
                .resolved_id
                .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                .unwrap();
            Ok(())
        } else {
            self.mctx.resolve_path(path)
        }
    }
}
