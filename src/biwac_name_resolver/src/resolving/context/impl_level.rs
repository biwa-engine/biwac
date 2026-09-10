use std::collections::HashMap;

use biwac_ast::{AbsolutePathHeader, ImplBlock};
use biwac_base::InternedIdent;
use biwac_hir::TyKind;
use biwac_span::{DefIdKind, LocalGenDefId, TraitAssocDefId, TraitDefId};

use crate::{
    ResolveError, lowering,
    resolving::{
        context::{
            ResolveCtx,
            fn_level::{GenArgEntry, resolve_genarg_assoc_path},
            module_level::ModuleResolveCtx,
            resolved_bound_traits,
        },
        def_collector::DefCollector,
    },
};

#[derive(Debug)]
pub struct ImplResolveCtx<'mctx> {
    mctx: &'mctx ModuleResolveCtx<'mctx>,
    genargs: HashMap<InternedIdent, GenArgEntry>,
    self_ty: TyKind,
}

impl ResolveCtx for ImplResolveCtx<'_> {
    fn lookup_trait_assoc(
        &self,
        bounds: &[TraitDefId],
        segment: &biwac_ast::PathSegment,
    ) -> Result<TraitAssocDefId, ResolveError> {
        self.mctx.lookup_trait_assoc(bounds, segment)
    }

    fn resolve_path(&self, path: &biwac_ast::Path) -> Result<(), crate::ResolveError> {
        if path.abs_header.is_none()
            && path.segments.len() == 1
            && let Some(entry) = self.genargs.get(&path.segments[0].ident.id)
        {
            let def_id_kind = DefIdKind::LocalGen(entry.def_id);
            if path.segments[0].resolved_id.get().is_none() {
                path.segments[0]
                    .resolved_id
                    .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                    .unwrap();
            }

            Ok(())
        } else if path.abs_header.is_none()
            && path.segments.len() == 2
            && let Some(entry) = self.genargs.get(&path.segments[0].ident.id)
        {
            resolve_genarg_assoc_path(path, entry, self.mctx)
        } else if let Some(AbsolutePathHeader::SelfTyp(self_typ)) = &path.abs_header {
            if self_typ.resolved_id.get().is_none() {
                if let TyKind::Defined(defined_ty) = &self.self_ty {
                    self_typ.resolved_id.set(defined_ty.def_id).unwrap();
                } else {
                    todo!()
                }
            }

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
        let genargs: HashMap<InternedIdent, GenArgEntry> = match &impl_block.genargs_decl {
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

                    (
                        item.id.id,
                        GenArgEntry {
                            def_id,
                            span: item.id.span.clone(),
                            bounds: Vec::new(),
                        },
                    )
                })
                .collect(),
            None => HashMap::new(),
        };

        let prectx = PreImplResolveCtx { mctx, genargs };
        // 制限は impl の対象型より先に解決する。
        // `impl[T: Gyao] Bbb[T]` の `Gyao` は対象型を知らなくても解けるし、
        // 対象型の側が制限を参照することもない。
        prectx.resolve_genarg_bounds(&impl_block.genargs_decl)?;
        prectx.resolve_typ(&impl_block.self_typ)?;
        let self_ty = lowering::ty_kind_from_typ_repr(&impl_block.self_typ, None);

        let mut genargs = prectx.genargs;
        if let Some(decl) = &impl_block.genargs_decl {
            for item in &decl.genargs {
                if let Some(entry) = genargs.get_mut(&item.id.id) {
                    entry.bounds = resolved_bound_traits(item);
                }
            }
        }

        Ok(Self {
            mctx,
            genargs,
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
    genargs: HashMap<InternedIdent, GenArgEntry>,
}

impl ResolveCtx for PreImplResolveCtx<'_> {
    fn resolve_path(&self, path: &biwac_ast::Path) -> Result<(), ResolveError> {
        if path.abs_header.is_none()
            && path.segments.len() == 1
            && let Some(entry) = self.genargs.get(&path.segments[0].ident.id)
        {
            let def_id_kind = DefIdKind::LocalGen(entry.def_id);
            if path.segments[0].resolved_id.get().is_none() {
                path.segments[0]
                    .resolved_id
                    .set(biwac_ast::PathSegmentResolution::Ok(def_id_kind))
                    .unwrap();
            }

            Ok(())
        } else {
            self.mctx.resolve_path(path)
        }
    }
}
