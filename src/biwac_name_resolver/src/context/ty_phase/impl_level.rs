use std::collections::{HashMap, hash_map::Entry};

use biwac_base::Span;
use biwac_hir::{DefinedTy, Hir, InferTy, LocGenTyId, Ty, TyKind};
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
    pub(crate) impl_block_genarg_vec: Vec<(Ident, LocGenTyId)>,
}

impl<'mctx> ImplLevelTyResolveCtx<'mctx> {
    pub(crate) fn new_empty(mctx: &'mctx ModuleLevelTyResolveCtx) -> Self {
        Self {
            mctx,
            impl_block_genargs: HashMap::new(),
            impl_block_genarg_vec: vec![],
        }
    }

    pub(crate) fn new(
        mctx: &'mctx ModuleLevelTyResolveCtx,
        impl_block_genargs: &Vec<Ident>,
    ) -> RsvResult<Self> {
        let mut next_gen_id = 0;
        let mut impl_block_genarg_map = HashMap::<String, (LocGenTyId, Ident)>::new();
        let mut impl_block_genarg_vec = vec![];

        for ident in impl_block_genargs {
            match impl_block_genarg_map.entry(ident.id.clone()) {
                Entry::Vacant(e) => {
                    let id = LocGenTyId::new(next_gen_id);
                    next_gen_id += 1;
                    e.insert((id, ident.clone()));
                    impl_block_genarg_vec.push((ident.clone(), id));
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
            impl_block_genarg_vec,
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
            && let Some((lgid, _)) = self.impl_block_genargs.get(id)
        {
            // TODO: T[U] のように、ジェネリック型にgenargsがあるのは不正
            Ok(Ty::new(TyKind::LocGen(*lgid), deftyp.qualid.span.clone()))
        } else {
            // ジェネリック引数の数が合うか検査済み
            let (tid, ty_existence) = self.mctx.try_resolve_defined_tid(deftyp, hir)?;

            // ジェネリック引数のspanは、qualidの次の位置に指定
            //  ```biwa
            //  foo::bar::Baz {
            //               ^
            //      x = x,
            //      y = y,
            //  }
            //  ```
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
}
