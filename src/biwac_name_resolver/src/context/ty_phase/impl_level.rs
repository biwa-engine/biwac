use std::collections::{HashMap, hash_map::Entry};

use biwac_base::Span;
use biwac_hir::{Hir, LocGenTyId, Ty};
use biwac_parser::{DefTyp, Ident, PrimTyp, TypRepr, TypReprVal};

use crate::{ResolveError, RsvResult, context::ty_phase::module_level::ModuleLevelTyResolveCtx};

//  impl[T] Foo[T] {
//         ^^^^^^^^
//         impl block の開始以降の型の名前解決をする
//  }
#[derive(Debug)]
pub(crate) struct ImplLevelTyResolveCtx<'mctx> {
    mctx: &'mctx ModuleLevelTyResolveCtx,
    pub(crate) impl_block_genargs: HashMap<String, (LocGenTyId, Span)>,
}

impl<'mctx> ImplLevelTyResolveCtx<'mctx> {
    pub(crate) fn new_empty(mctx: &'mctx ModuleLevelTyResolveCtx) -> Self {
        Self {
            mctx,
            impl_block_genargs: HashMap::new(),
        }
    }

    pub(crate) fn new(
        mctx: &'mctx ModuleLevelTyResolveCtx,
        impl_block_genargs: &Vec<Ident>,
    ) -> RsvResult<Self> {
        let mut next_gen_id = 0;
        let mut impl_block_genarg_map = HashMap::<String, (LocGenTyId, Ident)>::new();

        for ident in impl_block_genargs {
            match impl_block_genarg_map.entry(ident.id.clone()) {
                Entry::Vacant(e) => {
                    let id = LocGenTyId::new(next_gen_id);
                    next_gen_id += 1;
                    e.insert((id, ident.clone()));
                }
                Entry::Occupied(e) => {
                    return Err(ResolveError::DuplicatedGenericTypeDeclaration {
                        tid1: Box::new(e.get().1.clone()),
                        tid2: Box::new(ident.clone()),
                    });
                }
            }
        }

        Ok(Self {
            mctx,
            impl_block_genargs: impl_block_genarg_map
                .into_iter()
                .map(|(name, (id, ident))| (name, (id, ident.span)))
                .collect::<HashMap<_, _>>(),
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
            Ok(Ty::LocGen(*gid))
        } else {
            self.mctx.try_resolve_defined_ty(deftyp, hir)
        }
    }

    pub(crate) fn try_resolve_ty(&self, typ: &TypRepr, hir: &Hir) -> RsvResult<Ty> {
        match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => Ok(Ty::Int),
                PrimTyp::Uint => Ok(Ty::Int), // TODO
                PrimTyp::Float => Ok(Ty::Float),
                PrimTyp::Bool => Ok(Ty::Bool),
            },
            TypReprVal::Defined(deftyp) => self.try_resolve_defined_ty(deftyp, hir),
        }
    }
}
