pub(crate) mod impl_level;
pub(crate) mod module_level;
pub(crate) mod ty_phase;
pub(crate) mod val_phase;

use biwac_ast::{Path, PathSegmentResolution, PrimTyp, TypRepr, TypReprVal};
use biwac_hir::{DefinedTy, Ty, TyKind};
use biwac_span::{DefIdKind, Span, TyDefId};

use crate::ResolveError;

fn ty_from_primitive(ptyp: &PrimTyp, span: Span) -> Ty {
    match ptyp {
        PrimTyp::Int => Ty::new(TyKind::Int, span.into()),
        // TODO: Uint
        PrimTyp::Uint => Ty::new(TyKind::Int, span.into()),
        PrimTyp::Float => Ty::new(TyKind::Float, span.into()),
        PrimTyp::Bool => Ty::new(TyKind::Bool, span.into()),
    }
}

pub(crate) trait ResolveCtx {
    fn resolve_path(&self, path: &Path) -> Result<DefIdKind, ResolveError>;

    fn resolve_typ(&self, typ: &TypRepr) -> Result<TyKind, Vec<ResolveError>> {
        match &typ.val {
            TypReprVal::Primitive(PrimTyp::Int) => Ok(TyKind::Int),
            TypReprVal::Primitive(PrimTyp::Uint) => Ok(TyKind::Int), // TODO:
            TypReprVal::Primitive(PrimTyp::Float) => Ok(TyKind::Float),
            TypReprVal::Primitive(PrimTyp::Bool) => Ok(TyKind::Bool),
            TypReprVal::Defined(def_typ) => {
                let res_def_id = self.resolve_path(&def_typ.path);

                match &def_typ.genargs {
                    Some(genargs) => {
                        match genargs
                            .iter()
                            .map(|typ| Ok(Ty::new(self.resolve_typ(typ)?, typ.span.clone())))
                            .collect::<Result<Vec<Ty>, Vec<ResolveError>>>()
                        {
                            Ok(genargs) => match ty_def_id_try_from_path(&def_typ.path) {
                                Ok(ty_def_id) => Ok(TyKind::Defined(DefinedTy {
                                    def_id: ty_def_id,
                                    genargs,
                                })),
                                Err(e) => Err(vec![e]),
                            },
                            Err(mut errors) => match res_def_id {
                                Ok(_) => {
                                    if let Err(e) = ty_def_id_try_from_path(&def_typ.path) {
                                        errors.push(e);
                                    }
                                    Err(errors)
                                }
                                Err(e) => {
                                    errors.push(e);
                                    Err(errors)
                                }
                            },
                        }
                    }
                    None => match ty_def_id_try_from_path(&def_typ.path) {
                        Ok(ty_def_id) => Ok(TyKind::Defined(DefinedTy {
                            def_id: ty_def_id,
                            genargs: Vec::new(),
                        })),
                        Err(e) => Err(vec![e]),
                    },
                }
            }
            TypReprVal::SelfTyp => {
                self.opt_self_ty()
                    .ok_or(vec![ResolveError::UnexpectedSelfType {
                        span: typ.span.clone(),
                    }])
            }
        }
    }

    fn opt_self_ty(&self) -> Option<TyKind> {
        None
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

pub(crate) fn ty_def_id_try_from_path(path: &Path) -> Result<TyDefId, ResolveError> {
    match def_id_kind_try_from_path(path)? {
        DefIdKind::Ty(ty_def_id) => Ok(ty_def_id),
        DefIdKind::Mod(mod_id) => Err(ResolveError::TypeNotFoundModuleFound {
            path: Box::new(path.clone()),
            mod_id,
        }),
        DefIdKind::Val(val_def_id) => Err(ResolveError::TypeNotFoundValueFound {
            path: Box::new(path.clone()),
            def_id: val_def_id,
        }),
    }
}
