use biwac_ast::{Path, PathSegmentResolution, PrimTyp, TypRepr, TypReprVal};
use biwac_hir::{DefinedTy, Hir, Ty, TyKind};
use biwac_span::{DefIdKind, GenDefId, LocalGenDefId, TyDefId};

use crate::ResolveError;

pub(crate) fn lower(ast: biwac_ast::ModAst) -> Result<Hir, Vec<ResolveError>> {
    todo!()
}

pub(crate) fn ty_kind_unwrap_from_typ_repr(typ: &TypRepr, self_typ: Option<&TyKind>) -> TyKind {
    match &typ.val {
        TypReprVal::Primitive(p) => match p {
            PrimTyp::Int => TyKind::Int,
            PrimTyp::Uint => TyKind::Int, // TODO
            PrimTyp::Float => TyKind::Float,
            PrimTyp::Bool => TyKind::Bool,
        },
        TypReprVal::Defined(deftyp) => match ty_def_id_try_from_path(&deftyp.path).unwrap() {
            TyDefIdKind::Ty(def_id) => TyKind::Defined(DefinedTy {
                def_id,
                genargs: deftyp
                    .genargs
                    .iter()
                    .flat_map(|genargs| {
                        genargs
                            .iter()
                            .map(|typ| {
                                Ty::new(
                                    ty_kind_unwrap_from_typ_repr(typ, self_typ),
                                    typ.span.clone(),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect(),
            }),
            TyDefIdKind::Gen(def_id) => TyKind::Gen(def_id),
            TyDefIdKind::LocalGen(def_id) => TyKind::LocGen(def_id),
        },
        TypReprVal::SelfTyp => self_typ.unwrap().clone(),
    }
}

/// NOTE: `path` must have been already tried to resolve.
pub(crate) fn def_id_kind_try_from_path(path: &Path) -> Result<DefIdKind, ResolveError> {
    for (i, segment) in path.segments.iter().enumerate() {
        match segment.resolved_id.get() {
            Some(PathSegmentResolution::Ok(def_id_kind)) => {
                if i + 1 == path.segments.len() {
                    return Ok(def_id_kind.clone());
                } else {
                    continue;
                }
            }
            Some(PathSegmentResolution::Err) => {
                return Err(ResolveError::IdentNotFound {
                    ident: segment.ident.clone(),
                });
            }
            None => {
                break;
            }
        }
    }

    panic!("compiler bug: `Path` not resolved yet.")
}

fn ty_def_id_try_from_path(path: &Path) -> Result<TyDefIdKind, ResolveError> {
    match def_id_kind_try_from_path(path)? {
        DefIdKind::Package(pkg_id) => Err(ResolveError::TypeNotFoundPackageFound {
            path: Box::new(path.clone()),
            pkg_id,
        }),
        DefIdKind::Ty(ty_def_id) => Ok(TyDefIdKind::Ty(ty_def_id)),
        DefIdKind::Mod(mod_id) => Err(ResolveError::TypeNotFoundModuleFound {
            path: Box::new(path.clone()),
            mod_id,
        }),
        DefIdKind::Val(val_def_id) => Err(ResolveError::TypeNotFoundValueFound {
            path: Box::new(path.clone()),
            def_id: val_def_id,
        }),
        DefIdKind::Gen(gen_def_id) => Ok(TyDefIdKind::Gen(gen_def_id)),
        DefIdKind::LocalGen(local_gen_def_id) => Ok(TyDefIdKind::LocalGen(local_gen_def_id)),
        DefIdKind::Var(var_id) => Err(ResolveError::TypeNotFoundVariableFound {
            path: Box::new(path.clone()),
            var_id,
        }),
    }
}

enum TyDefIdKind {
    Ty(TyDefId),
    Gen(GenDefId),
    LocalGen(LocalGenDefId),
}

// Ok(genargs) => match ty_def_id_try_from_path(&def_typ.path) {
//     Ok(TyDefIdKind::Ty(def_id)) => {
//         Ok(TyKind::Defined(DefinedTy { def_id, genargs }))
//     }
//     Ok(TyDefIdKind::Gen(def_id)) => {
//         if genargs.is_empty() {
//             Ok(TyKind::Gen(def_id))
//         } else {
//             Err(vec![ResolveError::GenericTypeWithGenArgs {
//                 path: Box::new(def_typ.path.clone()),
//                 def_id,
//             }])
//         }
//     }
//     Ok(TyDefIdKind::LocalGen(def_id)) => {
//         if genargs.is_empty() {
//             Ok(TyKind::LocGen(def_id))
//         } else {
//             Err(vec![ResolveError::LocalGenericTypeWithGenArgs {
//                 path: Box::new(def_typ.path.clone()),
//                 def_id,
//             }])
//         }
//     }
//     Err(e) => Err(vec![e]),
// },
