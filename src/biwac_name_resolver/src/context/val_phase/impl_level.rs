use std::collections::HashMap;

use biwac_base::Span;
use biwac_hir::{DefinedTy, Hir, InferTy, LocGenTyId, Ty, TyKind};
use biwac_parser::{DefTyp, PrimTyp, QualifiedId, TypRepr, TypReprVal};

use crate::{
    RsvResult,
    context::val_phase::{ResolvedValue, module_level::ModuleLevelResolveCtx},
};

//  impl[T] Foo[T] {
//         ^^^^^^^^
//         impl block の開始以降の型の名前解決をする
//  }
#[derive(Debug)]
pub(crate) struct ImplLevelResolveCtx<'mctx> {
    mctx: &'mctx ModuleLevelResolveCtx,
    pub(crate) impl_block_genargs: HashMap<String, (LocGenTyId, Span)>,
}

impl<'mctx> ImplLevelResolveCtx<'mctx> {
    pub(crate) fn new_empty(mctx: &'mctx ModuleLevelResolveCtx) -> Self {
        Self {
            mctx,
            impl_block_genargs: HashMap::new(),
        }
    }

    pub(crate) fn new(
        mctx: &'mctx ModuleLevelResolveCtx,
        impl_block_genargs: HashMap<String, (LocGenTyId, Span)>,
    ) -> RsvResult<Self> {
        Ok(Self {
            mctx,
            impl_block_genargs,
        })
    }

    pub(crate) fn try_resolve_defined_ty(&self, deftyp: &DefTyp, hir: &Hir) -> RsvResult<Ty> {
        // deftypがidのみ(ex: `T`)の場合、
        // 内側から名前解決する
        //
        //  (2) それ以外の外部に定義された型から解決が試みられる
        //
        //  impl[T] Foo[T] {
        //       ^
        //       | (1)次に解決が試みられる
        //  }
        if let Some(id) = deftyp.qualid.only_id()
            && let Some((gid, _)) = self.impl_block_genargs.get(id)
        {
            Ok(Ty::new(TyKind::LocGen(*gid), deftyp.qualid.span.clone()))
        } else {
            // ジェネリック引数の数が合うか検査済み
            let (tid, ty_existence) = self.mctx.try_resolve_defined_tid(deftyp, hir)?;

            let garg_span = Span::new(
                deftyp.qualid.span.module().clone(),
                deftyp.qualid.span.end().clone(),
                deftyp.qualid.span.end().clone(),
            );

            Ok(Ty::new(
                TyKind::Defined(DefinedTy {
                    tid,
                    genargs: if let Some(genargs) = &deftyp.genargs {
                        genargs
                            .iter()
                            .map(|typ| self.try_resolve_ty(typ, hir))
                            .collect::<RsvResult<_>>()?
                    } else {
                        vec![
                            Ty::new(TyKind::Infer(InferTy::Unknown), garg_span);
                            ty_existence.genarg_len
                        ]
                    },
                }),
                deftyp.qualid.span.clone(),
            ))
        }
    }

    pub(crate) fn try_resolve_ty(&self, typ: &TypRepr, hir: &Hir) -> RsvResult<Ty> {
        match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => Ok(Ty::new(TyKind::Int, typ.span.clone())),
                // TODO: Uint
                PrimTyp::Uint => Ok(Ty::new(TyKind::Int, typ.span.clone())),
                PrimTyp::Float => Ok(Ty::new(TyKind::Float, typ.span.clone())),
                PrimTyp::Bool => Ok(Ty::new(TyKind::Bool, typ.span.clone())),
            },
            TypReprVal::Defined(deftyp) => self.try_resolve_defined_ty(deftyp, hir),
        }
    }

    #[inline]
    pub(crate) fn try_resolve_value(
        &self,
        qualid: &QualifiedId,
        hir: &Hir,
    ) -> RsvResult<ResolvedValue> {
        self.mctx.try_resolve_value(qualid, hir)
    }
}
