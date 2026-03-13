use std::collections::{HashMap, hash_map::Entry};

use biwac_hir::{DefinedTy, GenTyId, Hir, InferTy, Ty};
use biwac_parser::{Ident, PrimTyp, TypRepr, TypReprVal};

use crate::{ResolveError, RsvResult, context::ty_phase::module_level::ModuleLevelTyResolveCtx};

#[derive(Debug)]
pub(crate) struct TyDefLevelTyResolveCtx<'mctx> {
    mctx: &'mctx ModuleLevelTyResolveCtx,
    ty_def_genargs: HashMap<String, GenTyId>,
    pub(crate) ty_def_genarg_vec: Vec<GenTyId>,
}

impl<'mctx> TyDefLevelTyResolveCtx<'mctx> {
    pub(crate) fn new(
        mctx: &'mctx ModuleLevelTyResolveCtx,
        ty_def_genargs: &Vec<Ident>,
    ) -> RsvResult<Self> {
        let mut next_gen_id = 0;
        let mut ty_def_genarg_map = HashMap::<String, (GenTyId, Ident)>::new();
        let mut ty_def_genarg_vec = Vec::<GenTyId>::new();

        for ident in ty_def_genargs {
            match ty_def_genarg_map.entry(ident.id.clone()) {
                Entry::Vacant(e) => {
                    let id = GenTyId::new(next_gen_id);
                    next_gen_id += 1;
                    e.insert((id, ident.clone()));
                    ty_def_genarg_vec.push(id);
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
            ty_def_genargs: ty_def_genarg_map
                .into_iter()
                .map(|(name, (id, _))| (name, id))
                .collect::<HashMap<_, _>>(),
            ty_def_genarg_vec,
        })
    }

    pub(crate) fn try_resolve_ty(&self, typ: &TypRepr, hir: &Hir) -> RsvResult<Ty> {
        match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => Ok(Ty::Int),
                PrimTyp::Uint => Ok(Ty::Int), // TODO
                PrimTyp::Float => Ok(Ty::Float),
                PrimTyp::Bool => Ok(Ty::Bool),
            },
            TypReprVal::Defined(deftyp) => {
                // deftypがidのみ(ex: `T`)の場合、
                // 内側から名前解決する
                //
                //  ```
                //  import foo::bar::T;
                //                   ^
                //                   | (3)さらに次に解決が試みられる
                //
                //  type T = Bar[Int];
                //       ^
                //       | (2)次に解決が試みられる
                //
                //  struct Foo[T, U] {
                //            ^^^^^^
                //            | (1)まず解決が試みられる
                //      x: T,
                //      y: U,
                //      z: Int,
                //  }
                //  ```
                if deftyp.genargs.is_none()
                    && let Some(id) = deftyp.qualid.only_id()
                    && let Some(gid) = self.ty_def_genargs.get(id)
                {
                    Ok(Ty::Gen(*gid))
                } else {
                    // ジェネリック引数の数が合うか検査済み
                    let (tid, ty_existence) = self.mctx.try_resolve_defined_tid(deftyp, hir)?;

                    Ok(Ty::Defined(DefinedTy {
                        tid,
                        genargs: if let Some(genargs) = &deftyp.genargs {
                            genargs
                                .iter()
                                .map(|typ| self.try_resolve_ty(typ, hir))
                                .collect::<RsvResult<_>>()?
                        } else {
                            vec![Ty::Infer(InferTy::Unknown); ty_existence.genarg_len]
                        },
                    }))
                }
            }
        }
    }
}
