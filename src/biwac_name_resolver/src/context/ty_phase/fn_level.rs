use std::collections::{HashMap, hash_map::Entry};

use biwac_hir::{Hir, LocGenTyId, Ty};
use biwac_parser::{Ident, PrimTyp, TypRepr, TypReprVal};

use crate::{ResolveError, RsvResult, context::ty_phase::impl_level::ImplLevelTyResolveCtx};

#[derive(Debug)]
pub(crate) struct FnLevelTyResolveCtx<'ictx> {
    ictx: &'ictx ImplLevelTyResolveCtx<'ictx>,
    fn_def_genargs: HashMap<String, LocGenTyId>,
    pub(crate) fn_def_genarg_vec: Vec<(Ident, LocGenTyId)>,
}

impl<'ictx> FnLevelTyResolveCtx<'ictx> {
    pub(crate) fn new(
        ictx: &'ictx ImplLevelTyResolveCtx<'ictx>,
        fn_def_genargs: &Vec<Ident>,
    ) -> RsvResult<Self> {
        let mut next_gen_id = 0;
        let mut fn_def_genarg_map = HashMap::<String, (LocGenTyId, Ident)>::new();
        let mut fn_def_genarg_vec = Vec::new();

        for ident in fn_def_genargs {
            match fn_def_genarg_map.entry(ident.id.clone()) {
                Entry::Vacant(e) => {
                    let id = LocGenTyId::new(next_gen_id);
                    next_gen_id += 1;
                    e.insert((id, ident.clone()));
                    fn_def_genarg_vec.push((ident.clone(), id));
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
            ictx,
            fn_def_genargs: fn_def_genarg_map
                .into_iter()
                .map(|(name, (id, _))| (name, id))
                .collect::<HashMap<_, _>>(),
            fn_def_genarg_vec,
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
                if let Some(id) = deftyp.qualid.only_id()
                    && let Some(gid) = self.fn_def_genargs.get(id)
                {
                    Ok(Ty::LocGen(*gid))
                } else {
                    self.ictx.try_resolve_defined_ty(deftyp, hir)
                }
            }
        }
    }
}
