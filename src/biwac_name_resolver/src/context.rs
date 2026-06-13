pub(crate) mod fn_level;
pub(crate) mod impl_level;
pub(crate) mod module_level;
pub(crate) mod ty_def_level;

use biwac_ast::{Path, PrimTyp, TypRepr, TypReprVal};
use biwac_hir::TyKind;
use biwac_span::{Span, VarId};

use crate::{ResolveError, ResolveErrorHandler};

pub(crate) trait ResolveCtx {
    fn resolve_path(&self, path: &Path) -> Result<(), ResolveError>;

    fn resolve_typ(&self, typ: &TypRepr) -> Result<(), Vec<ResolveError>> {
        match &typ.val {
            TypReprVal::Primitive(PrimTyp::Int) => Ok(()),
            TypReprVal::Primitive(PrimTyp::Uint) => Ok(()), // TODO:
            TypReprVal::Primitive(PrimTyp::Float) => Ok(()),
            TypReprVal::Primitive(PrimTyp::Bool) => Ok(()),
            TypReprVal::Defined(def_typ) => {
                let mut errors = Vec::new();

                self.resolve_path(&def_typ.path).handle(&mut errors);

                if let Some(genargs) = &def_typ.genargs {
                    for typ in genargs {
                        self.resolve_typ(typ).handle(&mut errors);
                    }
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
            TypReprVal::SelfTyp => match self.opt_self_ty() {
                Some(_) => Ok(()),
                None => Err(vec![ResolveError::UnexpectedSelfType {
                    span: typ.span.clone(),
                }]),
            },
        }
    }

    fn opt_self_ty(&self) -> Option<TyKind> {
        None
    }
}

pub(crate) trait LocalResolveCtx: ResolveCtx {
    fn declare_variable(&mut self, ident: &biwac_ast::Ident) -> Result<VarId, ResolveError>;
    fn inner_scope<F: FnOnce(&mut Self) -> Result<(), Vec<ResolveError>>>(
        &mut self,
        f: F,
    ) -> Result<(), Vec<ResolveError>>;
    fn resolve_self_var(&self, span: &Span) -> Result<VarId, ResolveError>;
}
